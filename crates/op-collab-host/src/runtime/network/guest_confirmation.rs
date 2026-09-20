//! The guest's blocking wait for a human decision about the owner identity.
//!
//! Mirrors the owner's `await_owner_approval`: the worker holds the socket in
//! its verified-but-unauthorized state and refuses to advance the admission
//! state machine until the GUI answers. Nothing has been requested from the
//! peer at this point, so no document, snapshot, presence, or session name can
//! exist yet, let alone be applied.

use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};
use std::time::{Duration, Instant};

use op_collab::ByeReason;

use super::super::types::{CollabRuntimeFailure, GuestNetworkCommand, GuestOwnerDecision};

/// Matches the owner's approval budget so both sides of an interactive
/// handshake give a human the same amount of time to answer.
pub(super) const OWNER_CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(120);
const CONFIRMATION_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// How the wait ended.
pub(super) enum GuestConfirmationOutcome {
    /// The human accepted the verified identity; the join may proceed.
    Confirmed,
    /// The human declined, or never answered. The caller must close.
    Declined,
    /// The worker is shutting down; the caller returns without a failure.
    Cancelled,
}

pub(super) fn await_owner_confirmation(
    commands: &Receiver<GuestNetworkCommand>,
    shutdown: &Receiver<ByeReason>,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<GuestConfirmationOutcome, CollabRuntimeFailure> {
    let deadline = Instant::now()
        .checked_add(OWNER_CONFIRMATION_TIMEOUT)
        .ok_or(CollabRuntimeFailure::Transport)?;
    loop {
        if is_cancelled() {
            return Ok(GuestConfirmationOutcome::Cancelled);
        }
        match shutdown.try_recv() {
            Ok(_) | Err(TryRecvError::Disconnected) => {
                return Ok(GuestConfirmationOutcome::Cancelled)
            }
            Err(TryRecvError::Empty) => {}
        }
        let now = Instant::now();
        if now >= deadline {
            // An unanswered prompt is not consent.
            return Ok(GuestConfirmationOutcome::Declined);
        }
        let wait = deadline
            .saturating_duration_since(now)
            .min(CONFIRMATION_POLL_INTERVAL);
        match commands.recv_timeout(wait) {
            Ok(GuestNetworkCommand::OwnerIdentityDecision(GuestOwnerDecision::Confirm)) => {
                return Ok(GuestConfirmationOutcome::Confirmed)
            }
            Ok(GuestNetworkCommand::OwnerIdentityDecision(GuestOwnerDecision::Reject)) => {
                return Ok(GuestConfirmationOutcome::Declined)
            }
            // Nothing else can legitimately be queued for a connection that
            // has not been admitted; treat it as a protocol fault rather than
            // letting a stray command imply consent.
            Ok(GuestNetworkCommand::Send { .. } | GuestNetworkCommand::VerifyRenewal(_)) => {
                return Err(CollabRuntimeFailure::Protocol)
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Ok(GuestConfirmationOutcome::Cancelled),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_collab_transport::TransportConfig;
    use std::sync::mpsc::sync_channel;

    fn never_cancelled() -> impl Fn() -> bool {
        || false
    }

    #[test]
    fn a_confirmation_and_a_rejection_are_both_answered_immediately() {
        let (commands_tx, commands) = sync_channel(4);
        let (_shutdown_tx, shutdown) = sync_channel::<ByeReason>(1);
        commands_tx
            .send(GuestNetworkCommand::OwnerIdentityDecision(
                GuestOwnerDecision::Confirm,
            ))
            .unwrap();
        assert!(matches!(
            await_owner_confirmation(&commands, &shutdown, &never_cancelled()).unwrap(),
            GuestConfirmationOutcome::Confirmed
        ));

        commands_tx
            .send(GuestNetworkCommand::OwnerIdentityDecision(
                GuestOwnerDecision::Reject,
            ))
            .unwrap();
        assert!(matches!(
            await_owner_confirmation(&commands, &shutdown, &never_cancelled()).unwrap(),
            GuestConfirmationOutcome::Declined
        ));
    }

    #[test]
    fn a_cancelled_or_disconnected_wait_never_reports_consent() {
        let (commands_tx, commands) = sync_channel::<GuestNetworkCommand>(1);
        let (_shutdown_tx, shutdown) = sync_channel::<ByeReason>(1);
        assert!(matches!(
            await_owner_confirmation(&commands, &shutdown, &|| true).unwrap(),
            GuestConfirmationOutcome::Cancelled
        ));
        drop(commands_tx);
        assert!(matches!(
            await_owner_confirmation(&commands, &shutdown, &never_cancelled()).unwrap(),
            GuestConfirmationOutcome::Cancelled
        ));
    }

    #[test]
    fn the_confirmation_budget_outlives_the_transport_admission_deadline() {
        // A human must be able to answer before the socket layer gives up on
        // the handshake for them.
        assert!(OWNER_CONFIRMATION_TIMEOUT >= Duration::from_secs(60));
        assert!(
            OWNER_CONFIRMATION_TIMEOUT > TransportConfig::default().timeouts.admission,
            "the prompt must not be pre-empted by the admission timeout"
        );
    }
}
