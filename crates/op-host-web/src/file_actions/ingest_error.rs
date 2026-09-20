//! Typed failures for the browser document-ingest paths in
//! [`crate::file_actions`].
//!
//! Split into its own sibling module because `file_actions.rs` already sits
//! close to the repository's 800-line ceiling. Every `Display` arm reproduces
//! the ad-hoc `String` message it replaced byte for byte, so the
//! `[open] …` / `[import-figma] …` console lines read exactly the same.

/// A source document that could not be turned into an `EditorState`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentIngestError {
    /// The canonical `.op` / `.pen` loader rejected the source.
    LoadCanonical(String),
    /// The Figma `.fig` binary parser rejected the bytes.
    FigmaParse(String),
    /// The isolated Figma Worker's warning array is not valid JSON.
    WorkerWarningsParse(String),
    /// The HTML project importer rejected the file set.
    HtmlProject(String),
    /// The HTML project parsed but produced no nodes.
    HtmlProjectEmpty,
}

impl std::fmt::Display for DocumentIngestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DocumentIngestError::LoadCanonical(error) => write!(f, "{error}"),
            DocumentIngestError::FigmaParse(error) => write!(f, "{error}"),
            DocumentIngestError::WorkerWarningsParse(error) => {
                write!(f, "decode Figma Worker warnings failed: {error}")
            }
            DocumentIngestError::HtmlProject(error) => write!(f, "{error}"),
            DocumentIngestError::HtmlProjectEmpty => {
                write!(f, "HTML project contains no importable content")
            }
        }
    }
}

impl std::error::Error for DocumentIngestError {}

impl From<DocumentIngestError> for String {
    fn from(error: DocumentIngestError) -> String {
        error.to_string()
    }
}
