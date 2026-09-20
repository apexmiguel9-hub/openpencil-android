//! The box family — frames, rectangles, groups and ellipses — plus
//! lines.
//!
//! Unlike the HTML exporter, shapes here are NOT nested. DrawingML's
//! group shape does not clip its children and gives no advantage in
//! placement (a group needs a child coordinate system stated twice), so
//! the walk flattens the scene into one list of absolutely placed
//! shapes in paint order. Document order in `<p:spTree>` IS paint order,
//! which is the same contract the DOM has, so the ordering logic
//! carries over unchanged.
//!
//! One consequence is worth stating: a container that paints nothing of
//! its own — a group, or a transparent layout frame — emits no shape at
//! all. Its children are already positioned absolutely, so the wrapper
//! would be an empty rectangle in the selection pane and nothing else.

use op_editor_ui::layout_scene::{SceneNode, SceneStroke, SceneStrokeAlign};
use op_editor_ui::{Color, Point2D, Rect};
use std::fmt::Write as _;

use super::units::solid_fill;
use super::xml::{
    effect_list, fill_element, filled_rect, line_element, nv_sp_pr, prst_geom, sp_pr, xfrm, Geom,
    EMPTY_TX_BODY,
};

/// Default stroke for a Line node that authored none (canvas parity).
const DEFAULT_LINE_WIDTH: f32 = 1.5;

/// Whether the node paints anything of its own.
///
/// A `false` here is what keeps a deck from carrying a hundred invisible
/// rectangles: the walker skips the shape and goes straight to the
/// children.
pub fn paints_anything(node: &SceneNode) -> bool {
    node.fill.is_some()
        || node.image_src.is_some()
        || node.gradient.is_some()
        || node.stroke.is_some_and(stroke_paints)
        || !node.effects.is_empty()
}

fn stroke_paints(stroke: SceneStroke) -> bool {
    match stroke.sides {
        Some(sides) => sides.iter().any(|w| *w > 0.0),
        None => stroke.width > 0.0,
    }
}

/// Emit the node's own box. `next_id` is advanced once per shape written
/// (a per-side stroke costs one extra shape per painted side).
pub fn emit_box(
    out: &mut String,
    node: &SceneNode,
    rect: Rect,
    alpha: f32,
    geom: Geom,
    next_id: &mut u32,
) {
    let uniform = uniform_stroke(node.stroke);
    let line = uniform.and_then(|s| line_element(s, alpha));
    let id = take(next_id);
    let _ = write!(
        out,
        "<p:sp>{}{}{}</p:sp>",
        nv_sp_pr(id, &node.id, false),
        sp_pr(
            &xfrm(rect, node),
            &prst_geom(node, geom, rect),
            &fill_element(node, alpha),
            line.as_deref(),
            &effect_list(node, alpha)
        ),
        EMPTY_TX_BODY
    );
    if uniform.is_none() {
        if let Some(stroke) = node.stroke {
            emit_side_strokes(out, node, rect, stroke, alpha, next_id);
        }
    }
}

/// The stroke as a single uniform band, or `None` when the sides differ.
fn uniform_stroke(stroke: Option<SceneStroke>) -> Option<SceneStroke> {
    let stroke = stroke?;
    match stroke.sides {
        None => (stroke.width > 0.0).then_some(stroke),
        Some([t, r, b, l]) => {
            let even = (r - t).abs() < 0.01 && (b - t).abs() < 0.01 && (l - t).abs() < 0.01;
            (even && t > 0.0).then_some(SceneStroke { width: t, ..stroke })
        }
    }
}

