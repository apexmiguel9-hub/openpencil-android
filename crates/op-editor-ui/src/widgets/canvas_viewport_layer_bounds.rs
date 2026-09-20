//! Conservative offscreen-layer bounds for canvas compositing.
//!
//! Node blend modes and Figma sibling masks need save-layers, but allocating
//! those layers for the whole viewport is unnecessarily expensive. These
//! helpers bound the painted subtree (including strokes, shadows, and layer
//! blur), transform it into viewport coordinates, and intersect it with the
//! caller's cull rect. Invalid geometry falls back to the cull rect so a bad
//! imported value cannot turn into an unbounded or NaN allocation hint.

use super::OverlayTransform;
use crate::layout_scene::{Effect, NodeKind, SceneNode, SceneStrokeAlign};
use crate::{Point2D, Rect};

/// Skia's practical Gaussian support is three sigma on either side. Both
/// scene blur radii and drop-shadow blur values are converted to sigma by
/// dividing by two, hence `radius * 1.5` below.
const FILTER_OUTSET_PER_RADIUS: f32 = 1.5;
/// Preserve the antialiased fringe at an otherwise exact layer edge.
const DEVICE_EDGE_PAD: f32 = 1.0;
const MIN_LAYER_EXTENT: f32 = 1.0;

type BoundsResult = Result<Option<Rect>, ()>;

/// Bounds for a node-level blend layer. The caller has already installed the
/// root node's transform, so only descendant transforms are folded into this
/// local-coordinate allocation hint.
pub(super) fn node_composite_layer_bounds(
    node: &SceneNode,
    viewport_origin: Point2D,
    zoom: f32,
    cull: Rect,
    hidden_id: Option<&str>,
    transforms: &[OverlayTransform],
) -> Rect {
    finish_layer_bounds(
        subtree_visual_bounds(node, false, hidden_id),
        viewport_origin,
        zoom,
        cull,
        transforms,
    )
}

/// Bounds for one sibling-mask run. Mask and content roots have not installed
/// their own transforms yet, so each root transform is included. `content`
/// must contain only the front siblings governed by this mask (the mask
/// painter stops the iterator at the next mask).
pub(super) fn sibling_mask_layer_bounds<'a>(
    mask: &SceneNode,
    content: impl IntoIterator<Item = &'a SceneNode>,
    viewport_origin: Point2D,
    zoom: f32,
    cull: Rect,
    hidden_id: Option<&str>,
    transforms: &[OverlayTransform],
) -> Rect {
    let mut bounds = subtree_visual_bounds(mask, true, hidden_id);
    for node in content {
        bounds = union_results(bounds, subtree_visual_bounds(node, true, hidden_id));
    }
    finish_layer_bounds(bounds, viewport_origin, zoom, cull, transforms)
}

/// Whether any paint from `node` or its descendants can reach `cull`.
///
/// Unlike the old leaf-only canvas cull, this uses the same conservative
/// effect- and transform-aware subtree bounds as offscreen layer allocation.
/// Invalid imported geometry stays paintable rather than disappearing.
pub(super) fn subtree_intersects_cull(
    node: &SceneNode,
    viewport_origin: Point2D,
    zoom: f32,
    cull: Rect,
    hidden_id: Option<&str>,
    transforms: &[OverlayTransform],
) -> bool {
    let cull = match finite_normalized_rect(cull) {
        Ok(Some(cull)) => cull,
        _ => return true,
    };
    let local_cull = match inverse_transform_rect(cull, transforms) {
        Ok(cull) => cull,
        Err(()) => return true,
    };
    let doc_bounds = match subtree_visual_bounds(node, true, hidden_id) {
        Ok(Some(bounds)) => bounds,
        Ok(None) => return false,
        Err(()) => return true,
    };
    let world_bounds = match doc_to_world_rect(doc_bounds, viewport_origin, zoom)
        .and_then(|bounds| outset_rect(bounds, DEVICE_EDGE_PAD))
    {
        Ok(bounds) => bounds,
        Err(()) => return true,
    };
    intersect_rects(world_bounds, local_cull).is_some()
}

