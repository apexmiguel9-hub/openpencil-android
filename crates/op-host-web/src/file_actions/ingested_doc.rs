//! The ingest result carried between the browser IO glue and the host.
//!
//! Split out of `file_actions.rs` at the 800-line cap. Besides the document
//! state and the loader's rendered warnings it carries the HTML importer's
//! TYPED degradations, which the post-import diagnostics overlay renders in
//! the viewer's locale — the rendered strings alone cannot be localized.

use op_editor_core::html_import_diagnostics::HtmlImportDiagnostic;
use op_editor_core::EditorState;

/// An ingested document plus the loader's best-effort schema
/// warnings (the desktop logs these to stderr; the web glue routes
/// them to `console.warn`).
pub struct IngestedDoc {
    pub state: EditorState,
    pub warnings: Vec<String>,
    /// Typed HTML-import degradations, for the post-import diagnostics
    /// overlay. Empty for every non-HTML ingest path.
    pub diagnostics: Vec<HtmlImportDiagnostic>,
}

impl IngestedDoc {
    /// Non-HTML ingest: loader warnings only, nothing for the overlay.
    pub fn new(state: EditorState, warnings: Vec<String>) -> Self {
        Self {
            state,
            warnings,
            diagnostics: Vec::new(),
        }
    }

    /// HTML ingest: carry the importer's typed diagnostics through to the
    /// overlay alongside the rendered strings the console already logs.
    pub fn from_html(
        state: EditorState,
        warnings: Vec<String>,
        typed: &[op_html::ImportWarning],
    ) -> Self {
        Self {
            state,
            warnings,
            diagnostics: op_editor_core::html_import_diagnostics::rows_from_parts(
                op_html::diagnostic_parts(typed),
            ),
        }
    }
}
