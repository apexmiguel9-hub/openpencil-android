//! Shape painters carved out of `canvas_viewport_paint.rs` to keep that
//! spine under the repository's 800-line cap: drop shadows, ellipse /
//! arc / donut tessellation, `clipContent` child clips and SVG-path
//! nodes. Pure code motion — behaviour is unchanged.

use super::MIN_VISIBLE_EFFECT_DEVICE_PX;
use crate::layout_scene::{Effect, NodeKind, SceneNode};
use crate::widgets::canvas_viewport_fill_layers::{
    fill_layer_fallback_color, paint_clipped_fill_layers_with, paint_clipped_shape_rich_fill_layer,
    paint_svg_path_fill_layers, paint_svg_path_gradient,
};
use crate::widgets::canvas_viewport_overlay::{align_stroke_rect, scaled_non_uniform_corner_radii};
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};
/// Paint every `Effect::DropShadow` on `node` as a blurred shape
/// behind its fill. The shadow corner radius matches the node
/// kind — `corner_radius` for Frame / Rect, min-half for an
/// ellipse silhouette. Offset + blur scale by `zoom` so the
/// shadow tracks the node across viewport zoom. A shadow whose
/// blur AND offset are both sub-pixel on screen skips entirely.
pub(super) fn paint_drop_shadows(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    world_rect: Rect,
    zoom: f32,
    dpi_scale: f32,
) {
    let radius = if node.kind == NodeKind::Ellipse {
        world_rect.size.x.min(world_rect.size.y) / 2.0
    } else {
        node.corner_radius * zoom
    };
    for effect in &node.effects {
        let Effect::DropShadow(s) = effect else {
            continue;
        };
        // Inset shadows are painted inside the silhouette by the
        // per-kind painter, not here — skip them in the outer pass.
        if s.inner {
            continue;
        }
        let footprint = s.blur.max(s.offset_x.abs()).max(s.offset_y.abs());
        if footprint * zoom * dpi_scale < MIN_VISIBLE_EFFECT_DEVICE_PX {
            continue;
        }
        let shadow_rect = Rect {
            origin: Point2D::new(
                world_rect.origin.x + s.offset_x * zoom,
                world_rect.origin.y + s.offset_y * zoom,
            ),
            size: world_rect.size,
        };
        cx.backend
            .fill_drop_shadow(shadow_rect, radius, s.blur * zoom, s.color);
    }
}

/// Tessellate an ellipse arc / pie / donut-sector into a closed
/// polygon outline. `start_deg` / `sweep_deg` use the screen
/// convention (0° = +X, positive = clockwise); `inner` is the
/// donut-hole radius as a 0.0..=1.0 fraction.
pub(crate) fn arc_polygon(rect: Rect, start_deg: f32, sweep_deg: f32, inner: f32) -> Vec<Point2D> {
    let cx_pt = rect.origin.x + rect.size.x / 2.0;
    let cy_pt = rect.origin.y + rect.size.y / 2.0;
    let rx = rect.size.x / 2.0;
    let ry = rect.size.y / 2.0;
    // ~1 segment per 4° of sweep, clamped to a sane range.
    let segs = ((sweep_deg.abs() / 4.0).ceil() as usize).clamp(2, 512);
    let point = |frac: f32, scale: f32| -> Point2D {
        let ang = (start_deg + sweep_deg * frac).to_radians();
        Point2D::new(
            cx_pt + rx * scale * ang.cos(),
            cy_pt + ry * scale * ang.sin(),
        )
    };
    let mut poly = Vec::with_capacity(segs * 2 + 2);
    if inner > 0.001 {
        // Annular sector: outer arc start→end, inner arc end→start.
        for i in 0..=segs {
            poly.push(point(i as f32 / segs as f32, 1.0));
        }
        for i in (0..=segs).rev() {
            poly.push(point(i as f32 / segs as f32, inner));
        }
    } else {
        // Pie wedge: centre + outer arc.
        poly.push(Point2D::new(cx_pt, cy_pt));
        for i in 0..=segs {
            poly.push(point(i as f32 / segs as f32, 1.0));
        }
    }
    poly
}

