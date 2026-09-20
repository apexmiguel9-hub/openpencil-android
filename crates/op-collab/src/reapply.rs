//! Host-driven replay of a cancelled optimistic property edit.
//!
//! When a guest's pending edit loses to authoritative history the session
//! rolls the displayed document back and reports the dropped intent through
//! [`crate::GuestEffect::PendingCancelled`]. Hosts may stash those property
//! changes and, on an explicit user request, reassert them over the current
//! document with [`reapply_property_changes`] before submitting the result as
//! a brand-new local edit.

use std::collections::BTreeMap;

use jian_ops_schema::PenDocument;

use crate::{
    apply_txn, canonical_node_hash, diff_fields::apply_changes_to_node, diff_index::TreeIndex,
    ApplyContext, CanonicalHashError, CollabApplyError, CollabOp, CollabTxn, DiffError,
    NodeFieldChange,
};

/// Failure to re-apply a previously discarded property edit.
#[derive(Debug, thiserror::Error)]
pub enum ReapplyError {
    #[error("node `{node_id}` no longer exists in the current document")]
    NodeMissing { node_id: String },
    #[error("discarded changes are not a supported property edit: {0}")]
    Unsupported(#[from] DiffError),
    #[error("replaying the discarded property edit failed: {0}")]
    Apply(#[from] CollabApplyError),
    #[error("hashing the replay target failed: {0}")]
    Hash(#[from] CanonicalHashError),
}

/// Re-apply the desired values of a discarded property edit onto `document`.
///
/// Unlike the automatic rebase, this deliberately ignores each change's
/// `before` value: the caller is executing an explicit user request to
/// reassert the dropped intent over whatever the fields hold now. The target
/// node is looked up by id wherever it currently lives, so a page move does
/// not block the replay. Fails if any target node no longer exists.
pub fn reapply_property_changes(
    document: &PenDocument,
    changes: &[NodeFieldChange],
) -> Result<PenDocument, ReapplyError> {
    let mut grouped: BTreeMap<&str, Vec<NodeFieldChange>> = BTreeMap::new();
    for change in changes {
        grouped
            .entry(change.node_id.as_str())
            .or_default()
            .push(change.clone());
    }
    let mut desired = document.clone();
    for (node_id, changes) in grouped {
        let index = TreeIndex::build(&desired)?;
        let entry = index
            .entry(node_id)
            .ok_or_else(|| ReapplyError::NodeMissing {
                node_id: node_id.to_owned(),
            })?;
        let replacement = apply_changes_to_node(entry.node, &changes)?;
        let operation = CollabOp::ReplaceExact {
            page: entry.page.clone(),
            node_id: node_id.to_owned(),
            expected_hash: canonical_node_hash(entry.node)?,
            node: replacement,
        };
        desired = apply_txn(
            &desired,
            &CollabTxn::new(vec![operation]),
            &ApplyContext::standalone_trusted(),
        )?;
    }
    Ok(desired)
}