/// Draw a per-side stroke as up to four filled bands.
///
/// `<a:ln>` has one width for the whole outline, so the two obvious
/// options for a bottom-only divider are both wrong: a full outline puts
/// a box where the design has a rule, and rasterising the node turns its
/// entire subtree — headings, labels, the lot — into pixels to render
/// one hairline. A band is a rectangle at the exact place the canvas
/// strokes, it costs one shape per painted side, and everything inside
/// the container stays live text.
fn emit_side_strokes(
    out: &mut String,
    node: &SceneNode,
    rect: Rect,
    stroke: SceneStroke,
    alpha: f32,
    next_id: &mut u32,
) {
    let Some([top, right, bottom, left]) = stroke.sides else {
        return;
    };
    let outset = |width: f32| match stroke.align {
        SceneStrokeAlign::Inside => 0.0,
        SceneStrokeAlign::Center => width * 0.5,
        SceneStrokeAlign::Outside => width,
    };
    let (x, y, w, h) = (
        rect.origin.x,
        rect.origin.y,
        rect.size.x.max(0.0),
        rect.size.y.max(0.0),
    );
    let bands = [
        (top, Rect::xywh(x, y - outset(top), w, top)),
        (
            right,
            Rect::xywh(x + w - right + outset(right), y, right, h),
        ),
        (
            bottom,
            Rect::xywh(x, y + h - bottom + outset(bottom), w, bottom),
        ),
        (left, Rect::xywh(x - outset(left), y, left, h)),
    ];
    for (width, band) in bands {
        if width <= 0.0 || band.size.x <= 0.0 || band.size.y <= 0.0 {
            continue;
        }
        out.push_str(&filled_rect(
            take(next_id),
            &node.id,
            band,
            stroke.color,
            alpha,
        ));
    }
}

/// Emit a Line node as a straight connector.
///
/// `origin` is the board origin: the endpoints come from the node's
/// SIGNED bounds (a line running up-and-left has a negative extent), and
/// direction is carried by `flipH` / `flipV` rather than by normalising
/// the rect, which would silently reverse the line.
pub fn emit_line(
    out: &mut String,
    node: &SceneNode,
    origin: Point2D,
    alpha: f32,
    next_id: &mut u32,
) {
    let (color, width) = match node.stroke {
        Some(s) if s.width > 0.0 => (s.color, s.width),
        _ => (node.fill.unwrap_or(Color::BLACK), DEFAULT_LINE_WIDTH),
    };
    let start = Point2D::new(
        node.bounds.origin.x - origin.x,
        node.bounds.origin.y - origin.y,
    );
    let (dx, dy) = (node.bounds.size.x, node.bounds.size.y);
    let box_rect = Rect::xywh(
        start.x.min(start.x + dx),
        start.y.min(start.y + dy),
        dx.abs(),
        dy.abs(),
    );
    let mut attrs = String::new();
    if (dx < 0.0) != node.flip_x {
        attrs.push_str(" flipH=\"1\"");
    }
    if (dy < 0.0) != node.flip_y {
        attrs.push_str(" flipV=\"1\"");
    }
    let rot = super::units::rot_60k(node.rotation);
    if rot != 0 {
        attrs = format!(" rot=\"{rot}\"{attrs}");
    }
    let stroke = SceneStroke {
        color,
        width,
        sides: None,
        align: SceneStrokeAlign::Center,
    };
    let _ = write!(
        out,
        "<p:cxnSp><p:nvCxnSpPr><p:cNvPr id=\"{}\" name=\"{}\"/><p:cNvCxnSpPr/><p:nvPr/>\
</p:nvCxnSpPr><p:spPr>{}<a:prstGeom prst=\"line\"><a:avLst/></a:prstGeom>{}</p:spPr>\
</p:cxnSp>",
        take(next_id),
        op_util::xml_escape::escape_xml(&node.id),
        line_xfrm(box_rect, &attrs),
        line_element(stroke, alpha).unwrap_or_else(|| solid_fill(color, alpha))
    );
}