fn subtree_visual_bounds(
    node: &SceneNode,
    apply_root_transform: bool,
    hidden_id: Option<&str>,
) -> BoundsResult {
    if node.hidden || hidden_id == Some(node.id.as_str()) {
        return Ok(None);
    }

    let own_bounds = finite_normalized_rect(node.bounds)?;
    let mut visual = own_bounds
        .map(|bounds| stroke_visual_bounds(node, bounds))
        .transpose()?;

    for child in &node.children {
        visual = union_results(Ok(visual), subtree_visual_bounds(child, true, hidden_id))?;
    }

    // The painter currently casts outer shadows only for shapes whose
    // silhouette can be represented by its rounded-rect/ellipse primitive.
    if matches!(
        node.kind,
        NodeKind::Frame | NodeKind::Rect | NodeKind::Ellipse
    ) {
        if let Some(own) = own_bounds {
            for effect in &node.effects {
                let Effect::DropShadow(shadow) = effect else {
                    continue;
                };
                if shadow.inner {
                    continue;
                }
                if !shadow.offset_x.is_finite()
                    || !shadow.offset_y.is_finite()
                    || !shadow.blur.is_finite()
                {
                    return Err(());
                }
                let mut shadow_bounds = translate_rect(own, shadow.offset_x, shadow.offset_y)?;
                if shadow.blur > 0.0 {
                    shadow_bounds =
                        outset_rect(shadow_bounds, shadow.blur * FILTER_OUTSET_PER_RADIUS)?;
                }
                visual = union_options(visual, Some(shadow_bounds))?;
            }
        }
    }

    // Layer blur captures shadows, own paint, and every descendant, so its
    // filter support expands the completed union rather than only own bounds.
    for effect in &node.effects {
        match effect {
            Effect::Blur(blur) => {
                if !blur.radius.is_finite() {
                    return Err(());
                }
                if blur.radius > 0.0 {
                    visual = visual
                        .map(|bounds| outset_rect(bounds, blur.radius * FILTER_OUTSET_PER_RADIUS))
                        .transpose()?;
                }
            }
            Effect::BackgroundBlur { radius } => {
                // Backdrop blur is clipped to the node silhouette and does
                // not extend the node's output, but reject non-finite input.
                if !radius.is_finite() {
                    return Err(());
                }
            }
            Effect::DropShadow(_) => {}
        }
    }

    let transformed = node.flip_x || node.flip_y || node.rotation.abs() > f32::EPSILON;
    if apply_root_transform && transformed {
        if !node.rotation.is_finite() {
            return Err(());
        }
        let Some(bounds) = visual else {
            return Ok(None);
        };
        let pivot_bounds = match finite_normalized_rect(node.aggregate_bounds())? {
            Some(bounds) => bounds,
            None => bounds,
        };
        let pivot = Point2D::new(
            pivot_bounds.origin.x + pivot_bounds.size.x * 0.5,
            pivot_bounds.origin.y + pivot_bounds.size.y * 0.5,
        );
        visual = Some(transformed_aabb(
            bounds,
            node.flip_x,
            node.flip_y,
            node.rotation,
            pivot,
        )?);
    }

    Ok(visual)
}

