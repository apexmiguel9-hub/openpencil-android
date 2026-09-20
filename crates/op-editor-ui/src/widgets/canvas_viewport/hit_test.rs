//! Selection / rotation / path / arc handle geometry and the host's
//! input hit-tests for [`super::CanvasViewport`].
//!
//! Split out of `canvas_viewport.rs` to keep that spine under the
//! repository's 800-line cap. Every public item is re-exported from the
//! spine so existing `canvas_viewport::…` paths keep resolving.

use crate::layout_scene::LayoutScene;
use crate::layout_scene::NodeKind;
use crate::layout_scene::{SceneAnchor, SceneNode};
use crate::{Point2D, Rect};
use op_editor_core::EditorState;
use op_editor_core::Viewport as DocViewport;
/// One of the 8 selection handles (corners + edge midpoints) the
/// selection overlay paints. Used by the host to dispatch resize
/// drags: each variant fixes the corresponding edge / corner of
/// the selected bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionHandle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

impl SelectionHandle {
    /// Authored dimensions changed by this handle.
    pub fn resize_axes(self) -> op_editor_core::drag_mutators::ResizeAxes {
        use op_editor_core::drag_mutators::ResizeAxes;
        match (self.resizes_width(), self.resizes_height()) {
            (true, true) => ResizeAxes::Both,
            (true, false) => ResizeAxes::Width,
            (false, true) => ResizeAxes::Height,
            (false, false) => unreachable!("every selection handle resizes at least one axis"),
        }
    }

    /// Whether this handle authors the selected node's width.
    pub fn resizes_width(self) -> bool {
        !matches!(self, Self::Top | Self::Bottom)
    }

    /// Whether this handle authors the selected node's height.
    pub fn resizes_height(self) -> bool {
        !matches!(self, Self::Left | Self::Right)
    }

    /// Whether dragging this handle moves the selected node's left edge.
    pub fn moves_left_edge(self) -> bool {
        matches!(self, Self::Left | Self::TopLeft | Self::BottomLeft)
    }

    /// Whether dragging this handle moves the selected node's top edge.
    pub fn moves_top_edge(self) -> bool {
        matches!(self, Self::Top | Self::TopLeft | Self::TopRight)
    }
}

/// Radius (screen px) of the rotation ring that sits OUTSIDE the
/// 4 selection corners. Matches the TS `ROTATE_OUTER_RADIUS`.
const ROTATE_OUTER_RADIUS: f32 = 16.0;

/// Rotate `p` by `radians` (clockwise, screen y-down) about `center`.
/// Used by the host to un-rotate a cursor point into a rotated
/// node's local frame before hit-testing its handles.
pub fn rotate_point(p: Point2D, center: Point2D, radians: f32) -> Point2D {
    let (s, c) = radians.sin_cos();
    let dx = p.x - center.x;
    let dy = p.y - center.y;
    Point2D::new(center.x + dx * c - dy * s, center.y + dx * s + dy * c)
}

/// Screen-px offset of a "ghost" handle dot from its anchor when the
/// handle is unset — far enough from the anchor body to grab.
pub const PATH_HANDLE_GHOST_PX: f32 = 26.0;

/// Doc-space positions of a path anchor's incoming + outgoing bezier
/// control handles. An unset handle is given a "ghost" position
/// offset from the anchor (scaled to `zoom`) so the user can grab it
/// to create the handle. Returns `(handle_in, handle_out)`. Shared by
/// the overlay painter and the host's handle hit-test.
pub fn path_handle_positions(anchor: &SceneAnchor, zoom: f32) -> (Point2D, Point2D) {
    let ghost = PATH_HANDLE_GHOST_PX / zoom.max(0.0001);
    let hin = anchor
        .handle_in
        .unwrap_or(Point2D::new(anchor.pos.x - ghost, anchor.pos.y));
    let hout = anchor
        .handle_out
        .unwrap_or(Point2D::new(anchor.pos.x + ghost, anchor.pos.y));
    (hin, hout)
}

