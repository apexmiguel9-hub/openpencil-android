//! Floating VariablesPanel placement + resize clamping, shared by the
//! native and web widget hosts (their
//! `widget_host/variables_panel_geometry.rs` twins were a verbatim pair).
//! Hosts pass in the canvas band their own `canvas_region` resolved plus
//! their toolbar insets, and keep the active resize edge + `mark_dirty`.

use op_editor_core::EditorState;

use crate::widgets::variables_panel::{
    VariablesResizeEdge, VARIABLES_PANEL_DEFAULT_HEIGHT, VARIABLES_PANEL_MIN_HEIGHT,
    VARIABLES_PANEL_MIN_WIDTH, VARIABLES_PANEL_WIDTH,
};
use crate::widgets::TOOLBAR_WIDTH;
use crate::{Point2D, Rect};

/// Gap between the floating toolbar column and the panel.
const VARIABLES_PANEL_GAP: f32 = 8.0;
/// TS reserves 72 px of container width / 16 px of height beyond the
/// panel when clamping a resize (`variables-panel.tsx` maxW/maxH).
const VARIABLES_PANEL_MAX_W_MARGIN: f32 = 72.0;
const VARIABLES_PANEL_MAX_H_MARGIN: f32 = 16.0;
/// Below these the panel would be unusable, so it is not painted at all.
const VARIABLES_PANEL_FLOOR_W: f32 = 240.0;
const VARIABLES_PANEL_FLOOR_H: f32 = 120.0;

/// Screen rect of the floating panel, or `None` when it is closed or the
/// canvas band is too small to host it.
///
/// `canvas` is `(left, top, width, height)` — exactly what both hosts'
/// `canvas_region` returns.
pub fn variables_panel_rect(
    state: &EditorState,
    canvas: (f32, f32, f32, f32),
    toolbar_inset_x: f32,
    toolbar_inset_y: f32,
) -> Option<Rect> {
    if !state.editor_ui.variables_panel_open {
        return None;
    }
    let (cx0, cy0, cw, ch) = canvas;
    let x = cx0 + toolbar_inset_x + TOOLBAR_WIDTH + VARIABLES_PANEL_GAP;
    let y = cy0 + toolbar_inset_y;
    let max_w = (cx0 + cw - x - VARIABLES_PANEL_MAX_W_MARGIN).max(0.0);
    let max_h = (cy0 + ch - y - VARIABLES_PANEL_MAX_H_MARGIN).max(0.0);
    if max_w < VARIABLES_PANEL_FLOOR_W || max_h < VARIABLES_PANEL_FLOOR_H {
        return None;
    }
    // User-resized size wins over the 820x480 default; both clamp
    // to [TS minimums, available canvas] so a viewport shrink
    // self-corrects a stale stored size.
    let (want_w, want_h) = state
        .editor_ui
        .variables_panel_size
        .unwrap_or((VARIABLES_PANEL_WIDTH, VARIABLES_PANEL_DEFAULT_HEIGHT));
    Some(Rect {
        origin: Point2D::new(x, y),
        size: Point2D::new(
            want_w.clamp(VARIABLES_PANEL_MIN_WIDTH.min(max_w), max_w),
            want_h.clamp(VARIABLES_PANEL_MIN_HEIGHT.min(max_h), max_h),
        ),
    })
}

/// Write the size an in-flight resize drag implies. The panel is
/// anchored top-left, so width / height derive directly from the cursor
/// minus the origin. Returns `true` when the stored size changed (the
/// host's `mark_dirty` trigger).
pub fn resize_from_cursor(
    state: &mut EditorState,
    edge: VariablesResizeEdge,
    rect: Rect,
    x: f32,
    y: f32,
) -> bool {
    let (mut w, mut h) = (rect.size.x, rect.size.y);
    if matches!(
        edge,
        VariablesResizeEdge::Right | VariablesResizeEdge::Corner
    ) {
        w = (x - rect.origin.x).max(VARIABLES_PANEL_MIN_WIDTH);
    }
    if matches!(
        edge,
        VariablesResizeEdge::Bottom | VariablesResizeEdge::Corner
    ) {
        h = (y - rect.origin.y).max(VARIABLES_PANEL_MIN_HEIGHT);
    }
    if (w, h) == (rect.size.x, rect.size.y) {
        return false;
    }
    // The rect getter clamps to the canvas region, so storing the raw
    // cursor-derived size is safe.
    state.editor_ui.variables_panel_size = Some((w, h));
    true
}
