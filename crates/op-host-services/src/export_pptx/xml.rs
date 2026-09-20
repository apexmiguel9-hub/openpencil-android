//! Shared DrawingML fragments — the parts of a shape that read the same
//! whether the shape came out of a frame, an ellipse or a picture.
//!
//! Element ORDER is part of the schema here, not a style choice: a
//! `<p:spPr>` must list `xfrm`, then geometry, then fill, then line,
//! then effects, and PowerPoint rejects the file outright if they are
//! written in any other order. Every builder in this module emits its
//! own fragment only; [`sp_pr`] is the one place that concatenates them,
//! so the order is stated once.

use op_editor_ui::layout_scene::{
    SceneFillType, SceneGradient, SceneGradientStop, SceneNode, SceneStroke,
};
use op_editor_ui::{Color, Rect};
use op_util::xml_escape::escape_xml;

use super::units::{color_element, degrees_60k, emu, emu_extent, pct_1000, rot_60k, solid_fill};

/// The preset shape a node maps onto.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Geom {
    Rect,
    Ellipse,
}

/// `<p:nvSpPr>` — the identity block every shape opens with.
///
/// `name` is diagnostic only (it is what the PowerPoint selection pane
/// shows), so it carries the scene node id: a slide that looks wrong is
/// then traceable back to a node without re-running the export.
pub fn nv_sp_pr(id: u32, node_id: &str, text_box: bool) -> String {
    let cnv = if text_box {
        "<p:cNvSpPr txBox=\"1\"/>"
    } else {
        "<p:cNvSpPr/>"
    };
    format!(
        "<p:nvSpPr><p:cNvPr id=\"{id}\" name=\"{}\"/>{cnv}<p:nvPr/></p:nvSpPr>",
        escape_xml(node_id)
    )
}

/// `<a:xfrm>` — placement, rotation and mirroring in one element.
pub fn xfrm(rect: Rect, node: &SceneNode) -> String {
    let mut attrs = String::new();
    let rot = rot_60k(node.rotation);
    if rot != 0 {
        attrs.push_str(&format!(" rot=\"{rot}\""));
    }
    if node.flip_x {
        attrs.push_str(" flipH=\"1\"");
    }
    if node.flip_y {
        attrs.push_str(" flipV=\"1\"");
    }
    plain_xfrm(rect, &attrs)
}

/// `<a:xfrm>` for a rect with no rotation or mirroring of its own.
pub fn xfrm_plain(rect: Rect) -> String {
    plain_xfrm(rect, "")
}

fn plain_xfrm(rect: Rect, attrs: &str) -> String {
    format!(
        "<a:xfrm{attrs}><a:off x=\"{}\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"{}\"/></a:xfrm>",
        emu(rect.origin.x),
        emu(rect.origin.y),
        emu_extent(rect.size.x),
        emu_extent(rect.size.y)
    )
}

/// `<a:prstGeom>` for a node.
///
/// A corner radius becomes `roundRect`, whose single `adj` value is a
/// fraction of HALF the shorter side — DrawingML has no per-corner
/// preset at all. When the four scene radii disagree, the top-left one
/// is used for all four and the difference is lost; the alternative
/// (rasterising every card with one squared corner) would cost the whole
/// subtree's live text to save one corner.
pub fn prst_geom(node: &SceneNode, geom: Geom, rect: Rect) -> String {
    match geom {
        Geom::Ellipse => "<a:prstGeom prst=\"ellipse\"><a:avLst/></a:prstGeom>".to_string(),
        Geom::Rect => match corner_radius(node) {
            Some(radius) => {
                let shorter = rect.size.x.min(rect.size.y).max(0.01);
                let adj = pct_1000((radius / shorter).clamp(0.0, 0.5));
                format!(
                    "<a:prstGeom prst=\"roundRect\"><a:avLst>\
<a:gd name=\"adj\" fmla=\"val {adj}\"/></a:avLst></a:prstGeom>"
                )
            }
            None => "<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom>".to_string(),
        },
    }
}

/// The node's effective corner radius in doc px, or `None` when it is
/// square-cornered. Per-corner radii collapse to the top-left value.
pub fn corner_radius(node: &SceneNode) -> Option<f32> {
    if let Some([tl, tr, br, bl]) = node.corner_radii {
        let first = [tl, tr, br, bl].into_iter().find(|r| *r > 0.0)?;
        return Some(first);
    }
    (node.corner_radius > 0.0).then_some(node.corner_radius)
}