/// The three arc-edit handles on a selected Ellipse — start angle,
/// sweep (end) angle, and the donut inner-radius.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArcHandle {
    /// Perimeter handle at the arc's start angle.
    Start,
    /// Perimeter handle at the arc's end angle (start + sweep).
    Sweep,
    /// Radial handle controlling the donut inner-radius fraction.
    Inner,
}

/// Doc-space positions of the three arc handles for an Ellipse
/// `SceneNode`. `None` for non-Ellipse kinds or a zero-size node.
/// Shared by the overlay painter and the host's arc-handle hit-test
/// so both agree on handle placement.
pub fn arc_handle_positions(node: &SceneNode) -> Option<[(ArcHandle, Point2D); 3]> {
    if !matches!(node.kind, NodeKind::Ellipse) {
        return None;
    }
    let b = node.bounds;
    if b.size.x <= 0.0 || b.size.y <= 0.0 {
        return None;
    }
    let cx = b.origin.x + b.size.x / 2.0;
    let cy = b.origin.y + b.size.y / 2.0;
    let rx = b.size.x / 2.0;
    let ry = b.size.y / 2.0;
    let start = node.arc_start_angle.unwrap_or(0.0);
    let sweep = node.arc_sweep_angle.unwrap_or(360.0);
    let inner = node.arc_inner_radius.unwrap_or(0.0).clamp(0.0, 1.0);
    let at = |deg: f32, scale: f32| -> Point2D {
        let a = deg.to_radians();
        Point2D::new(cx + rx * scale * a.cos(), cy + ry * scale * a.sin())
    };
    Some([
        (ArcHandle::Start, at(start, 1.0)),
        (ArcHandle::Sweep, at(start + sweep, 1.0)),
        (ArcHandle::Inner, at(start, inner)),
    ])
}

/// The single resolved scene node the editor's selection anchor
/// points at, or `None` when the selection isn't a single node that
/// resolves on the active page. Shared by the two selection-overlay
/// hit-tests below — they only fire on single-select.
fn selected_scene_node<'a>(
    scene: &'a LayoutScene,
    state: &EditorState,
) -> Option<&'a crate::layout_scene::SceneNode> {
    if state.selection_count() != 1 {
        return None;
    }
    let anchor = state.selection.anchor.as_str();
    scene.active_page()?.find(anchor)
}

/// Hit-test the rotation ring that sits just outside the four
/// corner handles. Returns the nearest corner (so the runner can
/// hint which way the rotation drag is anchored) or `None` if the
/// cursor isn't in a rotation zone.
///
/// The rotation zone is an annulus around each corner — beyond
/// the 6 px handle slop and inside the 16 px outer radius. Matches
/// the TS `hitTestRotation` logic.
///
/// INPUT path — reads the layout-resolved [`LayoutScene`] (selected
/// node geometry) + the editor's selection / viewport state.
pub fn rotation_corner_at_point(
    canvas_rect: Rect,
    scene: &LayoutScene,
    state: &EditorState,
    point: Point2D,
) -> Option<SelectionHandle> {
    // Rotation rings are only painted on single-select (the
    // multi-select overlay is outline-only), so gate the hit-test
    // to match — otherwise non-anchor "rotation zones" would
    // intercept clicks on dead air.
    let node = selected_scene_node(scene, state)?;
    let bounds = node.aggregate_bounds();
    if bounds.size.x <= 0.0 || bounds.size.y <= 0.0 {
        return None;
    }
    let viewport = DocViewport {
        pan_x: state.viewport.pan_x,
        pan_y: state.viewport.pan_y,
        zoom: state.viewport.zoom,
    };
    let left = canvas_rect.origin.x + viewport.pan_x + bounds.origin.x * viewport.zoom;
    let top = canvas_rect.origin.y + viewport.pan_y + bounds.origin.y * viewport.zoom;
    let right = left + bounds.size.x * viewport.zoom;
    let bottom = top + bounds.size.y * viewport.zoom;
    // Inverse-rotate the cursor into the node's local space so the
    // hit-test annulus tracks the rendered (rotated) corners.
    let cx = (left + right) / 2.0;
    let cy = (top + bottom) / 2.0;
    let local = inverse_rotate(point, Point2D::new(cx, cy), node.rotation);
    let inner = 6.0_f32;
    let outer = ROTATE_OUTER_RADIUS;
    let corners = [
        (SelectionHandle::TopLeft, left, top),
        (SelectionHandle::TopRight, right, top),
        (SelectionHandle::BottomLeft, left, bottom),
        (SelectionHandle::BottomRight, right, bottom),
    ];
    for (kind, cx, cy) in corners {
        let dx = local.x - cx;
        let dy = local.y - cy;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist > inner && dist <= outer {
            return Some(kind);
        }
    }
    None
}

