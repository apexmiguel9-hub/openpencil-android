// Typed failures for the read-only `Viewer`.
//
// The JS boundary still surfaces a plain string (`JsValue::from_str`), so
// `Display` reproduces the previous ad-hoc `String` messages byte for byte —
// only the Rust-side signatures gained types.

use op_editor_ui::svg_export::SvgExportError;

/// A failed document load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewerLoadError(pub String);

impl std::fmt::Display for ViewerLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ViewerLoadError {}

/// A failed SVG export from the viewer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewerExportError {
    /// `load()` / `rebuild_scene()` has not run yet.
    NoScene,
    /// The scene serializer rejected the active page.
    Svg(SvgExportError),
}

impl std::fmt::Display for ViewerExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViewerExportError::NoScene => {
                write!(f, "no scene — call load() then rebuild_scene() first")
            }
            ViewerExportError::Svg(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ViewerExportError {}

impl From<SvgExportError> for ViewerExportError {
    fn from(error: SvgExportError) -> Self {
        ViewerExportError::Svg(error)
    }
}
