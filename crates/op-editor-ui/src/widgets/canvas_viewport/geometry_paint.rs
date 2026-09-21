//! Geometry/vertex edit overlay painting.
//!
//! Renders anchor dots, control-handle lines, and segment
//! highlights for the active `GeometryEditSession`.
//!
//! Follows the same pattern as `canvas_path_overlay.rs`
//! — uses `with_node_overlay_transform` to apply the
//! root→node transform chain, then paints anchors, handles,
//! and segments in doc-space coordinates transformed to screen.

use crate::layout_scene::SceneNode;
use crate::theme::Theme;
use crate::widgets::canvas_overlay_transform::OverlayTransform;
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect};
use op_editor_core::Viewport;
use op_editor_core::geometry_edit::{DraggedHandle, GeometryEditSession};

/// Primary selection color — white with blue border.
const ANCHOR_FILL: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};
const ANCHOR_STROKE: Color = Color {
    r: 0.0,
    g: 0.48,
    b: 1.0,
    a: 1.0,
};
/// Selected anchor color.
const SELECTED_ANCHOR_FILL: Color = Color {
    r: 0.0,
    g: 0.48,
    b: 1.0,
    a: 1.0,
};
/// Handle line color (dim).
const HANDLE_LINE_DIM: Color = Color {
    r: 0.7,
    g: 0.7,
    b: 0.7,
    a: 0.5,
};
/// Handle line color (bright).
const HANDLE_LINE_BRIGHT: Color = Color {
    r: 0.0,
    g: 0.48,
    b: 1.0,
    a: 1.0,
};
/// Segment line color (dim).
const SEGMENT_DIM: Color = Color {
    r: 0.8,
    g: 0.8,
    b: 0.8,
    a: 0.3,
};
/// Segment line color (bright).
const SEGMENT_BRIGHT: Color = Color {
    r: 0.0,
    g: 0.48,
    b: 1.0,
    a: 0.8,
};

/// Anchor radius in screen px (unselected).
const ANCHOR_RADIUS: f32 = 4.0;
/// Anchor radius in screen px (selected).
const ANCHOR_RADIUS_SELECTED: f32 = 7.0;
/// Anchor radius in screen px (hovered).
const ANCHOR_RADIUS_HOVER: f32 = 6.0;
/// Handle dot radius in screen px.
const HANDLE_DOT_RADIUS: f32 = 4.0;

