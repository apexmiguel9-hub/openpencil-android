//! Collaboration notice kinds and the discarded-edit display projection.
//!
//! Split out of `collab_ui_state` at the 800-line cap; that module re-exports
//! everything here so existing `collab_ui_state::*` import paths stay stable.

use crate::CollabConnectErrorUi;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollabRejectUiCode {
    StaleBase,
    ReadOnly,
    Unsupported,
    Conflict,
    ResourceLimit,
    Authentication,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollabNoticeKind {
    Connect(CollabConnectErrorUi),
    DisconnectedReadOnly,
    TicketExpired,
    OwnerLeft,
    EpochChanged,
    Reject(CollabRejectUiCode),
    /// A pending local edit lost a concurrency conflict and was rolled back,
    /// with the dropped intent stashed for replay. Distinct from
    /// `Reject(Conflict)` so the notice detail (node/fields) is only ever
    /// attached to the cancellation that produced the stash.
    EditConflictDiscarded,
    /// A server-authoritative deployment replaced this tab's document with the
    /// authoritative one, and the local copy that was overwritten has been
    /// stashed so the user can put it back. Distinct from
    /// `EditConflictDiscarded`, which is a session rejecting one edit: this is
    /// a whole-document accept with nothing rejected.
    LocalEditPreserved,
    UndoConflict,
    UnsupportedEdit(crate::collab_gate::CollabUnsupportedFeature),
}

impl CollabNoticeKind {
    pub const fn i18n_key(self) -> &'static str {
        match self {
            Self::Connect(error) => error.i18n_key(),
            Self::DisconnectedReadOnly => "collab.status.disconnectedReadOnly",
            Self::TicketExpired => "collab.status.ticketExpired",
            Self::OwnerLeft => "collab.status.ownerLeft",
            Self::EpochChanged => "collab.status.epochChanged",
            Self::Reject(CollabRejectUiCode::StaleBase) => "collab.reject.staleBase",
            Self::Reject(CollabRejectUiCode::ReadOnly) => "collab.reject.readOnly",
            Self::Reject(CollabRejectUiCode::Unsupported) => "collab.reject.unsupported",
            Self::Reject(CollabRejectUiCode::Conflict) => "collab.reject.conflict",
            Self::Reject(CollabRejectUiCode::ResourceLimit) => "collab.reject.resourceLimit",
            Self::Reject(CollabRejectUiCode::Authentication) => "collab.reject.authentication",
            Self::Reject(CollabRejectUiCode::Unknown) => "collab.reject.unknown",
            Self::EditConflictDiscarded => "collab.reject.conflict",
            Self::LocalEditPreserved => "collab.status.localEditPreserved",
            Self::UndoConflict => "collab.status.undoConflict",
            Self::UnsupportedEdit(_) => "collab.status.unsupportedEdit",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollabNotice {
    pub kind: CollabNoticeKind,
    pub created_at_ms: u64,
}

/// Display bound for the conflict-detail node label.
pub const MAX_COLLAB_DISCARDED_NODE_LABEL_CHARS: usize = 40;
/// Display bound for the number of conflicting field names shown.
pub const MAX_COLLAB_DISCARDED_FIELDS: usize = 8;

/// Display projection of one local edit that lost a collaboration conflict
/// and was rolled back. The raw replayable changes stay in the host runtime;
/// this carries only what the notice and the Reapply action need to paint.
#[derive(Clone, PartialEq, Eq)]
pub struct CollabDiscardedEditUi {
    pub node_label: String,
    /// Wire names of the dropped fields, deduplicated and bounded.
    pub fields: Vec<String>,
}

impl std::fmt::Debug for CollabDiscardedEditUi {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The node label is authored document content; keep it out of debug
        // dumps like the rest of the collaboration display projection. Field
        // names are fixed schema tokens and stay visible for diagnosis.
        formatter
            .debug_struct("CollabDiscardedEditUi")
            .field("node_label", &"[REDACTED]")
            .field("fields", &self.fields)
            .finish()
    }
}

impl CollabDiscardedEditUi {
    pub fn bounded(
        node_label: impl Into<String>,
        fields: impl IntoIterator<Item = String>,
    ) -> Self {
        let node_label: String = node_label
            .into()
            .trim()
            .chars()
            .filter(|character| !character.is_control())
            .take(MAX_COLLAB_DISCARDED_NODE_LABEL_CHARS)
            .collect();
        let mut bounded = Vec::new();
        for field in fields {
            if field.is_empty() || bounded.iter().any(|existing| existing == &field) {
                continue;
            }
            bounded.push(field);
            if bounded.len() == MAX_COLLAB_DISCARDED_FIELDS {
                break;
            }
        }
        Self {
            node_label,
            fields: bounded,
        }
    }
}
