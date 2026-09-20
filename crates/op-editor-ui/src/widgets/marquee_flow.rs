//! Marquee-release commit shared by the native and web widget hosts.
//!
//! Their `widget_host/click.rs` / `widget_host/release_input.rs` twins
//! carried this screen→doc rect walk plus the additive / replace
//! selection branch as byte-identical copies. It is pure `EditorState`
//! mutation over a `LayoutScene` query, so it lives here and each host
//! keeps a thin forwarding method.

use op_editor_core::host_drag_state::MarqueeDragState;
use op_editor_core::{EditorState, NodeId};

use crate::layout_scene::LayoutScene;
use crate::widgets::host_canvas_geometry::canvas_doc_point_unclamped;
use crate::Rect;

/// 2 screen-px marquee threshold (TS `useMarqueeStart`). Measured in
/// SCREEN pixels so it stays consistent regardless of canvas zoom.
const MARQUEE_THRESHOLD_PX: f32 = 2.0;

/// Whether `m` cleared the drag threshold — below it the gesture was a
/// click without a drag and the marquee commits nothing.
pub fn marquee_dragged(m: &MarqueeDragState) -> bool {
    let screen_dx = (m.current_screen_x - m.start_screen_x).abs();
    let screen_dy = (m.current_screen_y - m.start_screen_y).abs();
    screen_dx >= MARQUEE_THRESHOLD_PX || screen_dy >= MARQUEE_THRESHOLD_PX
}

/// Convert a marquee drag (screen space) into a doc-space rect, ask the
/// resolved scene which nodes overlap it, and either replace or extend
/// the selection.
///
/// Returns `true` when the selection changed, so the host marks dirty.
/// Callers must have refreshed their `LayoutScene` — the hit query reads
/// it, not the document.
///
/// The `additive` branch is ADD-only: every hit joins the set and
/// already-selected hits stay selected (TS shift-marquee parity, which
/// never removes). A non-additive marquee that hit nothing leaves the
/// selection alone — the empty-marquee clear already happened at press
/// time.
///
/// The tail always runs `sync_entered_container_with_selection`: a
/// marquee selection landing outside the entered container steps out of
/// it (the selection-outside-exits rule). Only the native twin carried
/// this; the web twin's omission was a drift, and the sync is a no-op
/// unless a container is actually entered — which the web host reaches
/// through the same `clear_selection_on_empty_canvas_press` seam.
pub fn commit_marquee_selection(
    state: &mut EditorState,
    scene: &LayoutScene,
    m: &MarqueeDragState,
) -> bool {
    if !marquee_dragged(m) {
        return false;
    }
    let p0 = canvas_doc_point_unclamped(state, m.start_screen_x, m.start_screen_y);
    let p1 = canvas_doc_point_unclamped(state, m.current_screen_x, m.current_screen_y);
    let rect = Rect::xywh(
        p0.x.min(p1.x),
        p0.y.min(p1.y),
        (p1.x - p0.x).abs(),
        (p1.y - p0.y).abs(),
    );
    // `nodes_intersecting_doc_rect` queries the `LayoutScene` — it
    // returns the resolved-scene node id strings.
    let ids = scene.nodes_intersecting_doc_rect(rect);
    let changed = if m.additive {
        for id in ids {
            let ec_id = NodeId::new(&id);
            if !state.is_selected(&ec_id) {
                state.toggle_selection(ec_id);
            }
        }
        true
    } else if !ids.is_empty() {
        // Replace with the hit set; anchor = last hit.
        let ec_ids: Vec<NodeId> = ids.iter().map(NodeId::new).collect();
        let anchor = ec_ids.last().cloned().unwrap_or(NodeId::NONE);
        state.selection.set = ec_ids;
        state.selection.anchor = anchor;
        true
    } else {
        false
    };
    state.sync_entered_container_with_selection();
    changed
}