/// The node's fill as a DrawingML fill element.
///
/// Returns `<a:noFill/>` for an unfilled node rather than nothing at
/// all: an omitted fill element means "inherit from the theme's style
/// matrix", which would paint a blue box under a shape the canvas draws
/// as empty.
pub fn fill_element(node: &SceneNode, alpha: f32) -> String {
    if let (SceneFillType::LinearGradient | SceneFillType::RadialGradient, Some(gradient)) =
        (node.fill_type, node.gradient.as_ref())
    {
        if let Some(value) = grad_fill(gradient, alpha) {
            return value;
        }
    }
    match node.fill {
        Some(color) => solid_fill(color, alpha),
        None => "<a:noFill/>".to_string(),
    }
}

/// A resolved gradient as `<a:gradFill>`, or `None` when it has too few
/// stops to be one (DrawingML requires at least two).
///
/// **Linear angles are exact.** The canonical `.op` convention is CSS's
/// (0° = bottom→top, growing clockwise) while DrawingML measures
/// clockwise from due east, so the conversion is a fixed −90° turn.
///
/// **Radial extent is not.** The scene states a radius as a fraction of
/// the box; `<a:path path="circle">` always runs the ramp from the focus
/// point out to the shape's edge, so a gradient authored to finish at
/// 50% is stretched to finish at 100%. The centre — the thing the eye
/// actually locates — is exact, via `fillToRect`.
pub fn grad_fill(gradient: &SceneGradient, alpha: f32) -> Option<String> {
    let (stops, opacity, tail) = match gradient {
        SceneGradient::Linear {
            angle_deg,
            opacity,
            stops,
        } => (
            stops,
            *opacity,
            format!(
                "<a:lin ang=\"{}\" scaled=\"0\"/>",
                degrees_60k(angle_deg - 90.0)
            ),
        ),
        SceneGradient::Radial {
            cx,
            cy,
            opacity,
            stops,
            ..
        } => {
            let (cx, cy) = (cx.clamp(0.0, 1.0), cy.clamp(0.0, 1.0));
            (
                stops,
                *opacity,
                format!(
                    "<a:path path=\"circle\"><a:fillToRect l=\"{}\" t=\"{}\" r=\"{}\" b=\"{}\"/>\
</a:path>",
                    pct_1000(cx),
                    pct_1000(cy),
                    pct_1000(1.0 - cx),
                    pct_1000(1.0 - cy)
                ),
            )
        }
        // A Gouraud lattice has no DrawingML spelling; the caller sends
        // the node to the raster path instead of flattening it.
        SceneGradient::Mesh { .. } => return None,
    };
    if stops.len() < 2 {
        return None;
    }
    Some(format!(
        "<a:gradFill flip=\"none\" rotWithShape=\"1\"><a:gsLst>{}</a:gsLst>{tail}</a:gradFill>",
        stop_list(stops, opacity * alpha)
    ))
}

fn stop_list(stops: &[SceneGradientStop], alpha: f32) -> String {
    let mut out = String::new();
    // Positions must not decrease; the scene can carry an authored stop
    // list that does, and PowerPoint rejects the part rather than
    // sorting it for us.
    let mut floor = 0.0f32;
    for stop in stops {
        let offset = stop.offset.clamp(0.0, 1.0).max(floor);
        floor = offset;
        out.push_str(&format!(
            "<a:gs pos=\"{}\">{}</a:gs>",
            pct_1000(offset),
            color_element(stop.color, alpha)
        ));
    }
    out
}

/// `<a:ln>` for a uniform stroke, or `None` when the node has none.
///
/// Per-side strokes do NOT come through here — see
/// `shape::emit_side_strokes` for why they are drawn as their own
/// rectangles instead.
///
/// DrawingML centres a line on the shape outline and has no alignment
/// property, so an `Inside` or `Outside` stroke lands half its width off
/// from where the canvas puts it. At the 1–2 px strokes decks use, that
/// is a sub-pixel difference on a projected slide, and the alternative —
/// insetting the shape to compensate — would move the FILL edge too.
pub fn line_element(stroke: SceneStroke, alpha: f32) -> Option<String> {
    if stroke.width <= 0.0 || !stroke.width.is_finite() {
        return None;
    }
    Some(format!(
        "<a:ln w=\"{}\" cap=\"flat\">{}<a:prstDash val=\"solid\"/></a:ln>",
        emu_extent(stroke.width),
        solid_fill(stroke.color, alpha)
    ))
}