fn stroke_visual_bounds(node: &SceneNode, bounds: Rect) -> Result<Rect, ()> {
    let Some(stroke) = node.stroke else {
        // Lines and unfilled open paths get a small implicit stroke in the
        // painter. Device padding below covers the normal case; this doc-space
        // allowance also protects high zoom values.
        return if matches!(node.kind, NodeKind::Line | NodeKind::Path) {
            outset_rect(bounds, 1.0)
        } else {
            Ok(bounds)
        };
    };
    let mut width = stroke.width;
    if let Some(sides) = stroke.sides {
        for side in sides {
            if !side.is_finite() {
                return Err(());
            }
            width = width.max(side);
        }
    }
    if !width.is_finite() {
        return Err(());
    }
    let width = width.max(0.0);
    let outset = match stroke.align {
        SceneStrokeAlign::Inside => 0.0,
        SceneStrokeAlign::Center => width * 0.5,
        SceneStrokeAlign::Outside => width,
    };
    outset_rect(bounds, outset)
}

fn finish_layer_bounds(
    doc_bounds: BoundsResult,
    viewport_origin: Point2D,
    zoom: f32,
    cull: Rect,
    transforms: &[OverlayTransform],
) -> Rect {
    let cull = match finite_normalized_rect(cull) {
        Ok(Some(cull)) => cull,
        _ => return Rect::xywh(0.0, 0.0, MIN_LAYER_EXTENT, MIN_LAYER_EXTENT),
    };
    let cull = inverse_transform_rect(cull, transforms).unwrap_or(cull);
    let doc_bounds = match doc_bounds {
        Ok(Some(bounds)) => bounds,
        Ok(None) => return minimum_rect_inside(cull),
        Err(()) => return cull,
    };
    let world = match doc_to_world_rect(doc_bounds, viewport_origin, zoom)
        .and_then(|bounds| outset_rect(bounds, DEVICE_EDGE_PAD))
    {
        Ok(bounds) => bounds,
        Err(()) => return cull,
    };
    intersect_rects(world, cull).unwrap_or_else(|| minimum_rect_inside(cull))
}

fn finite_normalized_rect(rect: Rect) -> Result<Option<Rect>, ()> {
    let values = [rect.origin.x, rect.origin.y, rect.size.x, rect.size.y];
    if values.iter().any(|value| !value.is_finite()) {
        return Err(());
    }
    let x2 = rect.origin.x + rect.size.x;
    let y2 = rect.origin.y + rect.size.y;
    if !x2.is_finite() || !y2.is_finite() {
        return Err(());
    }
    let min_x = rect.origin.x.min(x2);
    let min_y = rect.origin.y.min(y2);
    let max_x = rect.origin.x.max(x2);
    let max_y = rect.origin.y.max(y2);
    if max_x <= min_x && max_y <= min_y {
        return Ok(None);
    }
    Ok(Some(Rect::xywh(min_x, min_y, max_x - min_x, max_y - min_y)))
}

fn union_results(left: BoundsResult, right: BoundsResult) -> BoundsResult {
    union_options(left?, right?)
}

fn union_options(left: Option<Rect>, right: Option<Rect>) -> Result<Option<Rect>, ()> {
    match (left, right) {
        (None, None) => Ok(None),
        (Some(rect), None) | (None, Some(rect)) => Ok(Some(rect)),
        (Some(left), Some(right)) => {
            let min_x = left.origin.x.min(right.origin.x);
            let min_y = left.origin.y.min(right.origin.y);
            let max_x = (left.origin.x + left.size.x).max(right.origin.x + right.size.x);
            let max_y = (left.origin.y + left.size.y).max(right.origin.y + right.size.y);
            finite_normalized_rect(Rect::xywh(min_x, min_y, max_x - min_x, max_y - min_y))
        }
    }
}

fn outset_rect(rect: Rect, amount: f32) -> Result<Rect, ()> {
    if !amount.is_finite() || amount < 0.0 {
        return Err(());
    }
    let result = Rect::xywh(
        rect.origin.x - amount,
        rect.origin.y - amount,
        rect.size.x + amount * 2.0,
        rect.size.y + amount * 2.0,
    );
    finite_normalized_rect(result)?.ok_or(())
}

