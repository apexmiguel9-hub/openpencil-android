//! The session push gate: which local document may reach the daemon.
//!
//! Split out of `collab_sync.rs` at the 800-line cap. It is one decision, and
//! the decision is load-bearing: too strict and every node created during a
//! session is stuck forever (and deleted by the next pull); too loose and this
//! peer forks the shared document with ids another peer may also mint.

use op_editor_core::CollabConnectionPhase;

use super::{document_node_ids, DAEMON_NODE_IDS, SESSION_NAMESPACE};

/// Whether a live session forbids pushing the current local document.
///
/// The browser has no owner-assigned id namespace: it mints `n<counter>` from a
/// local sequential allocator, and two peers creating a node in the same moment
/// would mint the same id. The collaboration protocol replays those ids
/// verbatim, so a colliding pair silently forks the document — the failure this
/// refuses to produce.
///
/// The check is deliberately whole-document rather than per-gesture: draw,
/// duplicate, paste, group and import all mint through the same counter, and
/// gating the single push covers every one of them without a guard at each
/// call site. Edits to existing nodes (move, restyle, delete) carry no new ids
/// and go through untouched.
///
/// ## Which unknown ids are allowed
///
/// An id the daemon has not seen is only dangerous when this peer had no
/// right to invent it. Once the owner grants a namespace and the allocator is
/// installed, `c_<namespace>_<counter>` ids ARE this peer's to mint — the
/// namespace is what makes them collision-free — so blocking them would mean
/// every node created during a session is stuck forever, and the next pull
/// would quietly delete it. So the gate refuses an id only when it is both
/// unknown to the daemon AND outside this peer's namespace.
///
/// With no namespace (an older daemon, or a session that has not reached
/// `Active`) the allocator is not installed and every unknown id is refused,
/// exactly as before: the browser is minting bare `n<counter>` ids that could
/// collide with a peer's.
///
/// The local node stays on screen until the next pull replaces it with the
/// daemon's document, so the divergence is bounded and self-healing.
pub(crate) fn push_blocked_by_session(state: &op_editor_core::EditorState) -> bool {
    if state.editor_ui.collab.phase != CollabConnectionPhase::Active {
        return false;
    }
    DAEMON_NODE_IDS.with(|known| {
        let known = known.borrow();
        let Some(known) = known.as_ref() else {
            // No daemon document seen yet in this session; refuse rather than
            // guess, since the pull that would settle it is one tick away.
            return true;
        };
        SESSION_NAMESPACE.with(|namespace| {
            let namespace = namespace.borrow();
            document_node_ids(state)
                .iter()
                .any(|id| !known.contains(id) && !minted_by_this_peer(id, namespace.as_ref()))
        })
    })
}

/// Whether `id` is one this peer was entitled to mint.
///
/// Parsed through the shared `op-util` grammar rather than a hand-rolled
/// prefix test: `c_`/`_` are structural, a namespace has its own character
/// rules, and a substring check would accept `c_teamA-evil_7` as belonging to
/// `teamA`. Comparing parsed namespaces is what makes that impossible.
fn minted_by_this_peer(
    id: &op_editor_core::NodeId,
    namespace: Option<&op_editor_core::PeerNamespace>,
) -> bool {
    let Some(namespace) = namespace else {
        return false;
    };
    op_util::collab_id::NamespacedId::parse(id.as_str())
        .is_ok_and(|parsed| parsed.namespace() == namespace)
}

#[cfg(test)]
mod tests {
    use super::super::note_daemon_document;
    use super::super::tests::{doc_with, namespace, reset_latches};
    use super::*;

    #[test]
    fn an_active_session_blocks_an_id_this_peer_had_no_right_to_invent() {
        reset_latches();
        let mut state = doc_with(&["n100"]);
        note_daemon_document(&state);
        state
            .editor_ui
            .collab
            .set_phase(CollabConnectionPhase::Active);

        // Editing what the daemon already knows is fine.
        assert!(!push_blocked_by_session(&state));

        // Minting a bare local id is not: with no owner-assigned namespace
        // installed, this id could collide with a peer's.
        let mut grown = doc_with(&["n100", "n101"]);
        grown
            .editor_ui
            .collab
            .set_phase(CollabConnectionPhase::Active);
        assert!(push_blocked_by_session(&grown));
    }

    #[test]
    fn a_namespaced_id_this_peer_minted_is_pushed() {
        // The regression this replaces: every node created during a session
        // was blocked forever, because an id the peer had just been granted
        // the right to mint is by definition not in the daemon's snapshot.
        reset_latches();
        let seed = doc_with(&["n100"]);
        note_daemon_document(&seed);
        SESSION_NAMESPACE.with(|slot| *slot.borrow_mut() = Some(namespace("teamA")));

        for created in [
            // create, duplicate and paste all mint through the same counter.
            vec!["n100", "c_teamA_1"],
            vec!["n100", "c_teamA_1", "c_teamA_2"],
            vec!["n100", "c_teamA_4294967296"],
        ] {
            let mut state = doc_with(&created);
            state
                .editor_ui
                .collab
                .set_phase(CollabConnectionPhase::Active);
            assert!(
                !push_blocked_by_session(&state),
                "a node this peer was entitled to create must reach the daemon: {created:?}"
            );
        }
    }

    #[test]
    fn an_id_from_another_peers_namespace_is_still_blocked() {
        reset_latches();
        let seed = doc_with(&["n100"]);
        note_daemon_document(&seed);
        SESSION_NAMESPACE.with(|slot| *slot.borrow_mut() = Some(namespace("teamA")));

        for hostile in [
            "c_teamB_1",      // another peer's namespace
            "c_teamA-evil_1", // a substring match a prefix test would accept
            "c_teamAevil_1",  // ditto, no separator
            "n101",           // a bare local id
            "c_teamA",        // no counter
            "teamA_1",        // no prefix
        ] {
            let mut state = doc_with(&["n100", hostile]);
            state
                .editor_ui
                .collab
                .set_phase(CollabConnectionPhase::Active);
            assert!(
                push_blocked_by_session(&state),
                "{hostile} is not this peer's to mint"
            );
        }
    }

    #[test]
    fn a_namespaced_id_is_blocked_when_no_allocator_is_installed() {
        // Fail-closed: without the allocator the peer is minting bare ids, so
        // a namespaced-looking id in the document was not minted here.
        reset_latches();
        let seed = doc_with(&["n100"]);
        note_daemon_document(&seed);

        let mut state = doc_with(&["n100", "c_teamA_1"]);
        state
            .editor_ui
            .collab
            .set_phase(CollabConnectionPhase::Active);
        assert!(push_blocked_by_session(&state));
    }
}
