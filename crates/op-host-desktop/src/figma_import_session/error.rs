//! Typed failures for the two-stage `.fig` import session
//! (`figma_import_session.rs`): the prepare worker, the convert worker, the
//! staging write, and the publication of the generated `.op` beside the
//! source file.
//!
//! Style follows `op_orchestrator::OrchestratorError`: a plain enum plus a
//! hand-written `Display`, no `thiserror` and no new dependency. Most
//! variants carry STRUCTURED fields and `Display` re-formats the sentence,
//! so the text is reproduced byte for byte — these messages reach the native
//! `rfd` error dialog through `persistence::show_error_dialog_public` and the
//! `[import-figma]` stderr lines, and the Save-As fallback body
//! (`"{error}\n\nThe imported design is open but unsaved. …"`) embeds one
//! verbatim.
//!
//! What the enum adds is the classification the session never had: a
//! user-initiated cancellation, a corrupt `.fig`, a worker that would not
//! spawn, and a publication race over the adjacent output file were all
//! indistinguishable `String`s travelling through the same worker channel.
//! They are now separate variants, so the pump can tell "the user cancelled"
//! from "the conversion failed" without matching on prose.
//!
//! Two seams still carry a `String` payload inside a variant rather than a
//! nested typed error, because they come from crates this pass does not own:
//! [`FigmaImportError::ParseFig`] / [`FigmaImportError::Convert`] hold
//! `op_figma`'s message, and the `*_message` fields hold the message from
//! `std::io` or `op_host_services::doc_io`. Each is produced with
//! `e.to_string()` rather than `String::from(e)` so the adapter keeps
//! compiling if those upstreams later grow their own typed errors.

use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FigmaImportError {
    /// The session was cancelled — either by the user dismissing the page
    /// selector or by a newer document-replacing action superseding it. Every
    /// cancellation check in the worker reports this, so the pump can treat
    /// it as "no error dialog" if it ever wants to.
    Cancelled,
    /// Reading the `.fig` bytes off disk failed. Carries `std::io::Error`'s
    /// own message, which is what the dialog used to show.
    ReadSource(String),
    /// `op_figma` refused the binary. Its message is carried verbatim.
    ParseFig(String),
    /// `op_figma` refused to materialise the selected page(s) into a
    /// document. Its message is carried verbatim.
    Convert(String),
    /// The prepare worker thread would not spawn. Reported through the normal
    /// pump path so the overlay flag still clears.
    PrepareWorkerSpawn(String),
    /// The convert worker thread would not spawn — same rationale.
    ConvertWorkerSpawn(String),
    /// The import source path has no file name, so no sibling `.op` name can
    /// be derived from it.
    SourceHasNoFileName,
    /// The import source is itself a `.op`, so the adjacent output would
    /// overwrite the input.
    SourceAlreadyOp,
    /// A candidate hidden staging path could not be probed for existence.
    StagingProbe { path: PathBuf, message: String },
    /// 100 staging-name candidates were all taken — the directory is either
    /// hostile or unwritable.
    StagingNamesExhausted { source_path: PathBuf },
    /// Serializing the converted document into the staging file failed.
    WriteStaged {
        source_path: PathBuf,
        message: String,
    },
    /// Publishing the staged document to its final name failed (hard-link or
    /// atomic replace). The staged file is removed by the caller either way.
    Publish { path: PathBuf, message: String },
    /// 10,000 `Name (N).op` candidates were all taken.
    OutputNamesExhausted { source_path: PathBuf },
    /// The adjacent-output guard could not read the destination's state.
    /// Carries [`OutputStateError`] so the shared leaf keeps one wording.
    OutputState(OutputStateError),
}

impl fmt::Display for FigmaImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FigmaImportError::Cancelled => f.write_str("Figma import was cancelled"),
            FigmaImportError::ReadSource(message)
            | FigmaImportError::ParseFig(message)
            | FigmaImportError::Convert(message) => f.write_str(message),
            FigmaImportError::PrepareWorkerSpawn(message) => {
                write!(f, "import worker failed to start: {message}")
            }
            FigmaImportError::ConvertWorkerSpawn(message) => {
                write!(f, "convert worker failed to start: {message}")
            }
            FigmaImportError::SourceHasNoFileName => {
                f.write_str("Figma import path has no file name")
            }
            FigmaImportError::SourceAlreadyOp => {
                f.write_str("Figma import source already has the .op extension")
            }
            FigmaImportError::StagingProbe { path, message } => write!(
                f,
                "could not inspect import staging path {}: {message}",
                path.display()
            ),
            FigmaImportError::StagingNamesExhausted { source_path } => write!(
                f,
                "could not allocate an import staging file beside {}",
                source_path.display()
            ),
            FigmaImportError::WriteStaged {
                source_path,
                message,
            } => write!(
                f,
                "could not write converted OpenPencil document beside {}: {message}",
                source_path.display()
            ),
            FigmaImportError::Publish { path, message } => write!(
                f,
                "could not publish converted OpenPencil document {}: {message}",
                path.display()
            ),
            FigmaImportError::OutputNamesExhausted { source_path } => write!(
                f,
                "could not find an unused OP file name beside {}",
                source_path.display()
            ),
            FigmaImportError::OutputState(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for FigmaImportError {}

/// Lets every `capture_output_state` call in the session use a plain `?`.
impl From<OutputStateError> for FigmaImportError {
    fn from(error: OutputStateError) -> FigmaImportError {
        FigmaImportError::OutputState(error)
    }
}

/// Typed failure of the adjacent-output guard (`output_guard.rs`).
///
/// This is a deliberately tiny leaf enum rather than a variant of
/// [`FigmaImportError`], because `capture_output_state` has TWO consumers in
/// different failure domains: this session and `legacy_op_upgrade.rs`. Both
/// absorb it through a `From` impl, so the single sentence below is written
/// once and neither domain has to re-format it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OutputStateError {
    /// `std::fs::metadata` failed for a reason other than "not found"
    /// (permissions, a broken mount, a path component that is not a
    /// directory). A missing entry is a valid state, not an error.
    Inspect { path: PathBuf, message: String },
}

impl fmt::Display for OutputStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputStateError::Inspect { path, message } => write!(
                f,
                "could not inspect adjacent OP path {}: {message}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for OutputStateError {}