fn translate_rect(rect: Rect, dx: f32, dy: f32) -> Result<Rect, ()> {
    let result = Rect::xywh(
        rect.origin.x + dx,
        rect.origin.y + dy,
        rect.size.x,
        rect.size.y,
    );
    finite_normalized_rect(result)?.ok_or(())
}

fn transformed_aabb(
    rect: Rect,
    flip_x: bool,
    flip_y: bool,
    radians: f32,
    pivot: Point2D,
) -> Result<Rect, ()> {
    if !radians.is_finite() || !pivot.x.is_finite() || !pivot.y.is_finite() {
        return Err(());
    }
    let (sin, cos) = radians.sin_cos();
    let transform = |x: f32, y: f32| {
        // Match `paint_node_inner`: mirror about the aggregate-bounds pivot,
        // then rotate about that same pivot. The root blend caller skips this
        // because its current canvas transform already applies both.
        let dx = (x - pivot.x) * if flip_x { -1.0 } else { 1.0 };
        let dy = (y - pivot.y) * if flip_y { -1.0 } else { 1.0 };
        Point2D::new(pivot.x + dx * cos - dy * sin, pivot.y + dx * sin + dy * cos)
    };
    let x2 = rect.origin.x + rect.size.x;
    let y2 = rect.origin.y + rect.size.y;
    let points = [
        transform(rect.origin.x, rect.origin.y),
        transform(x2, rect.origin.y),
        transform(x2, y2),
        transform(rect.origin.x, y2),
    ];
    if points
        .iter()
        .any(|point| !point.x.is_finite() || !point.y.is_finite())
    {
        return Err(());
    }
    let min_x = points
        .iter()
        .map(|point| point.x)
        .fold(f32::INFINITY, f32::min);
    let min_y = points
        .iter()
        .map(|point| point.y)
        .fold(f32::INFINITY, f32::min);
    let max_x = points
        .iter()
        .map(|point| point.x)
        .fold(f32::NEG_INFINITY, f32::max);
    let max_y = points
        .iter()
        .map(|point| point.y)
        .fold(f32::NEG_INFINITY, f32::max);
    finite_normalized_rect(Rect::xywh(min_x, min_y, max_x - min_x, max_y - min_y))?.ok_or(())
}

/// `cull` arrives in page/screen coordinates while a save-layer bound is
/// interpreted in the canvas's current coordinate system. Undo the active
/// flip/rotation chain (child first) before intersecting the two. Taking an
/// AABB after each inverse step is conservative for nested rotations.
fn inverse_transform_rect(mut rect: Rect, transforms: &[OverlayTransform]) -> Result<Rect, ()> {
    for transform in transforms.iter().rev() {
        if !transform.rotation.is_finite()
            || !transform.pivot.x.is_finite()
            || !transform.pivot.y.is_finite()
        {
            return Err(());
        }
        let (sin, cos) = transform.rotation.sin_cos();
        let inverse = |x: f32, y: f32| {
            let dx = x - transform.pivot.x;
            let dy = y - transform.pivot.y;
            // Forward order is flip then rotation. Inverse order is the
            // opposite: rotate back, then apply the self-inverse mirrors.
            let rx = dx * cos + dy * sin;
            let ry = -dx * sin + dy * cos;
            Point2D::new(
                transform.pivot.x + rx * if transform.flip_x { -1.0 } else { 1.0 },
                transform.pivot.y + ry * if transform.flip_y { -1.0 } else { 1.0 },
            )
        };
        let x2 = rect.origin.x + rect.size.x;
        let y2 = rect.origin.y + rect.size.y;
        let points = [
            inverse(rect.origin.x, rect.origin.y),
            inverse(x2, rect.origin.y),
            inverse(x2, y2),
            inverse(rect.origin.x, y2),
        ];
        if points
            .iter()
            .any(|point| !point.x.is_finite() || !point.y.is_finite())
        {
            return Err(());
        }
        let min_x = points
            .iter()
            .map(|point| point.x)
            .fold(f32::INFINITY, f32::min);
        let min_y = points
            .iter()
            .map(|point| point.y)
            .fold(f32::INFINITY, f32::min);
        let max_x = points
            .iter()
            .map(|point| point.x)
            .fold(f32::NEG_INFINITY, f32::max);
        let max_y = points
            .iter()
            .map(|point| point.y)
            .fold(f32::NEG_INFINITY, f32::max);
        rect = finite_normalized_rect(Rect::xywh(min_x, min_y, max_x - min_x, max_y - min_y))?
            .ok_or(())?;
    }
    Ok(rect)
}

