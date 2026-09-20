//! Typed failures for the browser export paths in [`crate::file_actions`].
//!
//! Split into its own sibling module because `file_actions.rs` already sits
//! close to the repository's 800-line ceiling. Every `Display` arm reproduces
//! the ad-hoc `String` message it replaced byte for byte, so the `console`
//! lines the web shell prints — and the tests asserting them — do not move.

use op_editor_ui::svg_export::SvgExportError;

/// A failed Save-As-image / export-request build or a rejected daemon export
/// response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentExportError {
    /// SVG serialization refused the active page or the current selection.
    Svg(SvgExportError),
    /// The canonical document could not be serialized to a JSON value.
    SerializeDocument(String),
    /// The canonical document serialized to something other than an object.
    DocumentNotObject,
    /// The `editorMeta` sidecar could not be serialized.
    SerializeEditorMeta(String),
    /// The export request body could not be serialized.
    SerializeRequest(String),
    /// The UI kit document could not be serialized.
    SerializeKit(String),
    /// The active export format has no raster encoder.
    RasterFormatUnsupported,
    /// A daemon export response is not valid JSON.
    ResponseParse(String),
    /// The daemon reported a failed export (message verbatim from the daemon,
    /// or the format-specific fallback when it sent none).
    Daemon(String),
    /// A successful PDF response carries no `dataBase64` payload.
    PdfMissingData,
    /// The PDF response payload is not valid base64.
    PdfDecode(String),
    /// A successful raster response carries no `dataBase64` payload.
    RasterMissingData,
    /// The raster response payload is not valid base64.
    RasterDecode(String),
}

impl std::fmt::Display for DocumentExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DocumentExportError::Svg(error) => write!(f, "{error}"),
            DocumentExportError::SerializeDocument(error) => write!(f, "{error}"),
            DocumentExportError::DocumentNotObject => {
                write!(f, "canonical document must serialize as an object")
            }
            DocumentExportError::SerializeEditorMeta(error) => write!(f, "{error}"),
            DocumentExportError::SerializeRequest(error) => write!(f, "{error}"),
            DocumentExportError::SerializeKit(error) => write!(f, "{error}"),
            DocumentExportError::RasterFormatUnsupported => {
                write!(f, "raster export requires PNG, JPEG, or WEBP")
            }
            DocumentExportError::ResponseParse(error) => write!(f, "{error}"),
            DocumentExportError::Daemon(message) => write!(f, "{message}"),
            DocumentExportError::PdfMissingData => {
                write!(f, "PDF export response missing dataBase64")
            }
            DocumentExportError::PdfDecode(error) => {
                write!(f, "decode PDF response failed: {error}")
            }
            DocumentExportError::RasterMissingData => {
                write!(f, "Raster export response missing dataBase64")
            }
            DocumentExportError::RasterDecode(error) => {
                write!(f, "decode raster response failed: {error}")
            }
        }
    }
}

impl std::error::Error for DocumentExportError {}

impl From<SvgExportError> for DocumentExportError {
    fn from(error: SvgExportError) -> Self {
        DocumentExportError::Svg(error)
    }
}

impl From<DocumentExportError> for String {
    fn from(error: DocumentExportError) -> String {
        error.to_string()
    }
}
