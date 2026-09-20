use super::*;
use std::sync::mpsc::{Receiver, TryRecvError};

use crate::host::HeadlessCollabHost;
use op_collab::canonical_document_hash;
use op_editor_core::CollabConnectionPhase;

use crate::runtime::network::guest_command_channel_with_capacity_for_test;

fn owner_auth(display_name: Option<&str>) -> VerifiedAuthMetadata {
    VerifiedAuthMetadata {
        issuer: "https://issuer.example".into(),
        subject: "11111111-1111-1111-1111-111111111111".into(),
        device_id: "22222222-2222-2222-2222-222222222222".into(),
        proof_binding: "binding".into(),
        expires_at_unix_ms: 10_000,
        display_name: display_name.map(str::to_owned),
        avatar_url: None,
    }
}

fn joining_guest_runtime() -> (
    CollabRuntime,
    HeadlessCollabHost,
    Receiver<GuestNetworkCommand>,
) {
    let mut host = HeadlessCollabHost::new();
    host.editor_state_mut()
        .editor_ui
        .collab
        .set_phase(CollabConnectionPhase::Joining);
    let (network, commands) = guest_command_channel_with_capacity_for_test(4);
    let mut runtime = CollabRuntime::new();
    runtime.network = Some(network);
    (runtime, host, commands)
}

fn published_request_key(host: &HeadlessCollabHost) -> CollabAdmissionRequestKey {
    host.editor_state()
        .editor_ui
        .collab
        .pending_owner_confirmation()
        .expect("a verified owner identity is awaiting confirmation")
        .request_key()
        .clone()
}

#[test]
fn a_confirmed_foreign_owner_is_admitted_and_nothing_is_applied_before_the_decision() {
    let (mut runtime, mut host, commands) = joining_guest_runtime();
    let before = canonical_document_hash(&host.editor_state().doc).unwrap();

    runtime
        .owner_identity_unconfirmed(
            ConnectionKey::new(1).unwrap(),
            owner_auth(Some("Ada")),
            &mut host,
        )
        .unwrap();

    // The worker stays blocked: no decision has been sent, and nothing from
    // the session has been applied or projected.
    assert!(matches!(commands.try_recv(), Err(TryRecvError::Empty)));
    assert!(runtime.actor.is_none());
    assert!(host
        .editor_state()
        .editor_ui
        .collab
        .authenticated_session()
        .is_none());
    assert!(host
        .editor_state()
        .editor_ui
        .collab
        .participants()
        .is_empty());
    assert_eq!(
        canonical_document_hash(&host.editor_state().doc).unwrap(),
        before
    );

    let pending = host
        .editor_state()
        .editor_ui
        .collab
        .pending_owner_confirmation()
        .expect("identity is shown before the decision");
    assert_eq!(
        pending.identity().subject(),
        "11111111-1111-1111-1111-111111111111"
    );
    assert_eq!(
        pending.identity().device_id(),
        "22222222-2222-2222-2222-222222222222"
    );
    assert_eq!(pending.identity().claimed_display_name(), Some("Ada"));

    let request_key = published_request_key(&host);
    runtime
        .resolve_owner_confirmation(
            &CollabUiAction::ConfirmOwnerIdentity {
                request_key: request_key.clone(),
            },
            &mut host,
        )
        .unwrap();
    assert!(matches!(
        commands.try_recv(),
        Ok(GuestNetworkCommand::OwnerIdentityDecision(
            GuestOwnerDecision::Confirm
        ))
    ));
    assert!(host
        .editor_state()
        .editor_ui
        .collab
        .pending_owner_confirmation()
        .is_none());

    // The decision is single-use: replaying the same key answers nothing.
    runtime
        .resolve_owner_confirmation(
            &CollabUiAction::RejectOwnerIdentity { request_key },
            &mut host,
        )
        .unwrap();
    assert!(matches!(commands.try_recv(), Err(TryRecvError::Empty)));
}

