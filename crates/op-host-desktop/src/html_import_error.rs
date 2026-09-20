//! Typed failures for the background HTML / ZIP project import
//! (`html_import_session.rs`).
//!
//! Style follows `op_orchestrator::OrchestratorError`: a plain enum plus a
//! hand-written `Display`, no `thiserror` and no new dependency. `Display`
//! reproduces the exact sentence the stringly code produced, so the
//! `[import-html] …` stderr line and the native open-failure dialog
//! (`persistence::show_error_dialog_public`) are unchanged byte for byte.
//!
//! The session deliberately reuses the Figma session's `PreparedImport` /
//! `PumpOutcome`, but NOT its error: the failure sets is different. HTML
//! import has no page-selection stage, no cancellation token, and no adjacent
//! `.op` publication (its next Save routes through Save As), while it does
//! have a case the Figma path cannot produce —
//! [`HtmlImportError::NoImportableContent`], a parse that succeeded and
//! yielded nothing paintable. Sharing one enum would have meant a union with
//! variants unreachable from either side.
//!
//! Both message-carrying variants adapt crates this pass does not own
//! (`std::io`, `op_html`) with `e.to_string()`, so they survive those
//! upstreams later typing their own errors.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HtmlImportError {
    /// The worker thread would not spawn. Delivered through the normal pump
    /// path so the progress overlay still clears.
    WorkerSpawn(String),
    /// Opening or reading the `.html` / `.zip` source failed.
    ReadSource(String),
    /// `op_html` refused the archive or the document. Its message is carried
    /// verbatim.
    Import(String),
    /// The import produced a document with no children. Carries EVERY
    /// warning the importer recorded, newline-joined, since the reason is
    /// often spread across several of them (an unreachable stylesheet AND an
    /// empty body); `None` when it recorded none, giving the generic sentence
    /// below. Reporting only the first hid the rest from the dialog.
    NoImportableContent(Option<String>),
}

impl fmt::Display for HtmlImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HtmlImportError::WorkerSpawn(message) => {
                write!(f, "import worker failed to start: {message}")
            }
            HtmlImportError::ReadSource(message) | HtmlImportError::Import(message) => {
                f.write_str(message)
            }
            HtmlImportError::NoImportableContent(Some(warning)) => f.write_str(warning),
            HtmlImportError::NoImportableContent(None) => f.write_str("no importable content"),
        }
    }
}

impl std::error::Error for HtmlImportError {}
