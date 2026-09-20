//! Versioned wire projection of the collaboration UI state.
//!
//! The daemon owns the collaboration runtime; the browser owns the panel that
//! paints it. This module is the contract between them, and it is deliberately
//! a **separate type family** from the internal `collab_*_ui` types rather than
//! a set of serde derives on them:
//!
//! * the internal types carry opaque tokens and redacting `Debug` impls whose
//!   contracts a derive would quietly undo;
//! * the wire needs to stay stable across internal refactors, so it carries
//!   [`COLLAB_WIRE_VERSION`] and its own enums;
//! * decoding must re-run every sanitising constructor, which only an explicit
//!   mapping can guarantee.
//!
//! Two sequence numbers ride along and mean different things. `documentRevision`
//! changes only when document *content* changes, so it is what a client polls to
//! decide whether to refetch the document. `collabSeq` changes whenever this
//! projection changes — a peer joining, a cursor moving, a notice appearing —
//! and must never by itself trigger a document fetch.

mod action;
mod parts;

pub use action::{CollabActionWire, CollabActionWireError, CollabWireCommand};
pub use parts::{
    CollabAdmissionWire, CollabAvailabilityWire, CollabConnectErrorWire, CollabConnectionPathWire,
    CollabDiscardedEditWire, CollabDiscoveredWire, CollabLocalPresenceWire, CollabNoticeKindWire,
    CollabNoticeWire, CollabOwnerConfirmationWire, CollabPanelViewWire, CollabParticipantWire,
    CollabPendingEditWire, CollabPhaseWire, CollabPointWire, CollabPresenceWire,
    CollabRejectCodeWire, CollabRelayRegionWire, CollabRoleWire,
};

use serde::{Deserialize, Serialize};

use crate::{
    CollabAdmissionRequestKey, CollabInviteCode, CollabOwnerIdentityUi, CollabShareEndpoint,
    CollabUiState, DiscoveredCollabEndpoint,
};

/// Bumped whenever an existing field changes meaning or disappears.
///
/// Purely additive changes do not bump it: unknown fields are ignored on
/// decode, so an older client keeps working against a newer daemon.
pub const COLLAB_WIRE_VERSION: u32 = 1;

/// The parts of the panel the daemon is authoritative for.
///
/// Panel open/close, the current screen, hover, and the join-address draft are
/// deliberately absent: those are local UI, and echoing them back would make
/// every poll fight the user's own typing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollabPanelWire {
    pub relay_region: CollabRelayRegionWire,
    pub discovered: Vec<CollabDiscoveredWire>,
}

/// The authenticated session, once there is one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollabSessionWire {
    pub session_name: String,
    pub role: CollabRoleWire,
    pub share_endpoint: Option<String>,
    pub invite: Option<String>,
    pub connection: Option<CollabConnectionPathWire>,
    /// Owner-assigned id namespace for this peer.
    ///
    /// A client that creates nodes MUST mint their ids from this namespace.
    /// The protocol replays ids verbatim, so two peers minting from a private
    /// sequential counter would eventually agree on an id for two different
    /// nodes and silently fork the document.
    ///
    /// Additive and optional: a client that finds it absent — an older daemon,
    /// or a session whose namespace is not yet known — must refuse to create
    /// nodes rather than fall back to a local counter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer_namespace: Option<String>,
}

/// The whole collaboration projection for one poll.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollabStateWire {
    pub wire_version: u32,
    /// Bumps on any change to this projection.
    pub collab_seq: u64,
    /// Bumps only on document-content change.
    pub document_revision: u64,
    pub availability: CollabAvailabilityWire,
    pub phase: CollabPhaseWire,
    pub panel: CollabPanelWire,
    pub session: Option<CollabSessionWire>,
    pub participants: Vec<CollabParticipantWire>,
    pub presence: Vec<CollabPresenceWire>,
    pub admissions: Vec<CollabAdmissionWire>,
    pub owner_confirmation: Option<CollabOwnerConfirmationWire>,
    pub notice: Option<CollabNoticeWire>,
    pub pending_edit: CollabPendingEditWire,
    pub discarded_edit: Option<CollabDiscardedEditWire>,
}

impl CollabStateWire {
    /// Project the live UI state for transmission.
    pub fn from_ui(ui: &CollabUiState, collab_seq: u64, document_revision: u64) -> Self {
        let session = ui.authenticated_session().map(|session| {
            let public = ui.public_session();
            CollabSessionWire {
                session_name: session.session_name.clone(),
                role: session.role.into(),
                share_endpoint: session
                    .share_endpoint
                    .as_ref()
                    .map(|endpoint| endpoint.as_str().to_owned()),
                invite: public
                    .and_then(|public| public.invite())
                    .map(|invite| invite.as_str().to_owned()),
                connection: public
                    .and_then(|public| public.connection())
                    .map(Into::into),
                // Not part of the shared UI projection — it lives in the
                // session actor, so the daemon layers it on with
                // [`Self::with_peer_namespace`].
                peer_namespace: None,
            }
        });
        Self {
            wire_version: COLLAB_WIRE_VERSION,
            collab_seq,
            document_revision,
            availability: ui.availability.into(),
            phase: ui.phase.into(),
            panel: CollabPanelWire {
                relay_region: ui.panel.relay_region.into(),
                discovered: ui
                    .panel
                    .discovered
                    .iter()
                    .map(|found| CollabDiscoveredWire {
                        discovery_id: found.discovery_id.clone(),
                        endpoint: found.endpoint.clone(),
                        compatible: found.compatible,
                    })
                    .collect(),
            },
            session,
            participants: ui.participants().iter().map(Into::into).collect(),
            presence: ui.presence().iter().map(Into::into).collect(),
            admissions: ui
                .pending_admissions()
                .iter()
                .map(|pending| CollabAdmissionWire {
                    request_key: pending.request_key().as_str().to_owned(),
                    resume_role: pending.resume_role().map(Into::into),
                })
                .collect(),
            owner_confirmation: ui.pending_owner_confirmation().map(|pending| {
                let identity = pending.identity();
                CollabOwnerConfirmationWire {
                    request_key: pending.request_key().as_str().to_owned(),
                    subject: identity.subject().to_owned(),
                    device_id: identity.device_id().to_owned(),
                    display_name: identity.claimed_display_name().map(str::to_owned),
                    avatar_url: identity.claimed_avatar_url().map(str::to_owned),
                }
            }),
            notice: ui.notice.map(Into::into),
            pending_edit: ui.pending_edit.into(),
            discarded_edit: ui.discarded_edit.as_ref().map(Into::into),
        }
    }

