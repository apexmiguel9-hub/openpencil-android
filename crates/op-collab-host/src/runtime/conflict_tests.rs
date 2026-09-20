//! Conflict-stash and replay coverage, plus the owner-retaining guest
//! harness it needs. Split off `runtime/tests.rs` at the 800-line
//! cap; pure code motion.

use std::sync::mpsc::Receiver;
use std::time::Duration;

use crate::host::HeadlessCollabHost;
use op_collab::{
    canonical_document_hash, CollabMessage, ConnectionKey, Epoch, FrameEnvelope,
    GuestConnectionState, Role, SessionId,
};
use op_editor_core::{CollabConnectionPhase, CollabNoticeKind, CollabPendingEditUi};

use super::actor::{set_guest_ui, EditorActor, GuestActor, OwnerActor};
use super::network::guest_command_channel_with_capacity_for_test;
use super::tests::{auth, connection, document_named, SESSION};
use super::types::{GuestNetworkCommand, NetworkEvent};
use super::CollabRuntime;

/// Like `tests::guest_runtime`, but keeps the authoring owner around so tests
/// can mint authentic authoritative commits against the shared session.
pub(super) fn guest_runtime_with_owner(
    capacity: usize,
) -> (
    CollabRuntime,
    HeadlessCollabHost,
    Receiver<GuestNetworkCommand>,
    ConnectionKey,
    op_collab::Welcome,
    OwnerActor,
    HeadlessCollabHost,
) {
    let mut owner_host = HeadlessCollabHost::new();
    owner_host.editor_state_mut().doc = document_named("Before");
    let mut owner =
        OwnerActor::new(SessionId::from(SESSION), Epoch(1), auth(0), &mut owner_host).unwrap();
    let connection = connection(2);
    let grant = owner.grant_new_peer(auth(1), Role::Editor).unwrap();
    let activation = owner
        .session
        .activate_peer(connection, grant, &owner_host)
        .unwrap();
    let welcome = activation.welcome.clone();

    let mut host = HeadlessCollabHost::new();
    let mut guest = GuestActor::new(
        SessionId::from(SESSION),
        Epoch(1),
        activation.welcome,
        connection,
        &mut host,
    )
    .unwrap();
    guest
        .session
        .accept_frame(
            FrameEnvelope::new(
                SessionId::from(SESSION),
                Epoch(1),
                CollabMessage::Snapshot(Box::new(activation.snapshot.unwrap())),
            ),
            &mut host,
        )
        .unwrap();
    assert_eq!(guest.session.core().state(), GuestConnectionState::Active);
    set_guest_ui(&mut host, &guest, CollabConnectionPhase::Active);

    let (network, commands) = guest_command_channel_with_capacity_for_test(capacity);
    let mut runtime = CollabRuntime::new();
    runtime.network = Some(network);
    runtime.actor = Some(EditorActor::Guest(Box::new(guest)));
    (
        runtime, host, commands, connection, welcome, owner, owner_host,
    )
}

#[test]
fn property_conflict_stashes_discarded_edit_and_reapply_resubmits_it() {
    let (mut runtime, mut host, commands, connection, _welcome, mut owner, mut owner_host) =
        guest_runtime_with_owner(8);

    // Guest optimistically renames the shared node.
    assert!(runtime.begin_local_edit(&mut host));
    host.editor_state_mut().doc = document_named("Guest intent");
    assert!(!matches!(
        runtime.finish_local_edit(&mut host),
        crate::runtime::local_edit::LocalEditOutcome::Failed { .. }
    ));
    let _submit = commands.recv_timeout(Duration::from_secs(1)).unwrap();

    // The owner concurrently renames the same field and wins seq 1.
    owner.session.begin_local_edit(&owner_host).unwrap();
    owner_host.editor_state_mut().doc = document_named("Owner intent");
    let output = owner.session.finish_local_edit(&mut owner_host).unwrap();
    let commit = output
        .effects
        .iter()
        .find_map(|effect| match effect {
            op_collab::OwnerEffect::BroadcastCommit { commit } => Some(commit.as_ref().clone()),
            _ => None,
        })
        .expect("owner local edit broadcasts a commit");
    runtime.handle_event(
        NetworkEvent::Frame {
            connection,
            frame: FrameEnvelope::new(
                SessionId::from(SESSION),
                Epoch(1),
                CollabMessage::Commit(commit),
            ),
        },
        &mut host,
    );

    // The guest edit lost: the document rolls back to the owner version, but
    // the dropped intent is stashed and named in the conflict projection.
    assert_eq!(
        canonical_document_hash(&host.editor_state().doc).unwrap(),
        canonical_document_hash(&document_named("Owner intent")).unwrap()
    );
    assert_eq!(
        host.editor_state().editor_ui.collab.notice.unwrap().kind,
        CollabNoticeKind::EditConflictDiscarded
    );
    let discarded = host
        .editor_state()
        .editor_ui
        .collab
        .discarded_edit
        .clone()
        .expect("conflict stashes a replayable edit");
    assert!(discarded.fields.iter().any(|field| field == "name"));

    // While a gesture owns the edit lane, reapply refuses without
    // consuming the stash so the user can retry later.
    runtime.transaction_active = true;
    host.editor_state_mut().editor_ui.collab.pending_action =
        Some(op_editor_core::CollabUiAction::ReapplyDiscarded);
    assert!(runtime.drain_ui_action(&mut host));
    assert!(runtime.discarded_property_edit.is_some());
    assert!(host
        .editor_state()
        .editor_ui
        .collab
        .discarded_edit
        .is_some());
    runtime.transaction_active = false;

    // The user asks to reapply: the stash resubmits as a fresh local edit.
    host.editor_state_mut().editor_ui.collab.pending_action =
        Some(op_editor_core::CollabUiAction::ReapplyDiscarded);
    assert!(runtime.drain_ui_action(&mut host));
    assert!(host
        .editor_state()
        .editor_ui
        .collab
        .discarded_edit
        .is_none());
    assert_eq!(
        canonical_document_hash(&host.editor_state().doc).unwrap(),
        canonical_document_hash(&document_named("Guest intent")).unwrap()
    );
    assert_eq!(
        host.editor_state().editor_ui.collab.pending_edit,
        CollabPendingEditUi::Submitting
    );
    let mut saw_submit = false;
    while let Ok(command) = commands.try_recv() {
        if let GuestNetworkCommand::Send { frame, .. } = command {
            if matches!(frame.decode_for_test().body(), CollabMessage::Submit(_)) {
                saw_submit = true;
            }
        }
    }
    assert!(saw_submit, "reapply resubmits the stashed edit");
}
