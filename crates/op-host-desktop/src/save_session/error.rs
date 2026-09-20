//! Typed failure for the background `.op` save worker (`save_session.rs`).
//!
//! Style follows `op_orchestrator::OrchestratorError`: a plain enum plus a
//! hand-written `Display`, no `thiserror` and no new dependency. `Display` is
//! transparent for the worker's own message, so the `[save] <path>: <error>`
//! stderr line and the native save-failure dialog
//! (`persistence::show_error_dialog_public`) are unchanged byte for byte.
//!
//! What the enum adds is the split the UI thread could not previously make:
//! [`SaveError::WorkerStopped`] means the disk was never touched (the worker
//! thread would not spawn, or died before reporting), whereas
//! [`SaveError::Write`] means a real write was attempted and failed. Both
//! arrived as the same `String` on the completion channel, so the ack path
//! had to treat "we never tried" and "the disk rejected it" identically.
//!
//! The write message itself is still text: it comes from
//! `op_host_services::doc_io::{save_snapshot_to_path,
//! copy_clean_document_with_editor_meta_to_path}`, in a crate this pass does
//! not own. It is adapted with `e.to_string()` so the bridge keeps compiling
//! if `doc_io` later types its own error.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SaveError {
    /// The document serializer or the clean-copy path refused. Carries the
    /// `doc_io` message verbatim.
    Write(String),
    /// The save worker never reported: it failed to spawn, or it panicked
    /// before sending. Nothing was written under the requested path.
    WorkerStopped,
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveError::Write(message) => f.write_str(message),
            SaveError::WorkerStopped => {
                f.write_str("background save worker stopped before reporting a result")
            }
        }
    }
}

impl std::error::Error for SaveError {}
