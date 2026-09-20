//! Typed failures for the one-time legacy `.op` rewrite
//! (`legacy_op_upgrade.rs`): the staging copy in the current schema and its
//! publication over the original or as a numbered sibling.
//!
//! Style follows `op_orchestrator::OrchestratorError`: a plain enum plus a
//! hand-written `Display`, no `thiserror` and no new dependency. Every
//! variant carries STRUCTURED fields and `Display` re-formats the sentence,
//! so the text stays byte-identical — `persistence.rs` routes these into the
//! native open-failure dialog, and `legacy_op_upgrade`'s own tests assert on
//! the "changed while it was opening" wording.
//!
//! The classification the enum adds is the one the flow actually branches on:
//! a REFUSAL (the source moved under the reader — consent no longer applies)
//! versus a mechanical staging/publication FAILURE. Both used to be the same
//! `String` and the caller could only tell them apart by reading the prose.
//!
//! This module is the sibling error for `legacy_op_upgrade.rs` rather than a
//! directory module because that file has no `legacy_op_upgrade/` directory —
//! same convention as `op-cli`'s flat `cli_error.rs` / `skill_install_error.rs`.
//!
//! Two seams still carry `String` payloads: the message from
//! `op_host_services::doc_io::{copy_document_to_current_schema_path,
//! commit_staged_document}` and from `std::io::Error`, neither of which this
//! pass owns. They are adapted with `e.to_string()` so the bridge survives if
//! those upstreams later type their own errors.

use std::fmt;
use std::path::PathBuf;

use crate::figma_import_session::OutputStateError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LegacyUpgradeError {
    /// The source file changed between the load that produced the report and
    /// the moment the rewrite was about to run. The user's consent applied to
    /// the bytes they opened, so the migration is refused outright rather than
    /// downgraded to a numbered copy.
    SourceChangedWhileOpening { source_path: PathBuf },
    /// No unused hidden staging name was free beside the source.
    StagingNamesExhausted { source_path: PathBuf },
    /// A candidate staging path could not be probed for existence.
    StagingProbe { path: PathBuf, message: String },
    /// Writing the current-schema copy into the staging file failed. The
    /// staging file is removed by the caller.
    WriteStaged(String),
    /// Replacing the original with the staged copy failed.
    CommitOverOriginal(String),
    /// Hard-linking the staged copy to a numbered sibling failed.
    PublishNumbered { path: PathBuf, message: String },
    /// 10,000 `Name (N).op` candidates were all taken.
    OutputNamesExhausted { source_path: PathBuf },
    /// The source-identity guard could not read the file's state. Carries
    /// [`OutputStateError`], the leaf shared with the Figma import session
    /// whose `capture_output_state` this flow reuses.
    OutputState(OutputStateError),
}

impl fmt::Display for LegacyUpgradeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LegacyUpgradeError::SourceChangedWhileOpening { source_path } => write!(
                f,
                "{} changed while it was opening; reload it before upgrading",
                source_path.display()
            ),
            LegacyUpgradeError::StagingNamesExhausted { source_path } => write!(
                f,
                "could not allocate an OP upgrade staging file beside {}",
                source_path.display()
            ),
            LegacyUpgradeError::StagingProbe { path, message } => write!(
                f,
                "could not inspect OP upgrade staging path {}: {message}",
                path.display()
            ),
            LegacyUpgradeError::WriteStaged(message)
            | LegacyUpgradeError::CommitOverOriginal(message) => f.write_str(message),
            LegacyUpgradeError::PublishNumbered { path, message } => write!(
                f,
                "could not publish numbered OpenPencil file {}: {message}",
                path.display()
            ),
            LegacyUpgradeError::OutputNamesExhausted { source_path } => write!(
                f,
                "could not find an unused OP file name beside {}",
                source_path.display()
            ),
            LegacyUpgradeError::OutputState(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for LegacyUpgradeError {}

/// Lets every `capture_output_state` call in the upgrade flow use a plain `?`.
impl From<OutputStateError> for LegacyUpgradeError {
    fn from(error: OutputStateError) -> LegacyUpgradeError {
        LegacyUpgradeError::OutputState(error)
    }
}