/// A connector's own `<a:xfrm>`.
///
/// It does not go through `xml::xfrm_plain` because a line legitimately
/// has a zero extent on one axis — a horizontal rule is exactly
/// `cy="0"` — while an area shape's extent is floored to one EMU so it
/// cannot vanish.
fn line_xfrm(rect: Rect, attrs: &str) -> String {
    use super::units::emu;
    format!(
        "<a:xfrm{attrs}><a:off x=\"{}\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"{}\"/></a:xfrm>",
        emu(rect.origin.x),
        emu(rect.origin.y),
        emu(rect.size.x).max(0),
        emu(rect.size.y).max(0)
    )
}

fn take(next_id: &mut u32) -> u32 {
    let id = *next_id;
    *next_id += 1;
    id
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_editor_ui::layout_scene::NodeKind;

    fn rect_node() -> SceneNode {
        let mut n = SceneNode::leaf("r1", NodeKind::Rect);
        n.bounds = Rect::xywh(0.0, 0.0, 100.0, 40.0);
        n.fill = Some(Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        });
        n
    }

    #[test]
    fn an_empty_group_paints_nothing_and_is_skipped() {
        let group = SceneNode::leaf("g1", NodeKind::Group);
        assert!(!paints_anything(&group));
        assert!(paints_anything(&rect_node()));
    }

    #[test]
    fn a_uniform_stroke_rides_on_the_shape_itself() {
        let mut n = rect_node();
        n.stroke = Some(SceneStroke {
            color: Color::BLACK,
            width: 2.0,
            sides: None,
            align: SceneStrokeAlign::Inside,
        });
        let mut out = String::new();
        let mut id = 2;
        emit_box(&mut out, &n, n.bounds, 1.0, Geom::Rect, &mut id);
        assert!(out.contains("<a:ln w=\"19050\""), "{out}");
        assert_eq!(id, 3, "one shape only");
    }

    #[test]
    fn a_bottom_only_border_becomes_a_band_not_a_full_outline() {
        let mut n = rect_node();
        n.stroke = Some(SceneStroke {
            color: Color::BLACK,
            width: 1.0,
            sides: Some([0.0, 0.0, 1.0, 0.0]),
            align: SceneStrokeAlign::Inside,
        });
        let mut out = String::new();
        let mut id = 2;
        emit_box(&mut out, &n, n.bounds, 1.0, Geom::Rect, &mut id);
        assert!(!out.contains("<a:ln "), "no full outline: {out}");
        assert_eq!(id, 4, "the box plus one band");
        // The band sits on the bottom edge: y = 40 - 1 px.
        assert!(
            out.contains(&format!("y=\"{}\"", super::super::units::emu(39.0))),
            "{out}"
        );
    }

    #[test]
    fn an_evenly_specified_per_side_stroke_is_still_one_outline() {
        let mut n = rect_node();
        n.stroke = Some(SceneStroke {
            color: Color::BLACK,
            width: 0.0,
            sides: Some([2.0, 2.0, 2.0, 2.0]),
            align: SceneStrokeAlign::Inside,
        });
        let mut out = String::new();
        let mut id = 2;
        emit_box(&mut out, &n, n.bounds, 1.0, Geom::Rect, &mut id);
        assert!(out.contains("<a:ln w=\"19050\""), "{out}");
        assert_eq!(id, 3);
    }

    #[test]
    fn a_line_running_backwards_keeps_its_direction_through_a_flip() {
        let mut n = SceneNode::leaf("l1", NodeKind::Line);
        n.bounds = Rect::xywh(100.0, 50.0, -60.0, 0.0);
        n.stroke = Some(SceneStroke {
            color: Color::BLACK,
            width: 2.0,
            sides: None,
            align: SceneStrokeAlign::Center,
        });
        let mut out = String::new();
        let mut id = 2;
        emit_line(&mut out, &n, Point2D::new(0.0, 0.0), 1.0, &mut id);
        assert!(out.contains("flipH=\"1\""), "{out}");
        assert!(out.contains("<p:cxnSp>"), "{out}");
        // The box starts at the LEFT end (40), not at the authored x.
        assert!(
            out.contains(&format!("x=\"{}\"", super::super::units::emu(40.0))),
            "{out}"
        );
    }
}