fn doc_to_world_rect(rect: Rect, viewport_origin: Point2D, zoom: f32) -> Result<Rect, ()> {
    if !viewport_origin.x.is_finite()
        || !viewport_origin.y.is_finite()
        || !zoom.is_finite()
        || zoom <= 0.0
    {
        return Err(());
    }
    finite_normalized_rect(Rect::xywh(
        viewport_origin.x + rect.origin.x * zoom,
        viewport_origin.y + rect.origin.y * zoom,
        rect.size.x * zoom,
        rect.size.y * zoom,
    ))?
    .ok_or(())
}

fn intersect_rects(left: Rect, right: Rect) -> Option<Rect> {
    let min_x = left.origin.x.max(right.origin.x);
    let min_y = left.origin.y.max(right.origin.y);
    let max_x = (left.origin.x + left.size.x).min(right.origin.x + right.size.x);
    let max_y = (left.origin.y + left.size.y).min(right.origin.y + right.size.y);
    (max_x > min_x && max_y > min_y).then(|| Rect::xywh(min_x, min_y, max_x - min_x, max_y - min_y))
}

fn minimum_rect_inside(cull: Rect) -> Rect {
    Rect::xywh(
        cull.origin.x,
        cull.origin.y,
        cull.size.x.clamp(f32::EPSILON, MIN_LAYER_EXTENT),
        cull.size.y.clamp(f32::EPSILON, MIN_LAYER_EXTENT),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_scene::{BlurEffect, DropShadow};
    use crate::Color;

    fn rect(id: &str, bounds: Rect) -> SceneNode {
        let mut node = SceneNode::leaf(id, NodeKind::Rect);
        node.bounds = bounds;
        node.fill = Some(Color::RED);
        node
    }

    #[test]
    fn node_bounds_union_descendants_then_transform_to_viewport() {
        let mut root = rect("root", Rect::xywh(100.0, 200.0, 20.0, 10.0));
        root.kind = NodeKind::Frame;
        root.children = vec![rect("child", Rect::xywh(140.0, 190.0, 5.0, 5.0))];
        assert_eq!(
            node_composite_layer_bounds(
                &root,
                Point2D::new(10.0, 20.0),
                2.0,
                Rect::xywh(0.0, 0.0, 1_000.0, 1_000.0),
                None,
                &[],
            ),
            Rect::xywh(209.0, 399.0, 92.0, 42.0)
        );
    }

    #[test]
    fn shadow_and_layer_blur_expand_completed_subtree() {
        let mut node = rect("effects", Rect::xywh(10.0, 10.0, 20.0, 20.0));
        node.effects = vec![
            Effect::DropShadow(DropShadow {
                offset_x: 10.0,
                offset_y: -5.0,
                blur: 8.0,
                color: Color::BLACK,
                inner: false,
            }),
            Effect::Blur(BlurEffect { radius: 4.0 }),
        ];
        assert_eq!(
            node_composite_layer_bounds(
                &node,
                Point2D::ZERO,
                1.0,
                Rect::xywh(-100.0, -100.0, 1_000.0, 1_000.0),
                None,
                &[],
            ),
            Rect::xywh(1.0, -14.0, 58.0, 58.0)
        );
    }

    fn horizontally_flipped_shadow_node(id: &str) -> SceneNode {
        let mut node = rect(id, Rect::xywh(0.0, 0.0, 10.0, 10.0));
        node.flip_x = true;
        node.effects = vec![Effect::DropShadow(DropShadow {
            offset_x: 20.0,
            offset_y: 0.0,
            blur: 0.0,
            color: Color::BLACK,
            inner: false,
        })];
        node
    }

    #[test]
    fn descendant_flip_mirrors_offset_shadow_before_parent_layer_bounds() {
        let mut root = SceneNode::leaf("root", NodeKind::Group);
        root.children = vec![horizontally_flipped_shadow_node("child")];
        assert_eq!(
            node_composite_layer_bounds(
                &root,
                Point2D::ZERO,
                1.0,
                Rect::xywh(-100.0, -100.0, 500.0, 500.0),
                None,
                &[],
            ),
            Rect::xywh(-21.0, -1.0, 32.0, 12.0)
        );
    }

    #[test]
    fn mask_root_flip_mirrors_offset_shadow_in_mask_layer_bounds() {
        let mask = horizontally_flipped_shadow_node("mask");
        assert_eq!(
            sibling_mask_layer_bounds(
                &mask,
                std::iter::empty(),
                Point2D::ZERO,
                1.0,
                Rect::xywh(-100.0, -100.0, 500.0, 500.0),
                None,
                &[],
            ),
            Rect::xywh(-21.0, -1.0, 32.0, 12.0)
        );
    }

    #[test]
    fn mask_bounds_union_only_supplied_run_and_intersect_cull() {
        let mask = rect("mask", Rect::xywh(0.0, 0.0, 10.0, 10.0));
        let content = rect("content", Rect::xywh(20.0, 0.0, 10.0, 10.0));
        assert_eq!(
            sibling_mask_layer_bounds(
                &mask,
                [&content],
                Point2D::ZERO,
                1.0,
                Rect::xywh(0.0, 0.0, 25.0, 100.0),
                None,
                &[],
            ),
            Rect::xywh(0.0, 0.0, 25.0, 11.0)
        );
    }

    #[test]
    fn rotated_ancestor_cull_is_inverse_mapped_before_intersection() {
        let node = rect("nested", Rect::xywh(10.0, 0.0, 10.0, 10.0));
        let ancestor = OverlayTransform {
            rotation: std::f32::consts::FRAC_PI_2,
            flip_x: false,
            flip_y: false,
            pivot: Point2D::ZERO,
        };
        // The node's padded local bounds are (9,-1)..(21,11). After a
        // quarter turn they occupy (-11,9)..(1,21). This visible global
        // slice maps back to local (10,0)..(20,5), rather than appearing
        // disjoint when compared without undoing the ancestor rotation.
        let actual = node_composite_layer_bounds(
            &node,
            Point2D::ZERO,
            1.0,
            Rect::xywh(-5.0, 10.0, 5.0, 10.0),
            None,
            &[ancestor],
        );
        let expected = Rect::xywh(10.0, 0.0, 10.0, 5.0);
        for (actual, expected) in [
            (actual.origin.x, expected.origin.x),
            (actual.origin.y, expected.origin.y),
            (actual.size.x, expected.size.x),
            (actual.size.y, expected.size.y),
        ] {
            assert!((actual - expected).abs() < 0.0001, "{actual} != {expected}");
        }
    }

    #[test]
    fn invalid_imported_geometry_falls_back_to_finite_cull() {
        let mut node = rect("bad", Rect::xywh(f32::NAN, 0.0, 10.0, 10.0));
        node.effects = vec![Effect::Blur(BlurEffect {
            radius: f32::INFINITY,
        })];
        let cull = Rect::xywh(5.0, 6.0, 700.0, 500.0);
        assert_eq!(
            node_composite_layer_bounds(&node, Point2D::ZERO, 1.0, cull, None, &[]),
            cull
        );
    }
}