/// `<a:effectLst>` for the node's shadows, or an empty string.
///
/// Blur effects are absent by construction: a node carrying one never
/// reaches this function, because `filter: blur()` and DrawingML's
/// `a:blur` are different operations and the raster path reproduces the
/// canvas exactly.
pub fn effect_list(node: &SceneNode, alpha: f32) -> String {
    use op_editor_ui::layout_scene::Effect;

    let mut body = String::new();
    for effect in &node.effects {
        let Effect::DropShadow(shadow) = effect else {
            continue;
        };
        let blur = emu(shadow.blur.max(0.0));
        let dist = emu((shadow.offset_x.powi(2) + shadow.offset_y.powi(2)).sqrt());
        let dir = degrees_60k(shadow.offset_y.atan2(shadow.offset_x).to_degrees());
        let color = color_element(shadow.color, alpha);
        if shadow.inner {
            body.push_str(&format!(
                "<a:innerShdw blurRad=\"{blur}\" dist=\"{dist}\" dir=\"{dir}\">{color}\
</a:innerShdw>"
            ));
        } else {
            body.push_str(&format!(
                "<a:outerShdw blurRad=\"{blur}\" dist=\"{dist}\" dir=\"{dir}\" \
rotWithShape=\"0\">{color}</a:outerShdw>"
            ));
        }
    }
    if body.is_empty() {
        String::new()
    } else {
        format!("<a:effectLst>{body}</a:effectLst>")
    }
}

/// `<p:spPr>` with its children in the order the schema demands.
pub fn sp_pr(xfrm: &str, geom: &str, fill: &str, line: Option<&str>, effects: &str) -> String {
    format!(
        "<p:spPr>{xfrm}{geom}{fill}{}{effects}</p:spPr>",
        line.unwrap_or("")
    )
}

/// A shape carrying no text still needs a `<p:txBody>`: PowerPoint
/// treats a `<p:sp>` without one as malformed even though the schema
/// marks it optional.
pub const EMPTY_TX_BODY: &str = "<p:txBody><a:bodyPr/><a:lstStyle/><a:p/></p:txBody>";

/// A flat filled rectangle with no stroke, geometry adjustment or
/// effects — the primitive behind per-side stroke bands.
pub fn filled_rect(id: u32, node_id: &str, rect: Rect, color: Color, alpha: f32) -> String {
    format!(
        "<p:sp>{}{}{}</p:sp>",
        nv_sp_pr(id, node_id, false),
        sp_pr(
            &xfrm_plain(rect),
            "<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom>",
            &solid_fill(color, alpha),
            None,
            ""
        ),
        EMPTY_TX_BODY
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_editor_ui::layout_scene::NodeKind;
    use op_editor_ui::Point2D;

    fn node() -> SceneNode {
        SceneNode::leaf("n1", NodeKind::Rect)
    }

    fn rect() -> Rect {
        Rect {
            origin: Point2D::new(0.0, 0.0),
            size: Point2D::new(200.0, 100.0),
        }
    }

    #[test]
    fn a_corner_radius_becomes_a_fraction_of_the_shorter_side() {
        let mut n = node();
        n.corner_radius = 25.0;
        let xml = prst_geom(&n, Geom::Rect, rect());
        // 25 / min(200, 100) = 0.25 of the shorter side.
        assert!(xml.contains("val 25000"), "{xml}");
        assert!(xml.contains("roundRect"), "{xml}");
    }

    #[test]
    fn an_unfilled_node_says_so_instead_of_inheriting_a_theme_fill() {
        assert_eq!(fill_element(&node(), 1.0), "<a:noFill/>");
    }

    #[test]
    fn a_css_gradient_angle_becomes_a_drawingml_one() {
        // CSS 0deg points up; DrawingML 0 points right, so straight up
        // is a three-quarter turn clockwise.
        let up = SceneGradient::Linear {
            angle_deg: 0.0,
            opacity: 1.0,
            stops: vec![
                SceneGradientStop {
                    offset: 0.0,
                    color: Color::BLACK,
                },
                SceneGradientStop {
                    offset: 1.0,
                    color: Color::WHITE,
                },
            ],
        };
        let xml = grad_fill(&up, 1.0).expect("two stops");
        assert!(xml.contains("<a:lin ang=\"16200000\""), "{xml}");
        assert!(xml.contains("pos=\"0\""), "{xml}");
        assert!(xml.contains("pos=\"100000\""), "{xml}");
    }

    #[test]
    fn a_one_stop_gradient_is_refused_so_the_caller_can_fall_back() {
        let single = SceneGradient::Linear {
            angle_deg: 90.0,
            opacity: 1.0,
            stops: vec![SceneGradientStop {
                offset: 0.0,
                color: Color::BLACK,
            }],
        };
        assert!(grad_fill(&single, 1.0).is_none());
    }
}