/// Hit-test the 8 selection handles around the currently-selected
/// node. Returns the handle at `point` (a small slop around each
/// handle center counts) or `None` if no selection / no handle.
///
/// `canvas_rect` is the on-screen rect the canvas widget paints
/// into (same value passed to `CanvasViewport::paint`). The
/// transform from document → screen is identical to paint so a
/// handle the user clicks is the handle they see.
///
/// INPUT path — reads the layout-resolved [`LayoutScene`] + the
/// editor's selection / viewport state (see [`rotation_corner_at_point`]).
pub fn selection_handle_at_point(
    canvas_rect: Rect,
    scene: &LayoutScene,
    state: &EditorState,
    point: Point2D,
) -> Option<SelectionHandle> {
    // Handles are only painted on single-select (the multi-select
    // overlay is outline-only — Figma parity), so gate the hit-
    // test to match. Otherwise the "anchor's handles" would hit-
    // test even though no handles are visible anywhere.
    let node = selected_scene_node(scene, state)?;
    let bounds = node.aggregate_bounds();
    if bounds.size.x <= 0.0 || bounds.size.y <= 0.0 {
        return None;
    }
    let viewport = DocViewport {
        pan_x: state.viewport.pan_x,
        pan_y: state.viewport.pan_y,
        zoom: state.viewport.zoom,
    };
    let left = canvas_rect.origin.x + viewport.pan_x + bounds.origin.x * viewport.zoom;
    let top = canvas_rect.origin.y + viewport.pan_y + bounds.origin.y * viewport.zoom;
    let right = left + bounds.size.x * viewport.zoom;
    let bottom = top + bounds.size.y * viewport.zoom;
    let mid_x = (left + right) / 2.0;
    let mid_y = (top + bottom) / 2.0;
    // Inverse-rotate the cursor so handle hit-test tracks rendered
    // (rotated) handle positions.
    let local = inverse_rotate(point, Point2D::new(mid_x, mid_y), node.rotation);
    let slop = 6.0;
    let anchors = [
        (SelectionHandle::TopLeft, left, top),
        (SelectionHandle::Top, mid_x, top),
        (SelectionHandle::TopRight, right, top),
        (SelectionHandle::Right, right, mid_y),
        (SelectionHandle::BottomRight, right, bottom),
        (SelectionHandle::Bottom, mid_x, bottom),
        (SelectionHandle::BottomLeft, left, bottom),
        (SelectionHandle::Left, left, mid_y),
    ];
    for (kind, hx, hy) in anchors {
        if (local.x - hx).abs() <= slop && (local.y - hy).abs() <= slop {
            return Some(kind);
        }
    }
    None
}

/// Apply the inverse of a rotation about `pivot` to `point`. Used
/// by hit-tests so a rotated selection's handles + rotation ring
/// + body all match the rendered (rotated) geometry.
fn inverse_rotate(point: Point2D, pivot: Point2D, radians: f32) -> Point2D {
    if radians.abs() < f32::EPSILON {
        return point;
    }
    let dx = point.x - pivot.x;
    let dy = point.y - pivot.y;
    let cos_t = (-radians).cos();
    let sin_t = (-radians).sin();
    Point2D::new(
        pivot.x + dx * cos_t - dy * sin_t,
        pivot.y + dx * sin_t + dy * cos_t,
    )
}
