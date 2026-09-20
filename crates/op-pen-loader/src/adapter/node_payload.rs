//! The `PenNode` -> `NodePayload` dispatcher plus the shared
//! post-processing steps (computed rects, path anchor absolutization,
//! vertical-centre text parity).

use super::*;

pub(crate) fn node_to_payload(node: &PenNode, rects: &BTreeMap<String, [f32; 4]>) -> NodePayload {
    node_to_payload_with_text_context(node, rects, None)
}

pub(super) fn node_to_payload_with_text_context(
    node: &PenNode,
    rects: &BTreeMap<String, [f32; 4]>,
    parent_text_center: Option<TextChildCenterContext>,
) -> NodePayload {
    use crate::widget_payload as wp;
    let mut p = match node {
        PenNode::Frame(n) => frame_to_payload(n, rects),
        PenNode::Group(n) => group_to_payload(n, rects),
        PenNode::Rectangle(n) => rect_to_payload(n, rects),
        PenNode::Ellipse(n) => ellipse_to_payload(n),
        PenNode::Line(n) => line_to_payload(n),
        PenNode::Polygon(n) => polygon_to_payload(n),
        PenNode::Path(n) => path_to_payload(n),
        PenNode::Text(n) => text_to_payload(n),
        PenNode::TextInput(n) => wp::text_input_to_payload(n),
        PenNode::TextArea(n) => wp::text_area_to_payload(n),
        PenNode::Select(n) => wp::select_to_payload(n),
        PenNode::Switch(n) => wp::switch_to_payload(n),
        PenNode::Checkbox(n) => wp::checkbox_to_payload(n),
        PenNode::Slider(n) => wp::slider_to_payload(n),
        PenNode::RadioGroup(n) => wp::radio_group_to_payload(n),
        PenNode::NumberInput(n) => wp::number_input_to_payload(n),
        PenNode::Progress(n) => wp::progress_to_payload(n),
        PenNode::Tabs(n) => wp::tabs_to_payload(n, rects),
        PenNode::Image(n) => image_to_payload(n),
        PenNode::IconFont(n) => icon_font_to_payload(n),
        PenNode::Ref(n) => empty_group(&n.base, "ref"),
    };
    // The line painter is special-cased on signed bounds, so don't
    // overwrite its hand-encoded geometry with the taffy AABB.
    if !matches!(node, PenNode::Line(_)) {
        apply_computed_rect(&mut p, rects);
    } else if let Some([x, y, w, h]) = rects.get(&p.schema_id).copied() {
        if w.is_nan() && h.is_nan() {
            p.x = x;
            p.y = y;
        }
    }
    apply_vertical_center_text_child_parity(node, &mut p, parent_text_center);
    crate::legacy_payload_repair::repair_payload_for_legacy_node(node, &mut p);
    // Canonical `PathNode.anchors` need the same transform the TS
    // renderer applies in `pen-renderer/node-renderer.ts::drawPath`:
    // compute the local geometry bounds (including Bezier handles
    // and cubic curve extrema, per
    // `pen-core/path-anchors.ts::getPathBoundsFromAnchors`), then
    // map each anchor onto canvas-absolute via
    // `(x + (anchor.x - bounds_min_x) * sx, …)`. Endpoint-only
    // bounds are wrong for curved paths because cubic Beziers can
    // extend well past their anchor endpoints.
    if let PenNode::Path(path) = node {
        if !p.points.is_empty() {
            absolutize_path_anchors(&mut p, path);
        }
    }
    // Carry canonical drop-shadow effects across — without this a
    // `.op` authored with shadows lost them on import (codex
    // stop-gate). Gaussian layer blur is carried via `layer_blur`;
    // backdrop blur is carried separately via `background_blur`.
    p.effects = crate::effects::shadows_from_canonical(node);
    p.layer_blur = crate::effects::blur_from_canonical(node);
    p.background_blur = crate::effects::background_blur_from_canonical(node);
    p
}

fn apply_vertical_center_text_child_parity(
    node: &PenNode,
    payload: &mut NodePayload,
    context: Option<TextChildCenterContext>,
) {
    let Some(context) = context else {
        return;
    };
    if !matches!(node, PenNode::Text(_)) || text_has_explicit_non_left_align(node) {
        return;
    }
    payload.text_align = "center".to_string();
    if context.w > 0.0 {
        payload.x = context.x;
        payload.w = context.w;
    }
}

