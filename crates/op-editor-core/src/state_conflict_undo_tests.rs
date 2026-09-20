//! Undo as the recovery path behind a server-authoritative conflict accept.
//!
//! Split out of `state.rs` at the 800-line cap. See the note on
//! `replace_document_clears_stale_draft_state_so_undo_cannot_resurrect_old_doc`
//! for why this path is exempt from that invariant.

use super::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_accepted_remote_document_can_be_undone_back_to_the_local_edit() {
        // The online conflict path: the daemon's document is accepted to lift
        // a latch that would otherwise never clear, overwriting whatever this
        // tab had not yet pushed. Undo is the user-reachable way back, and it
        // must restore the LOCAL edit — not the document that preceded it.
        let mut s = EditorState::new();
        let mut opened = empty_document();
        opened.name = Some("OPENED".to_string());
        s.replace_document(opened);

        // The user edits locally; that edit has not reached the daemon.
        let mut local_edit = empty_document();
        local_edit.name = Some("LOCAL-EDIT".to_string());
        s.replace_document_with_undo(local_edit);
        assert_eq!(s.doc.name.as_deref(), Some("LOCAL-EDIT"));

        // A concurrent writer's document arrives and the conflict auto-resolves
        // by accepting it — the apply the shell makes undoable.
        let mut remote = empty_document();
        remote.name = Some("REMOTE".to_string());
        s.replace_document_with_undo(remote);
        assert_eq!(s.doc.name.as_deref(), Some("REMOTE"));

        // One undo returns the user's own overwritten work…
        assert!(s.history.can_undo());
        assert!(s.undo());
        assert_eq!(
            s.doc.name.as_deref(),
            Some("LOCAL-EDIT"),
            "undo must restore the edit the accept overwrote"
        );
        // …and the document now differs from what the daemon last sent, which
        // is exactly the condition the next push tick acts on.
        assert!(s.redo());
        assert_eq!(s.doc.name.as_deref(), Some("REMOTE"));
    }
}
