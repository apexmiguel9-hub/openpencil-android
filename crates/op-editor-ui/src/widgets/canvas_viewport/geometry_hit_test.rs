//! Geometry/vertex edit hit-testing.
//!
//! Hit-tests for anchors, handles, and segments in the
//! geometry edit mode. Uses the same coordinate transform
//! pattern as `hit_test.rs`.

use crate::layout_scene::LayoutScene;
use crate::layout_scene::SceneNode;
use crate::{Point2D, Rect};
use op_editor_core::EditorState;
use op_editor_core::Viewport as DocViewport;
use op_editor_core::geometry_edit::{DraggedHandle, GeometryEditSession, GeometryHitTarget, SEGMENT_BAND_PX};

/// Screen-px radius for anchor hit-test.
const ANCHOR_HIT_RADIUS_PX: f32 = 15.0;
/// Screen-px radius for handle hit-test.
const HANDLE_HIT_RADIUS_PX: f32 = 10.0;

/// Hit-test the geometry edit overlay for anchors, handles,
/// and segments. Returns the hit target or `GeometryHitTarget::Empty`.
///
/// `canvas_rect` is the on-screen rect the canvas widget paints
/// into (same value passed to `CanvasViewport::paint`).
///
/// INPUT path — reads the layout-resolved [`LayoutScene`] + the
/// editor's selection / viewport state.
pub fn geometry_hit_test(
    canvas_rect: Rect,
    scene: &LayoutScene,
    state: &EditorState,
    session: &GeometryEditSession,
    point: Point2D,
) -> GeometryHitTarget {
    let node = match selected_geometry_node(scene, session) {
        Some(n) => n,
        None => return GeometryHitTarget::Empty,
    };
    let viewport = DocViewport {
        pan_x: state.viewport.pan_x,
        pan_y: state.viewport.pan_y,
        zoom: state.viewport.zoom,
    };
    let anchors = &node.path_anchors;
    if anchors.is_empty() {
        return GeometryHitTarget::Empty;
    }

    // First, check anchors (highest priority)
    for (idx, anchor) in anchors.iter().enumerate() {
        let screen_anchor = doc_to_screen(anchor.pos, canvas_rect, &viewport);
        let dist = (point.x - screen_anchor.x).hypot(point.y - screen_anchor.y);
        if dist <= ANCHOR_HIT_RADIUS_PX {
            return GeometryHitTarget::Anchor(idx);
        }
    }

    // Then check handles
    for (idx, anchor) in anchors.iter().enumerate() {
        let _screen_anchor = doc_to_screen(anchor.pos, canvas_rect, &viewport);

        if let Some(hin) = anchor.handle_in {
            let screen_hin = doc_to_screen(hin, canvas_rect, &viewport);
            let dist = (point.x - screen_hin.x).hypot(point.y - screen_hin.y);
            if dist <= HANDLE_HIT_RADIUS_PX {
                return GeometryHitTarget::Handle(DraggedHandle::In(idx));
            }
        }

        if let Some(hout) = anchor.handle_out {
            let screen_hout = doc_to_screen(hout, canvas_rect, &viewport);
            let dist = (point.x - screen_hout.x).hypot(point.y - screen_hout.y);
            if dist <= HANDLE_HIT_RADIUS_PX {
                return GeometryHitTarget::Handle(DraggedHandle::Out(idx));
            }
        }
    }

    // Finally check segments
    let closed = node.path_closed;
    let seg_count = if closed { anchors.len() } else { anchors.len().saturating_sub(1) };
    for i in 0..seg_count {
        let p1 = anchors[i].pos;
        let p2 = if i + 1 < anchors.len() {
            anchors[i + 1].pos
        } else if closed {
            anchors[0].pos
        } else {
            continue;
        };
        if point_near_segment(point, p1, p2, SEGMENT_BAND_PX, canvas_rect, &viewport) {
            return GeometryHitTarget::Segment(i);
        }
    }

    GeometryHitTarget::Empty
}

/// Return the SceneNode being geometry-edited, or `None`
/// if no valid node is found.
fn selected_geometry_node<'a>(
    scene: &'a LayoutScene,
    session: &GeometryEditSession,
) -> Option<&'a SceneNode> {
    let node_id = session.edited_node_ids.iter().next()?;
    scene.active_page()?.find(node_id)
}

/// Convert a doc-space point to screen space.
fn doc_to_screen(p: Point2D, canvas_rect: Rect, viewport: &DocViewport) -> Point2D {
    Point2D::new(
        canvas_rect.origin.x + viewport.pan_x + p.x * viewport.zoom,
        canvas_rect.origin.y + viewport.pan_y + p.y * viewport.zoom,
    )
}

/// Check if `point` is within `band` screen-px of the line
/// segment from `a` to `b`.
fn point_near_segment(
    point: Point2D,
    a: Point2D,
    b: Point2D,
    band: f32,
    canvas_rect: Rect,
    viewport: &DocViewport,
) -> bool {
    let screen_a = doc_to_screen(a, canvas_rect, viewport);
    let screen_b = doc_to_screen(b, canvas_rect, viewport);

    let dx = screen_b.x - screen_a.x;
    let dy = screen_b.y - screen_a.y;
    let len_sq = dx * dx + dy * dy;

    if len_sq < 0.0001 {
        // Degenerate segment — check distance to point a
        let dist = (point.x - screen_a.x).hypot(point.y - screen_a.y);
        return dist <= band;
    }

    // Project point onto the segment line
    let t = ((point.x - screen_a.x) * dx + (point.y - screen_a.y) * dy) / len_sq;
    let t_clamped = t.max(0.0).min(1.0);

    let closest_x = screen_a.x + t_clamped * dx;
    let closest_y = screen_a.y + t_clamped * dy;

    let dist = (point.x - closest_x).hypot(point.y - closest_y);
    dist <= band
}
