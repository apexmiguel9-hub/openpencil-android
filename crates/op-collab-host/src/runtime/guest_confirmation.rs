//! GUI-owned guest confirmation of the owner identity behind an unpinned join.
//!
//! The mirror image of `admission.rs`: there the owner decides whether to admit
//! a verified guest, here the guest decides whether to accept a verified owner.
//! The shapes are deliberately the same — a random routing key in shared UI
//! state, the live connection kept in the runtime — with one deliberate
//! difference. The owner's prompt shows no identity, because an owner is
//! deciding about someone who asked to join *their* session. A guest is
//! deciding whose session to enter, and cannot answer that without seeing who
//! the peer is, so the verified subject and device id do cross into UI state.
//! What crosses is only ever the verified ticket's own claims.

use op_collab::{ConnectionKey, VerifiedAuthMetadata};
use op_editor_core::{
    CollabAdmissionRequestKey, CollabConnectionPhase, CollabOwnerIdentityUi, CollabUiAction,
};

use super::actor::{random_identifier, EditorActor};
use super::types::{
    CollabRuntimeError, CollabRuntimeFailure, GuestNetworkCommand, GuestOwnerDecision,
};
use super::CollabRuntime;
use crate::host::CollabHost;

/// The live connection a published confirmation request routes back to.
pub(super) struct PendingOwnerConfirmation {
    pub(super) request_key: CollabAdmissionRequestKey,
    pub(super) connection: ConnectionKey,
}

impl CollabRuntime {
    /// A guest verified the owner's ticket over an unpinned join. Publish the
    /// verified identity and wait; the worker is blocked until we answer.
    pub(super) fn owner_identity_unconfirmed(
        &mut self,
        connection: ConnectionKey,
        auth: VerifiedAuthMetadata,
        host: &mut impl CollabHost,
    ) -> Result<(), CollabRuntimeError> {
        // A session that already has an actor is past this decision; a stray
        // prompt there would be a second, unanchored consent.
        if self.actor.is_some() || self.pending_owner_confirmation.is_some() {
            return self.decline_owner_identity(CollabRuntimeFailure::Protocol);
        }
        let Some(identity) = CollabOwnerIdentityUi::from_verified(
            &auth.subject,
            &auth.device_id,
            auth.display_name.as_deref(),
            auth.avatar_url.as_deref(),
        ) else {
            // Nothing nameable to confirm; refuse rather than prompt about a
            // blank peer.
            return self.decline_owner_identity(CollabRuntimeFailure::Protocol);
        };
        let request_key = random_identifier("owner-confirm")
            .ok()
            .and_then(CollabAdmissionRequestKey::new);
        let Some(request_key) = request_key else {
            return self.decline_owner_identity(CollabRuntimeFailure::SecureKeyUnavailable);
        };
        host.editor_state_mut()
            .editor_ui
            .collab
            .set_phase(CollabConnectionPhase::Authenticating);
        if !host
            .editor_state_mut()
            .editor_ui
            .collab
            .publish_owner_confirmation(request_key.clone(), identity)
        {
            return self.decline_owner_identity(CollabRuntimeFailure::InvalidSession);
        }
        self.pending_owner_confirmation = Some(PendingOwnerConfirmation {
            request_key,
            connection,
        });
        Ok(())
    }

    pub(super) fn resolve_owner_confirmation(
        &mut self,
        action: &CollabUiAction,
        host: &mut impl CollabHost,
    ) -> Result<(), CollabRuntimeError> {
        let (request_key, confirm) = match action {
            CollabUiAction::ConfirmOwnerIdentity { request_key } => (request_key, true),
            CollabUiAction::RejectOwnerIdentity { request_key } => (request_key, false),
            _ => return Err(CollabRuntimeError::invalid_session()),
        };
        host.editor_state_mut()
            .editor_ui
            .collab
            .clear_owner_confirmation();
        // A key that does not match the live request answers nothing: a stale
        // click must never be able to confirm a different connection.
        let Some(pending) = self
            .pending_owner_confirmation
            .take_if(|pending| pending.request_key == *request_key)
        else {
            return Ok(());
        };
        debug_assert!(matches!(self.actor, None | Some(EditorActor::Guest(_))));
        let _ = pending.connection;
        let decision = if confirm {
            GuestOwnerDecision::Confirm
        } else {
            GuestOwnerDecision::Reject
        };
        self.send_guest_owner_decision(decision)
    }

    pub(super) fn clear_owner_confirmation(&mut self, host: &mut impl CollabHost) {
        self.pending_owner_confirmation = None;
        host.editor_state_mut()
            .editor_ui
            .collab
            .clear_owner_confirmation();
    }

    /// Tell the blocked worker to close without ever showing a prompt.
    fn decline_owner_identity(
        &mut self,
        failure: CollabRuntimeFailure,
    ) -> Result<(), CollabRuntimeError> {
        let _ = self.send_guest_owner_decision(GuestOwnerDecision::Reject);
        Err(CollabRuntimeError::new(failure))
    }

    fn send_guest_owner_decision(
        &self,
        decision: GuestOwnerDecision,
    ) -> Result<(), CollabRuntimeError> {
        self.network
            .as_ref()
            .ok_or_else(CollabRuntimeError::invalid_session)?
            .send_guest(GuestNetworkCommand::OwnerIdentityDecision(decision))
            .map_err(|_| CollabRuntimeError::resource_limit())
    }
}

#[cfg(test)]
#[path = "guest_confirmation_tests.rs"]
mod tests;