/// Paint the geometry/vertex edit overlay for the given session.
pub(super) fn paint_geometry_edit(
    cx: &mut PaintCx<'_>,
    _theme: &Theme,
    session: &GeometryEditSession,
    node: &SceneNode,
    canvas_rect: Rect,
    viewport: &Viewport,
    selected_transforms: &[OverlayTransform],
) {
    if !session.is_active() {
        return;
    }
    if !matches!(node.kind, crate::layout_scene::NodeKind::Path) {
        return;
    }
    if node.path_anchors.is_empty() {
        return;
    }

    with_node_overlay_transform(cx, node, canvas_rect, viewport, selected_transforms, |cx| {
        let zoom = viewport.zoom;
        let to_screen = |p: Point2D| -> Point2D {
            Point2D::new(
                canvas_rect.origin.x + viewport.pan_x + p.x * zoom,
                canvas_rect.origin.y + viewport.pan_y + p.y * zoom,
            )
        };

        // 1. Draw segments (dim by default, bright if selected)
        let anchors = &node.path_anchors;
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
            let screen_p1 = to_screen(p1);
            let screen_p2 = to_screen(p2);
            let is_selected = session.selected_segment == Some(i);
            cx.backend.stroke_line(
                screen_p1,
                screen_p2,
                if is_selected { SEGMENT_BRIGHT } else { SEGMENT_DIM },
                if is_selected { 2.0 } else { 1.0 },
            );
        }

        // 2. Draw handles (lines from anchor to handle endpoints)
        for (idx, anchor) in anchors.iter().enumerate() {
            let screen_anchor = to_screen(anchor.pos);
            let is_anchor_selected = session.is_anchor_selected(idx);
            let _is_anchor_hovered = session.hovered_anchor == Some(idx);

            // Handle in
            if let Some(hin) = anchor.handle_in {
                let screen_hin = to_screen(hin);
                let is_handle_hovered = session.hovered_handle == Some(DraggedHandle::In(idx));
                let is_handle_dragged = session.dragged_handle == Some(DraggedHandle::In(idx));
                let is_selected = is_anchor_selected || is_handle_hovered || is_handle_dragged;
                cx.backend.stroke_line(
                    screen_anchor,
                    screen_hin,
                    if is_selected { HANDLE_LINE_BRIGHT } else { HANDLE_LINE_DIM },
                    if is_selected { 2.0 } else { 1.0 },
                );
                let r = if is_handle_dragged {
                    HANDLE_DOT_RADIUS + 2.0
                } else if is_handle_hovered {
                    HANDLE_DOT_RADIUS + 1.0
                } else {
                    HANDLE_DOT_RADIUS
                };
                let bounds = Rect {
                    origin: Point2D::new(screen_hin.x - r, screen_hin.y - r),
                    size: Point2D::new(r * 2.0, r * 2.0),
                };
                cx.backend.fill_oval(bounds, if is_selected { SELECTED_ANCHOR_FILL } else { ANCHOR_FILL });
                cx.backend.stroke_oval(bounds, ANCHOR_STROKE, 1.0);
            }

            // Handle out
            if let Some(hout) = anchor.handle_out {
                let screen_hout = to_screen(hout);
                let is_handle_hovered = session.hovered_handle == Some(DraggedHandle::Out(idx));
                let is_handle_dragged = session.dragged_handle == Some(DraggedHandle::Out(idx));
                let is_selected = is_anchor_selected || is_handle_hovered || is_handle_dragged;
                cx.backend.stroke_line(
                    screen_anchor,
                    screen_hout,
                    if is_selected { HANDLE_LINE_BRIGHT } else { HANDLE_LINE_DIM },
                    if is_selected { 2.0 } else { 1.0 },
                );
                let r = if is_handle_dragged {
                    HANDLE_DOT_RADIUS + 2.0
                } else if is_handle_hovered {
                    HANDLE_DOT_RADIUS + 1.0
                } else {
                    HANDLE_DOT_RADIUS
                };
                let bounds = Rect {
                    origin: Point2D::new(screen_hout.x - r, screen_hout.y - r),
                    size: Point2D::new(r * 2.0, r * 2.0),
                };
                cx.backend.fill_oval(bounds, if is_selected { SELECTED_ANCHOR_FILL } else { ANCHOR_FILL });
                cx.backend.stroke_oval(bounds, ANCHOR_STROKE, 1.0);
            }
        }

        // 3. Draw anchor dots
        for (idx, anchor) in anchors.iter().enumerate() {
            let screen_anchor = to_screen(anchor.pos);
            let is_selected = session.is_anchor_selected(idx);
            let is_hovered = session.hovered_anchor == Some(idx);
            let is_dragged = session.dragged_anchor == Some(idx);
            let r = if is_dragged {
                ANCHOR_RADIUS_SELECTED + 2.0
            } else if is_selected {
                ANCHOR_RADIUS_SELECTED
            } else if is_hovered {
                ANCHOR_RADIUS_HOVER
            } else {
                ANCHOR_RADIUS
            };
            let bounds = Rect {
                origin: Point2D::new(screen_anchor.x - r, screen_anchor.y - r),
                size: Point2D::new(r * 2.0, r * 2.0),
            };
            cx.backend.fill_oval(bounds, if is_selected { SELECTED_ANCHOR_FILL } else { ANCHOR_FILL });
            cx.backend.stroke_oval(bounds, ANCHOR_STROKE, 1.0);
        }
    });
}

/// Wrap `f` in the same root→node transform chain the path
/// painted under. Fallback to the legacy own-node rotation.
fn with_node_overlay_transform(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    canvas_rect: Rect,
    viewport: &Viewport,
    transforms: &[OverlayTransform],
    f: impl FnOnce(&mut PaintCx<'_>),
) {
    let transformed = crate::widgets::canvas_overlay_transform::replay_on_backend(cx, transforms)
        || replay_legacy_node_rotation(cx, node, canvas_rect, viewport, transforms);
    f(cx);
    if transformed {
        cx.backend.restore();
    }
}

fn replay_legacy_node_rotation(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    canvas_rect: Rect,
    viewport: &Viewport,
    transforms: &[OverlayTransform],
) -> bool {
    if !transforms.is_empty() || node.rotation.abs() <= f32::EPSILON {
        return false;
    }
    {
        let b = node.aggregate_bounds();
        let pivot = Point2D::new(
            canvas_rect.origin.x + viewport.pan_x + (b.origin.x + b.size.x / 2.0) * viewport.zoom,
            canvas_rect.origin.y + viewport.pan_y + (b.origin.y + b.size.y / 2.0) * viewport.zoom,
        );
        cx.backend.save();
        cx.backend.rotate(node.rotation, pivot);
    }
    true
}
