//! The single-slot recovery cache behind an online auto-accept.
//!
//! A server-authoritative deployment resolves a sync conflict by accepting the
//! daemon's document (see `live_sync_conflict`). That is the right call — the
//! daemon is the sole sequencer and the latch would otherwise never lift — but
//! it overwrites whatever this tab had not yet pushed, and until now that copy
//! was simply gone.
//!
//! So the accept stashes it first. One slot, overwritten by the next conflict:
//! the value of a stash decays fast (the user is looking at a newer document
//! every second), and an unbounded history of documents is exactly the kind of
//! memory growth a wasm heap never gives back.
//!
//! The stash is reachable through the collaboration panel's existing
//! "reapply discarded edit" control — `collab_sync::drain_pending_action`
//! intercepts that action and restores from here rather than posting it to a
//! daemon that has no session to replay it into.

use std::cell::RefCell;

use jian_ops_schema::PenDocument;

/// What was overwritten, and when.
pub(crate) struct StashedDocument {
    pub document: PenDocument,
    /// Wall clock at the moment of the accept, for the panel's label.
    pub stashed_at_ms: u64,
}

thread_local! {
    static STASH: RefCell<Option<StashedDocument>> = const { RefCell::new(None) };
}

/// Keep `document` as the recoverable copy, replacing any previous stash.
pub(crate) fn stash(document: PenDocument, stashed_at_ms: u64) {
    STASH.with(|slot| {
        *slot.borrow_mut() = Some(StashedDocument {
            document,
            stashed_at_ms,
        });
    });
}

/// Whether a recoverable copy exists.
pub(crate) fn has_stash() -> bool {
    STASH.with(|slot| slot.borrow().is_some())
}

/// Take the recoverable copy, emptying the slot.
pub(crate) fn take() -> Option<StashedDocument> {
    STASH.with(|slot| slot.borrow_mut().take())
}

/// Drop the stash without using it — an identity change, where the previous
/// account's document must not be restorable in the new account's tab.
pub(crate) fn clear() {
    STASH.with(|slot| *slot.borrow_mut() = None);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(version: &str) -> PenDocument {
        let mut doc = op_editor_core::EditorState::starter().doc;
        doc.version = version.to_string();
        doc
    }

    #[test]
    fn a_stash_round_trips_and_empties_the_slot() {
        clear();
        assert!(!has_stash());
        stash(document("local-1"), 1_000);
        assert!(has_stash());

        let taken = take().expect("stashed");
        assert_eq!(taken.document.version, "local-1");
        assert_eq!(taken.stashed_at_ms, 1_000);
        assert!(!has_stash(), "taking it empties the slot");
    }

    #[test]
    fn a_second_conflict_replaces_the_first_stash() {
        // One slot on purpose: the value of a stash decays fast, and an
        // unbounded history is memory a wasm heap never gives back.
        clear();
        stash(document("local-1"), 1_000);
        stash(document("local-2"), 2_000);
        let taken = take().expect("stashed");
        assert_eq!(taken.document.version, "local-2");
        assert_eq!(taken.stashed_at_ms, 2_000);
        assert!(!has_stash(), "only ever one");
    }

    #[test]
    fn clearing_makes_the_previous_document_unrecoverable() {
        // Used on an identity change: account B must not be able to restore
        // account A's document into its own tab.
        clear();
        stash(document("local-1"), 1_000);
        clear();
        assert!(!has_stash());
        assert!(take().is_none());
    }
}

#[cfg(test)]
mod call_site_tests {
    /// The stash was previously defined but never called — a dead safety net.
    /// This pins that the auto-accept path actually references it, so a future
    /// refactor that drops the call fails here rather than silently.
    #[test]
    fn the_auto_accept_path_stashes_before_resolving() {
        let glue = include_str!("live_sync_glue.rs");
        let resolve_at = glue
            .find("resolve_accept_remote")
            .expect("the auto-accept path exists");
        let stash_at = glue
            .find("preserve_local_document")
            .expect("the auto-accept path must stash the local document first");
        assert!(
            stash_at < resolve_at,
            "the stash must run while the local document is still on screen"
        );
    }
}

#[cfg(test)]
mod skip_without_backup_tests {
    /// A failed stash must abort the accept for this tick, not proceed
    /// unprotected: overwriting unpushed work with no way back is the exact
    /// loss this path exists to prevent, and the latch simply retries.
    #[test]
    fn the_accept_is_skipped_when_the_document_cannot_be_stashed() {
        let glue = include_str!("live_sync_glue.rs");
        let guarded = glue
            .find("if !live_sync_conflict::preserve_local_document(inner,")
            .expect("the accept must be guarded on a successful stash");
        let resolve = glue
            .find("resolve_accept_remote")
            .expect("the accept exists");
        assert!(guarded < resolve, "the guard must precede the accept");
    }
}