    /// Attach the owner-assigned id namespace to an already-built projection.
    ///
    /// Separate from [`Self::from_ui`] because the namespace is owned by the
    /// session actor rather than the UI state the panel paints; only the host
    /// running the session can supply it. A projection with no session, or a
    /// session whose namespace is not yet assigned, keeps `None` — and a client
    /// reading `None` must not create nodes.
    #[must_use]
    pub fn with_peer_namespace(mut self, namespace: Option<String>) -> Self {
        if let Some(session) = self.session.as_mut() {
            session.peer_namespace = namespace;
        }
        self
    }

    /// Install this projection into a client's UI state.
    ///
    /// Every write goes through a sanitising `set_*` / `publish_*` method, and
    /// the call order is load-bearing because each of those fails closed on the
    /// *current* phase: an authenticated session may only be installed on an
    /// authenticated phase, an owner confirmation only while `Authenticating`
    /// and before a session exists, and admissions only on an active owner.
    /// Writing the phase first and the phase-gated payloads after is what keeps
    /// a legitimate payload from being silently dropped.
    pub fn apply_to(&self, ui: &mut CollabUiState, now_ms: u64) {
        ui.set_availability(self.availability.into());

        let participants: Vec<_> = self
            .participants
            .iter()
            .map(CollabParticipantWire::to_ui)
            .collect();
        match self.session.as_ref() {
            Some(session) => {
                let installed = ui.set_authenticated_session(
                    self.phase.into(),
                    crate::AuthenticatedCollabSession {
                        session_name: session.session_name.clone(),
                        role: session.role.into(),
                        share_endpoint: session
                            .share_endpoint
                            .as_deref()
                            .and_then(CollabShareEndpoint::new),
                    },
                    participants,
                );
                if installed {
                    if let Some(connection) = session.connection {
                        ui.set_public_session(
                            session.invite.as_deref().and_then(CollabInviteCode::new),
                            connection.into(),
                        );
                    }
                }
            }
            None => {
                ui.clear_authenticated();
                ui.set_phase(self.phase.into());
                // The owner-confirmation prompt is the one payload that exists
                // before a session does; it is gated on exactly this phase.
                if let Some(confirmation) = self.owner_confirmation.as_ref() {
                    if let (Some(request_key), Some(identity)) = (
                        CollabAdmissionRequestKey::new(confirmation.request_key.as_str()),
                        CollabOwnerIdentityUi::from_verified(
                            &confirmation.subject,
                            &confirmation.device_id,
                            confirmation.display_name.as_deref(),
                            confirmation.avatar_url.as_deref(),
                        ),
                    ) {
                        ui.publish_owner_confirmation(request_key, identity);
                    }
                }
            }
        }

        ui.clear_pending_admissions();
        for admission in &self.admissions {
            if let Some(request_key) =
                CollabAdmissionRequestKey::new(admission.request_key.as_str())
            {
                ui.publish_pending_admission(request_key, admission.resume_role.map(Into::into));
            }
        }

        ui.queue_presence_snapshot(
            self.presence
                .iter()
                .map(CollabPresenceWire::to_ui)
                .collect(),
        );
        ui.flush_presence(now_ms);

        ui.panel.relay_region = self.panel.relay_region.into();
        ui.panel.discovered = std::sync::Arc::new(
            self.panel
                .discovered
                .iter()
                .map(|found| DiscoveredCollabEndpoint {
                    discovery_id: found.discovery_id.clone(),
                    endpoint: found.endpoint.clone(),
                    compatible: found.compatible,
                })
                .collect(),
        );
        ui.pending_edit = self.pending_edit.into();
        ui.discarded_edit = self
            .discarded_edit
            .as_ref()
            .map(CollabDiscardedEditWire::to_ui);
    }
}

/// Read the collaboration sequence out of a `GET /api/mcp/version` body.
///
/// The daemon answers `{"version":N,"collabSeq":M}` on that one probe, so a
/// client polls once and routes each number to the loop that cares: `version`
/// drives the document fetch, `collabSeq` drives this projection. A body
/// without the field — an older daemon — reads as `None`, which simply leaves
/// collaboration idle.
pub fn parse_collab_seq_probe(body: &str) -> Option<u64> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    value.get("collabSeq")?.as_u64()
}

#[cfg(test)]
#[path = "collab_wire/tests.rs"]
mod tests;