fn text_has_explicit_non_left_align(node: &PenNode) -> bool {
    matches!(
        node,
        PenNode::Text(TextNode {
            text_align: Some(
                jian_ops_schema::node::TextAlign::Center
                    | jian_ops_schema::node::TextAlign::Right
                    | jian_ops_schema::node::TextAlign::Justify,
            ),
            ..
        })
    )
}

/// Translate `p.points` from local-to-`base.x/base.y` into the
/// canvas-absolute frame the shell's path painter expects.
/// Mirrors `pen-renderer/node-renderer.ts::drawPath`:
/// - Local geometry bounds come from `path_bounds_from_anchors`
///   (curve extrema + handle-extended segments), not just the
///   anchor endpoints, so a path whose handles bow well past its
///   endpoints still scales correctly.
/// - Scale = explicit `width`/`height` over native span.
/// - Translate so the local geometry's top-left lands at `(p.x, p.y)`.
pub(super) fn absolutize_path_anchors(p: &mut NodePayload, path: &PathNode) {
    let closed = path.closed.unwrap_or(false);
    let bounds = path_bounds_from_anchors(path.anchors.as_deref().unwrap_or(&[]), closed);
    let (min_x, min_y, native_w, native_h) = bounds;
    let sx = if native_w > 0.01 && p.w > 0.0 {
        p.w / native_w
    } else {
        1.0
    };
    let sy = if native_h > 0.01 && p.h > 0.0 {
        p.h / native_h
    } else {
        1.0
    };
    let (ox, oy) = (p.x, p.y);
    for pt in &mut p.points {
        pt[0] = ox + (pt[0] - min_x) * sx;
        pt[1] = oy + (pt[1] - min_y) * sy;
    }
    // Resolve bezier anchors into the same absolute frame — anchor
    // positions track `points`, handle deltas scale by `(sx, sy)`.
    if let Some(anchors) = &path.anchors {
        p.path_anchors = anchors
            .iter()
            .map(|a| {
                let ax = ox + (a.x as f32 - min_x) * sx;
                let ay = oy + (a.y as f32 - min_y) * sy;
                let resolve = |h: &jian_ops_schema::node::PenPathHandle| {
                    [ax + h.x as f32 * sx, ay + h.y as f32 * sy]
                };
                crate::payload::AnchorPayload {
                    x: ax,
                    y: ay,
                    handle_in: a.handle_in.as_ref().map(resolve),
                    handle_out: a.handle_out.as_ref().map(resolve),
                    point_type: point_type_code(a.point_type.as_ref()),
                }
            })
            .collect();
    }
}

/// Schema point-type → payload code (0 corner / 1 mirrored / 2
/// independent).
fn point_type_code(pt: Option<&jian_ops_schema::node::PenPathPointType>) -> u8 {
    use jian_ops_schema::node::PenPathPointType;
    match pt {
        Some(PenPathPointType::Mirrored) => 1,
        Some(PenPathPointType::Independent) => 2,
        _ => 0,
    }
}

/// Replace `(x, y, w, h)` on `p` with the absolute scene-coord rect
/// the layout engine resolved for this node. Width / height fall
/// back to authored size only when taffy reports `Size::ZERO` (the
/// `leaf_size` resolver covers text / text_input / icon_font /
/// image but returns `(None, None)` for ellipse / polygon / path).
/// Position always uses the layout engine's `(x, y)` so the root's
/// canvas offset propagates onto zero-size shape fallbacks too —
/// otherwise an authored `x=20, y=30` ellipse inside a root at
/// `(-1098, 2963)` would paint at world `(20, 30)` instead of
/// `(-1078, 2993)` and detach from its parent design.
pub(super) fn apply_computed_rect(p: &mut NodePayload, rects: &BTreeMap<String, [f32; 4]>) {
    if let Some([x, y, w, h]) = rects.get(&p.schema_id).copied() {
        p.x = x;
        p.y = y;
        if w > 0.0 {
            p.w = w;
        }
        if h > 0.0 {
            p.h = h;
        }
    }
}

/// Numeric width/height from a schema sizing field. Flex tokens
/// (`fill_container` / `fit_content`) and expressions collapse to
/// 0; jian-core's taffy compute fills those in via the layout map.
pub(super) fn sizing_to_f32(s: &Option<SizingBehavior>) -> f32 {
    match s {
        Some(SizingBehavior::Number(n)) => *n as f32,
        _ => 0.0,
    }
}