#[test]
fn a_rejected_owner_closes_the_connection_without_starting_a_session() {
    let (mut runtime, mut host, commands) = joining_guest_runtime();
    let before = canonical_document_hash(&host.editor_state().doc).unwrap();
    runtime
        .owner_identity_unconfirmed(ConnectionKey::new(1).unwrap(), owner_auth(None), &mut host)
        .unwrap();
    let request_key = published_request_key(&host);

    runtime
        .resolve_owner_confirmation(
            &CollabUiAction::RejectOwnerIdentity { request_key },
            &mut host,
        )
        .unwrap();

    assert!(matches!(
        commands.try_recv(),
        Ok(GuestNetworkCommand::OwnerIdentityDecision(
            GuestOwnerDecision::Reject
        ))
    ));
    assert!(runtime.pending_owner_confirmation.is_none());
    assert!(runtime.actor.is_none());
    assert!(host
        .editor_state()
        .editor_ui
        .collab
        .pending_owner_confirmation()
        .is_none());
    assert_eq!(
        canonical_document_hash(&host.editor_state().doc).unwrap(),
        before
    );
}

#[test]
fn a_stale_request_key_can_never_answer_the_live_connection() {
    let (mut runtime, mut host, commands) = joining_guest_runtime();
    runtime
        .owner_identity_unconfirmed(ConnectionKey::new(1).unwrap(), owner_auth(None), &mut host)
        .unwrap();

    let forged = CollabAdmissionRequestKey::new("guessed-request-key").unwrap();
    runtime
        .resolve_owner_confirmation(
            &CollabUiAction::ConfirmOwnerIdentity {
                request_key: forged,
            },
            &mut host,
        )
        .unwrap();

    assert!(matches!(commands.try_recv(), Err(TryRecvError::Empty)));
    assert!(
        runtime.pending_owner_confirmation.is_some(),
        "the live request survives a mismatched answer"
    );
}

#[test]
fn an_unnameable_or_duplicate_identity_is_refused_instead_of_prompted() {
    let (mut runtime, mut host, commands) = joining_guest_runtime();
    let mut blank = owner_auth(None);
    blank.subject = String::new();
    let error = runtime
        .owner_identity_unconfirmed(ConnectionKey::new(1).unwrap(), blank, &mut host)
        .expect_err("an identity with no subject is not a decision a human can make");
    assert_eq!(error.failure, CollabRuntimeFailure::Protocol);
    assert!(matches!(
        commands.try_recv(),
        Ok(GuestNetworkCommand::OwnerIdentityDecision(
            GuestOwnerDecision::Reject
        ))
    ));
    assert!(host
        .editor_state()
        .editor_ui
        .collab
        .pending_owner_confirmation()
        .is_none());

    // A second prompt on the same runtime would be a second, unanchored
    // consent; it is refused rather than queued.
    runtime
        .owner_identity_unconfirmed(ConnectionKey::new(1).unwrap(), owner_auth(None), &mut host)
        .unwrap();
    assert!(matches!(
        commands.try_recv(),
        Ok(GuestNetworkCommand::OwnerIdentityDecision(
            GuestOwnerDecision::Confirm
        )) | Err(TryRecvError::Empty)
    ));
    let error = runtime
        .owner_identity_unconfirmed(ConnectionKey::new(2).unwrap(), owner_auth(None), &mut host)
        .expect_err("only one owner confirmation may be outstanding");
    assert_eq!(error.failure, CollabRuntimeFailure::Protocol);
}

#[test]
fn the_declined_worker_failure_ends_the_join_cleanly_at_idle() {
    // What the blocked worker returns when the human declines. It must land as
    // a completed setup decision — back to Idle with an explanatory notice —
    // not as a live session that broke and offers a reconnect.
    let (mut runtime, mut host, _commands) = joining_guest_runtime();
    runtime.fail_network(&mut host, CollabRuntimeFailure::OwnerIdentityRejected);

    let collab = &host.editor_state().editor_ui.collab;
    assert_eq!(collab.phase, CollabConnectionPhase::Idle);
    assert_eq!(
        collab.notice.map(|notice| notice.kind),
        Some(op_editor_core::CollabNoticeKind::Connect(
            op_editor_core::CollabConnectErrorUi::OwnerNotConfirmed
        ))
    );
    assert!(collab.authenticated_session().is_none());
    assert!(runtime.actor.is_none());
    assert!(runtime.pending_owner_confirmation.is_none());
    assert!(collab.pending_owner_confirmation().is_none());
}
