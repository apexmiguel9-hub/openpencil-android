//! GUI-owned local-edit gestures: capture begin/finish and the replay of a
//! conflict-discarded edit. Split off the runtime spine at the
//! 800-line cap; pure code motion.

/// What a session did with a completed local-edit capture.
///
/// `finish_local_edit` used to return `bool`, which could not distinguish
/// "the session sequenced this" from "the session rejected it and rolled it
/// back" — so a rejected whole-document push was answered 200 with a version
/// bump and the browser never learned to refetch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalEditOutcome {
    /// Sequenced and committed.
    Committed,
    /// The capture closed with nothing to send — the document did not move.
    NoChange,
    /// The session refused the edit and rolled it back. The caller's copy is
    /// now behind and must be refetched.
    Rejected,
    /// The capture could not be closed cleanly and the session was torn down.
    ///
    /// `document_rolled_back` says whether the document went back to its
    /// pre-edit content. It is NOT always true: the standalone fallback
    /// (a delivery failure that retires the session) deliberately KEEPS the
    /// edit, because the user's work is still theirs even though the session
    /// is gone. A caller that undoes side effects — thumbnails, caches — must
    /// only undo them when the document was actually rolled back, or it
    /// desynchronises them from a document that kept its new content.
    Failed { document_rolled_back: bool },
}

use op_editor_core::{CollabNoticeKind, CollabRejectUiCode};

use super::actor::EditorActor;
use super::types::CollabRuntimeError;
use super::CollabRuntime;
use crate::host::CollabHost;

impl CollabRuntime {
    /// Whether a GUI edit capture is open right now.
    ///
    /// A pointer gesture is one transaction: press opens the capture, release
    /// closes it. Actions queued by that same press (approving an admission,
    /// for example) must not be drained mid-gesture, because the session's
    /// document actor refuses to act while a capture is in flight.
    pub const fn local_edit_in_flight(&self) -> bool {
        self.transaction_active
    }

    /// Start one GUI-owned edit capture; `false` means busy or no live session is bound.
    pub fn begin_local_edit(&mut self, host: &mut impl CollabHost) -> bool {
        if self.transaction_active {
            return false;
        }
        let started = match self.actor.as_mut() {
            Some(EditorActor::Owner(actor)) => actor.session.begin_local_edit(host).is_ok(),
            Some(EditorActor::Guest(actor)) => actor.session.begin_local_edit(host).is_ok(),
            None => return false,
        };
        match started {
            true => {
                self.transaction_active = true;
                true
            }
            false => {
                self.set_notice(host, CollabNoticeKind::Reject(CollabRejectUiCode::ReadOnly));
                false
            }
        }
    }

    pub fn finish_local_edit(&mut self, host: &mut impl CollabHost) -> LocalEditOutcome {
        if !std::mem::take(&mut self.transaction_active) {
            // Nothing was captured, so whatever the caller installed is still
            // in place — not rolled back.
            return LocalEditOutcome::Failed {
                document_rolled_back: false,
            };
        }
        let Some(mut actor) = self.actor.take() else {
            return LocalEditOutcome::Failed {
                document_rolled_back: false,
            };
        };
        // Cleared here so a stale resolution from an earlier edit cannot be
        // read as this one's answer.
        self.last_local_edit = None;
        let result = match &mut actor {
            EditorActor::Owner(owner) => match owner.session.finish_local_edit(host) {
                Ok(output) => self.route_owner_output(owner, output, host),
                Err(_) => Err(CollabRuntimeError::invalid_session()),
            },
            EditorActor::Guest(guest) => match guest.session.finish_local_edit(host) {
                Ok(output) => self.route_guest_output(guest, output, host),
                Err(_) => Err(CollabRuntimeError::invalid_session()),
            },
        };
        self.actor = Some(actor);
        match result {
            // The routing layer records what the session actually decided; a
            // bare `Ok` used to be reported as success even when the session
            // had REJECTED and rolled the edit back, so the caller answered
            // 200 for a write that never landed.
            Ok(()) => self
                .last_local_edit
                .take()
                .unwrap_or(LocalEditOutcome::Committed),
            Err(error) => {
                // A local editor/core failure can occur after the document
                // changed or the sequencer prepared a commit. Continuing to
                // advertise Active would silently fork owner and guests.
                self.fail_network(host, error.failure);
                // The standalone fallback keeps the edited document — see
                // `reliable_owner_delivery_failure_falls_back_to_standalone`.
                LocalEditOutcome::Failed {
                    document_rolled_back: false,
                }
            }
        }
    }

    /// Resubmit the stashed conflict-discarded property edit as a brand-new
    /// local edit over the current authoritative document.
    ///
    /// The replay deliberately reasserts the dropped desired values over
    /// whatever the fields hold now; it goes through the normal local-edit
    /// pipeline, so a fresh conflict simply produces a fresh stash.
    pub(super) fn reapply_discarded(&mut self, host: &mut impl CollabHost) {
        if self.discarded_property_edit.is_none() {
            return;
        }
        if !self.begin_local_edit(host) {
            // Busy or read-only; begin_local_edit already raised the notice.
            // The stash is kept so the user can retry once the lane is free.
            return;
        }
        let changes = self
            .discarded_property_edit
            .take()
            .expect("stash presence was checked above");
        host.editor_state_mut().editor_ui.collab.discarded_edit = None;
        match op_collab::reapply_property_changes(&host.editor_state().doc, &changes) {
            Ok(desired) => {
                // Mutating the document inside the capture mirrors a GUI
                // gesture: end_local_edit diffs it and bumps the revision.
                host.editor_state_mut().doc = desired;
            }
            Err(_) => {
                // The target node no longer exists; finish as a no-op.
                self.set_notice(host, CollabNoticeKind::Reject(CollabRejectUiCode::Conflict));
            }
        }
        self.finish_local_edit(host);
    }
}
