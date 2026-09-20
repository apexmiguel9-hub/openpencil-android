//! Conflict-discarded edit handling: notice mapping, the replayable stash,
//! and its bounded display projection. Split off `runtime/effects.rs`
//! at the 800-line cap; pure code motion.

use op_collab::{EditChanges, NodeFieldChange, PendingCancelReason, RejectCode};
use op_editor_core::{CollabDiscardedEditUi, CollabNoticeKind, CollabRejectUiCode};

use super::CollabRuntime;
use crate::host::CollabHost;

impl CollabRuntime {
    /// A pending local edit was rolled back. `AlreadySatisfied` means the
    /// authoritative history already contains the same values — nothing was
    /// lost, so no toast. A genuine concurrency loss (property conflict or an
    /// owner-side precondition failure) stashes the dropped property intent
    /// for the panel's Reapply action and names it in the toast; every other
    /// rejection maps to its own notice and clears any older stash, so a
    /// policy or size rejection never offers to resubmit a forbidden edit.
    pub(super) fn observe_pending_cancelled(
        &mut self,
        reason: PendingCancelReason,
        changes: EditChanges,
        host: &mut impl CollabHost,
    ) {
        let concurrency_loss = matches!(
            reason,
            PendingCancelReason::PropertyConflict { .. }
                | PendingCancelReason::StructuralConflict
                | PendingCancelReason::Rejected(RejectCode::PreconditionFailed)
        );
        let notice = match reason {
            PendingCancelReason::AlreadySatisfied => return,
            PendingCancelReason::PropertyConflict { .. }
            | PendingCancelReason::StructuralConflict
            | PendingCancelReason::Rejected(RejectCode::PreconditionFailed) => {
                CollabNoticeKind::Reject(CollabRejectUiCode::Conflict)
            }
            PendingCancelReason::Rejected(RejectCode::StaleBase) => {
                CollabNoticeKind::Reject(CollabRejectUiCode::StaleBase)
            }
            PendingCancelReason::Rejected(RejectCode::PermissionDenied) => {
                CollabNoticeKind::Reject(CollabRejectUiCode::ReadOnly)
            }
            PendingCancelReason::Rejected(
                RejectCode::UnsupportedEdit | RejectCode::InvalidOperation,
            ) => CollabNoticeKind::Reject(CollabRejectUiCode::Unsupported),
            PendingCancelReason::Rejected(RejectCode::ResourceLimit) => {
                CollabNoticeKind::Reject(CollabRejectUiCode::ResourceLimit)
            }
            PendingCancelReason::Rejected(
                RejectCode::ExpiredClientOpId | RejectCode::CounterGap | RejectCode::SessionChanged,
            ) => CollabNoticeKind::Reject(CollabRejectUiCode::Unknown),
        };
        match changes {
            EditChanges::Property(changes) if concurrency_loss && !changes.is_empty() => {
                let projection = discarded_edit_projection(&changes, &host.editor_state().doc);
                self.discarded_property_edit = Some(changes);
                host.editor_state_mut().editor_ui.collab.discarded_edit = Some(projection);
                // The stash-bearing notice kind is the only one whose text
                // names the discarded node/fields.
                self.set_notice(host, CollabNoticeKind::EditConflictDiscarded);
                return;
            }
            _ => self.clear_discarded_stash(host),
        }
        self.set_notice(host, notice);
    }

    /// Drop the replayable stash together with its display projection so the
    /// two can never diverge.
    pub(super) fn clear_discarded_stash(&mut self, host: &mut impl CollabHost) {
        self.discarded_property_edit = None;
        host.editor_state_mut().editor_ui.collab.discarded_edit = None;
    }
}

/// Bounded display projection of the dropped property changes: the layer
/// labels of every distinct target node (in change order) plus the
/// deduplicated field names, so a multi-node edit never attributes one
/// node's fields to another.
fn discarded_edit_projection(
    changes: &[NodeFieldChange],
    doc: &jian_ops_schema::PenDocument,
) -> CollabDiscardedEditUi {
    let mut node_ids: Vec<&str> = Vec::new();
    for change in changes {
        if !node_ids.contains(&change.node_id.as_str()) {
            node_ids.push(change.node_id.as_str());
        }
    }
    let labels = node_ids
        .iter()
        .map(|node_id| node_display_label(doc, node_id))
        .collect::<Vec<_>>()
        .join(", ");
    CollabDiscardedEditUi::bounded(
        labels,
        changes
            .iter()
            .map(|change| change.field.wire_name().to_string()),
    )
}

/// Layer-panel label rules: authored node name, else the id as a last resort.
fn node_display_label(doc: &jian_ops_schema::PenDocument, node_id: &str) -> String {
    use op_editor_core::PenNodeExt as _;

    let id = op_editor_core::NodeId::new(node_id);
    let node = doc
        .pages
        .as_ref()
        .into_iter()
        .flatten()
        .find_map(|page| op_editor_core::walkers::find_node(&page.children, &id))
        .or_else(|| op_editor_core::walkers::find_node(&doc.children, &id));
    node.and_then(|node| node.base().name.clone())
        .unwrap_or_else(|| node_id.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discarded_projection_labels_every_distinct_node() {
        let doc: jian_ops_schema::PenDocument = serde_json::from_value(serde_json::json!({
            "version": "1.0",
            "children": [
                {"type": "rectangle", "id": "a", "name": "Alpha", "x": 0, "y": 0},
                {"type": "rectangle", "id": "b", "x": 0, "y": 0}
            ]
        }))
        .unwrap();
        let change = |node_id: &str, field: op_collab::SupportedNodeField| NodeFieldChange {
            page: op_collab::PageRef::DocumentRoot,
            node_id: node_id.to_owned(),
            field,
            before: op_collab::FieldValue::Missing,
            desired: op_collab::FieldValue::Value(serde_json::json!(1.0)),
        };
        let projection = discarded_edit_projection(
            &[
                change("a", op_collab::SupportedNodeField::X),
                change("b", op_collab::SupportedNodeField::Y),
                change("a", op_collab::SupportedNodeField::X),
            ],
            &doc,
        );
        // Named node uses its layer name; a nameless node falls back to id.
        assert_eq!(projection.node_label, "Alpha, b");
        assert_eq!(projection.fields, vec!["x".to_string(), "y".to_string()]);
    }
}