/// Paint an Ellipse node — a full oval when no arc geometry is
/// authored, otherwise a tessellated pie / arc / donut sector.
pub(super) fn paint_ellipse(cx: &mut PaintCx<'_>, node: &SceneNode, world_rect: Rect, zoom: f32) {
    let inner = node.arc_inner_radius.unwrap_or(0.0).clamp(0.0, 1.0);
    let has_arc = node.arc_start_angle.is_some() || node.arc_sweep_angle.is_some() || inner > 0.001;
    let sweep = node.arc_sweep_angle.unwrap_or(360.0);
    let plain_oval = !has_arc || (sweep.abs() >= 359.9 && inner <= 0.001);
    let arc = (!plain_oval).then(|| {
        arc_polygon(
            world_rect,
            node.arc_start_angle.unwrap_or(0.0),
            sweep,
            inner,
        )
    });
    let layered = paint_clipped_fill_layers_with(
        cx,
        node,
        world_rect,
        |backend| {
            if let Some(poly) = arc.as_deref() {
                backend.clip_polygon(poly);
            } else {
                backend.clip_oval(world_rect);
            }
        },
        |cx, layer| {
            if paint_clipped_shape_rich_fill_layer(cx, node, layer, world_rect, zoom) {
                return;
            }
            let Some(fill) = fill_layer_fallback_color(layer) else {
                return;
            };
            // The exact anti-aliased silhouette is already installed as the
            // clip above. Painting that same edge a second time would multiply
            // clip and draw coverage, leaving a dark/shrunken fringe.
            cx.backend.fill_rect(world_rect, fill);
        },
    );

    if plain_oval {
        if !layered {
            if let Some(fill) = node.fill {
                cx.backend.fill_oval(world_rect, fill);
            }
        }
        if let Some(stroke) = node.stroke {
            let w = stroke.width * zoom;
            let (rect, _) = align_stroke_rect(world_rect, 0.0, w, stroke.align);
            cx.backend.stroke_oval(rect, stroke.color, w);
        }
        return;
    }
    let poly = arc.as_deref().expect("non-oval ellipse has arc geometry");
    if !layered {
        if let Some(fill) = node.fill {
            cx.backend.fill_polygon(poly, fill);
        }
    }
    if let Some(stroke) = node.stroke {
        let w = stroke.width * zoom;
        if sweep.abs() >= 359.9 && inner > 0.001 {
            // Full ring — stroke the two concentric ovals so the
            // polygon's radial seam isn't drawn.
            cx.backend.stroke_oval(world_rect, stroke.color, w);
            let iw = world_rect.size.x * inner;
            let ih = world_rect.size.y * inner;
            let inner_rect = Rect {
                origin: Point2D::new(
                    world_rect.origin.x + (world_rect.size.x - iw) / 2.0,
                    world_rect.origin.y + (world_rect.size.y - ih) / 2.0,
                ),
                size: Point2D::new(iw, ih),
            };
            cx.backend.stroke_oval(inner_rect, stroke.color, w);
        } else {
            cx.backend.stroke_polygon(poly, stroke.color, w);
        }
    }
}

/// Push a children-clip for a `clipContent` container (root frames
/// included — the scene builder bakes that rule). Mirrors the TS
/// renderer (`document-flattener.ts` clip stack + `node-renderer.ts`
/// `clipRRect`): children clip to the container's bounds with the
/// corner radius clamped to half the height; the container's OWN fill
/// / stroke paint un-clipped before this. Returns whether a
/// `save` was pushed (caller must `restore` after the children).
/// Off-clip children skip via the regular viewport cull anyway — the
/// clip only trims partially-overflowing descendants.
pub(super) fn push_clip_content(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    world_rect: Rect,
    zoom: f32,
) -> bool {
    if !node.clip_content
        || node.children.is_empty()
        || world_rect.size.x <= 0.0
        || world_rect.size.y <= 0.0
    {
        return false;
    }
    cx.backend.save();
    // TS flattener: `cr = Math.min(crRaw, nodeH / 2)`.
    let radius = node.corner_radius.min(node.bounds.size.y / 2.0).max(0.0) * zoom;
    let per_corner = scaled_non_uniform_corner_radii(node, zoom).map(|radii| {
        let max_radius = world_rect.size.y / 2.0;
        radii.map(|value| value.min(max_radius).max(0.0))
    });
    if let Some(radii) = per_corner {
        cx.backend.clip_round_rect_per_corner(world_rect, radii);
    } else if radius > 0.5 {
        cx.backend.clip_round_rect(world_rect, radius);
    } else {
        cx.backend.clip_rect(world_rect);
    }
    true
}

pub(crate) fn paint_svg_path_node(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    world_rect: Rect,
    zoom: f32,
    d: &str,
) {
    let layered = paint_svg_path_fill_layers(cx, node, world_rect, zoom, d);
    if !layered {
        // Gradient-filled paths paint through the dedicated gradient method
        // (real shader on native, solid first-stop fallback elsewhere).
        match node.gradient.as_ref() {
            Some(gradient) => paint_svg_path_gradient(cx, node, gradient, world_rect, d, node.fill),
            None => {
                if let Some(fill) = node.fill {
                    cx.backend.fill_svg_path_in_rect_with_fill_rule(
                        d,
                        world_rect,
                        fill,
                        node.even_odd_fill,
                    );
                }
            }
        }
    }
    // Inset shadows paint over the fill, clipped to the path
    // silhouette. Outer shadows on paths stay deferred (no shape-mask
    // drop-shadow path for arbitrary vectors yet).
    for effect in &node.effects {
        let Effect::DropShadow(s) = effect else {
            continue;
        };
        if s.inner {
            cx.backend.fill_inner_shadow_svg_path_with_fill_rule(
                d,
                world_rect,
                s.offset_x * zoom,
                s.offset_y * zoom,
                s.blur * zoom,
                s.color,
                node.even_odd_fill,
            );
        }
    }
    if let Some(stroke) = node.stroke {
        cx.backend
            .stroke_svg_path_in_rect(d, world_rect, stroke.color, stroke.width * zoom);
    }
}
