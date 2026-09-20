//! Active-page background resolution for the editor canvas.

use crate::util::parse_hex_color;
use crate::Color;
use op_editor_core::EditorState;

/// Resolve the active page's authored background. The boolean reports
/// whether the normal editor grid remains visible.
pub(super) fn resolve(state: &EditorState, fallback: Color) -> (Color, bool) {
    let Some(pages) = state.doc.pages.as_ref().filter(|pages| !pages.is_empty()) else {
        return (fallback, true);
    };
    let page = &pages[state.ui.active_page_index.min(pages.len() - 1)];
    match page.background_color.as_deref().and_then(parse_hex_color) {
        Some(color) => (color, false),
        None => (fallback, true),
    }
}
