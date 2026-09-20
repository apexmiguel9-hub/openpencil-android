//! The raster escape hatch — the invariant that makes the rest of the
//! module safe to write.
//!
//! Any node the structured emitters judge they cannot express is
//! rendered on its own through the shared scene painter (the same one
//! the editor canvas and the PNG export use) and embedded as a picture
//! pinned at the node's exact rect. Its subtree is NOT walked
//! afterwards: the raster already contains every descendant, so
//! recursing would double-paint them.
//!
//! Because the fallback is per node rather than per slide, a board that
//! is entirely unexpressible degrades on its own to a single full-slide
//! image — the artifact this exporter replaced — while a board with one
//! awkward node keeps editable text everywhere else.
//!
//! The geometry below deliberately mirrors
//! `export_html_structured::fallback`. The two exporters landed in
//! parallel and share the rule but not the code; if a third backend
//! appears, this is the pair to hoist.

use op_editor_ui::layout_scene::{Effect, SceneNode};
use op_editor_ui::{Point2D, Rect};

use crate::export::{paint_node, render_raster_bytes, ExportError, RasterFormat};

/// Fallback rasters render at 2x.
///
/// Slides are projected up from their authored size, and structured
/// shapes stay sharp at any scale because they are vectors and live
/// text. A 1x raster beside them would be visibly the soft one. 2x costs
/// four times the bytes, but only on the nodes that took this path.
const FALLBACK_SCALE: f32 = 2.0;

/// Render `node` alone as PNG bytes, with the doc-space rect the image
/// must be placed at.
///
/// The rect is returned rather than recomputed by the caller because the
/// placement has to be the SAME rect the surface covers; deriving it
/// twice would make their agreement a coincidence. `None` means the node
/// has no positive-area painted bounds — there is nothing to rasterise.
pub fn render(node: &SceneNode) -> Result<Option<(Vec<u8>, Rect)>, ExportError> {
    let Some(doc_rect) = raster_rect(node) else {
        return Ok(None);
    };
    let png = render_raster_bytes(doc_rect, RasterFormat::Png, FALLBACK_SCALE, 0.0, |canvas| {
        paint_node(canvas, node);
    })?;
    Ok(Some((png, doc_rect)))
}

/// The doc-space rect the fallback surface must cover.
///
/// Starts from the node's paint-visible bounds (which already stop at a
/// clipping container and include unclipped descendant overflow), grows
/// them by whatever paints outside the silhouette — stroke half-width,
/// shadow reach, blur radius — and finally takes the axis-aligned
/// bounding box of that rect under the node's own rotation, because
/// `paint_node` applies the rotation itself and the surface has to be
/// big enough to hold the turned result.
fn raster_rect(node: &SceneNode) -> Option<Rect> {
    let base = op_editor_ui::scene_bounds::normalize_rect(node.visual_bounds());
    if base.size.x <= 0.0 || base.size.y <= 0.0 {
        return None;
    }
    let mut pad = node.stroke.map_or(0.0, |s| s.width.max(0.0));
    for effect in &node.effects {
        pad = pad.max(match effect {
            Effect::DropShadow(s) if !s.inner => {
                s.blur.max(0.0) + s.offset_x.abs().max(s.offset_y.abs())
            }
            Effect::DropShadow(_) => 0.0,
            Effect::Blur(b) => b.radius.max(0.0),
            Effect::BackgroundBlur { .. } => 0.0,
        });
    }
    let padded = Rect {
        origin: Point2D::new(base.origin.x - pad, base.origin.y - pad),
        size: Point2D::new(base.size.x + pad * 2.0, base.size.y + pad * 2.0),
    };
    Some(rotated_aabb(padded, node.rotation, node.aggregate_bounds()))
}

/// AABB of `rect` rotated by `radians` about the centre of `pivot_of`.
fn rotated_aabb(rect: Rect, radians: f32, pivot_of: Rect) -> Rect {
    if radians == 0.0 || !radians.is_finite() {
        return rect;
    }
    let (cx, cy) = (
        pivot_of.origin.x + pivot_of.size.x * 0.5,
        pivot_of.origin.y + pivot_of.size.y * 0.5,
    );
    let (sin, cos) = radians.sin_cos();
    let corners = [
        (rect.origin.x, rect.origin.y),
        (rect.origin.x + rect.size.x, rect.origin.y),
        (rect.origin.x, rect.origin.y + rect.size.y),
        (rect.origin.x + rect.size.x, rect.origin.y + rect.size.y),
    ];
    let (mut min_x, mut min_y) = (f32::MAX, f32::MAX);
    let (mut max_x, mut max_y) = (f32::MIN, f32::MIN);
    for (x, y) in corners {
        let (dx, dy) = (x - cx, y - cy);
        let rx = cx + dx * cos - dy * sin;
        let ry = cy + dx * sin + dy * cos;
        min_x = min_x.min(rx);
        min_y = min_y.min(ry);
        max_x = max_x.max(rx);
        max_y = max_y.max(ry);
    }
    Rect {
        origin: Point2D::new(min_x, min_y),
        size: Point2D::new(max_x - min_x, max_y - min_y),
    }
}
