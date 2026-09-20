//! Typed failures for the desktop document Open flow (`persistence.rs`).
//!
//! Style follows `op_orchestrator::OrchestratorError`: a plain enum plus a
//! hand-written `Display`, no `thiserror` and no new dependency. Every
//! variant wraps an already-typed error from the stage that produced it and
//! `Display` delegates to it, so the `[open]` / `[open-recent]` stderr lines
//! and the native `rfd` Open-error dialog body are unchanged byte for byte.
//!
//! What the enum adds is the stage attribution `load_into_host` never had.
//! Opening a document is three fallible stages that used to funnel into one
//! `String`: fingerprinting the file BEFORE the read (so a concurrent Figma
//! import cannot publish over it mid-open), parsing it through the canonical
//! loader, and — for a legacy-schema `.op` — the consented in-place rewrite.
//! They fail for entirely different reasons and only the middle one means
//! "this file is not a document we can read", which is what the caller's
//! stale-recent-entry pruning is really trying to detect.
//!
//! Save is deliberately NOT modelled here: it is a single stage that reports
//! `op_host_services::doc_io::DocIoError` directly, so wrapping it would add
//! a layer without adding a distinction.

use std::fmt;

use op_host_services::doc_io::DocIoError;

use crate::figma_import_session::OutputStateError;
use crate::legacy_op_upgrade_error::LegacyUpgradeError;

#[derive(Debug)]
pub(crate) enum DocumentOpenError {
    /// Fingerprinting the file before the read failed, so the open cannot
    /// prove the bytes it is about to parse are the bytes the user picked.
    OutputState(OutputStateError),
    /// The canonical loader refused the file — the "this is not a document we
    /// can read" case, and the only one that justifies pruning a stale recent
    /// entry.
    Load(DocIoError),
    /// The consented legacy-schema rewrite failed. The document itself parsed
    /// fine; what failed is persisting it back at the current schema.
    Upgrade(LegacyUpgradeError),
}

impl fmt::Display for DocumentOpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocumentOpenError::OutputState(error) => error.fmt(f),
            DocumentOpenError::Load(error) => error.fmt(f),
            DocumentOpenError::Upgrade(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for DocumentOpenError {}

impl From<OutputStateError> for DocumentOpenError {
    fn from(error: OutputStateError) -> DocumentOpenError {
        DocumentOpenError::OutputState(error)
    }
}

impl From<DocIoError> for DocumentOpenError {
    fn from(error: DocIoError) -> DocumentOpenError {
        DocumentOpenError::Load(error)
    }
}

impl From<LegacyUpgradeError> for DocumentOpenError {
    fn from(error: LegacyUpgradeError) -> DocumentOpenError {
        DocumentOpenError::Upgrade(error)
    }
}
