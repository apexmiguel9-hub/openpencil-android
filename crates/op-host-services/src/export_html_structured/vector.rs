//! Emitters for the node kinds a `<div>` cannot describe: icons, and
//! the line / polygon / path family.
//!
//! All three write an inline `<svg>`. Inline means the glyph data ships
//! as markup inside the one file — no icon font to download, no sprite
//! sheet, no request. The `<svg>` box is pinned by the same absolute
//! positioning every other node uses, and its `viewBox` is stated in
//! ABSOLUTE doc coordinates so the geometry the scene already resolved
//! (which is absolute) drops in without a second coordinate change.

use op_editor_ui::layout_scene::{regular_polygon_points, SceneNode};
use op_editor_ui::widgets::icon_catalog::{lookup_icon, IconRenderStyle};
use op_editor_ui::widgets::icons::Icon;
use op_editor_ui::{Color, Point2D, Rect};
use op_util::xml_escape::escape_xml;
use std::fmt::Write as _;

use super::css;

/// Painter default icon tint when the node authored no fill.
const ICON_DEFAULT_FILL: Color = Color {
    r: 0.39,
    g: 0.45,
    b: 0.55,
    a: 1.0,
};

/// Lucide's dot placeholder — the same `d` the canvas strokes for an
/// unresolvable glyph name, so an unknown icon reads identically in the
/// editor and in the exported deck.
const FALLBACK_ICON_D: &str = "M12 12m-3 0a3 3 0 1 0 6 0a3 3 0 1 0 -6 0";

/// Lucide's authoring viewBox.
const LUCIDE_VIEWBOX: f32 = 24.0;

/// Emit an `icon_font` node as an inline SVG.
///
/// `rect` is already board-local. The glyph is centred in a square of
/// `min(w, h)`, matching `icons::paint_icon_font_node`, so an icon in a
/// non-square box sits where the canvas puts it rather than stretching.
pub fn emit_icon(out: &mut String, n: &SceneNode, rect: Rect) {
    let size = rect.size.x.min(rect.size.y);
    if size <= 0.0 {
        return;
    }
    let square = Rect {
        origin: Point2D::new(
            rect.origin.x + (rect.size.x - size) / 2.0,
            rect.origin.y + (rect.size.y - size) / 2.0,
        ),
        size: Point2D::new(size, size),
    };
    let name = n.text.as_deref().unwrap_or("");
    let family = {
        let f = n.font_family.trim();
        if f.is_empty() {
            "lucide"
        } else {
            f
        }
    };
    let tint = css::color(n.fill.unwrap_or(ICON_DEFAULT_FILL));

    let (viewbox, paints) = resolve_glyph(family, name);
    let mut style = String::new();
    css::place(&mut style, square);

    // The canvas strokes at `(size / 24) * 2` doc px against a 24-unit
    // viewBox. In user units that is `viewbox / 12`, independent of the
    // rendered size — which is what makes the icon scale like the
    // canvas does instead of thickening as the slide is projected up.
    let stroke_width = css::num(viewbox / 12.0);
    let _ = write!(
        out,
        r#"<svg class="n" style="{style}" viewBox="0 0 {vb} {vb}" fill="none" stroke-width="{stroke_width}" stroke-linecap="round" stroke-linejoin="round">"#,
        vb = css::num(viewbox)
    );
    for (d, painted) in paints {
        match painted {
            IconRenderStyle::Fill => {
                let _ = write!(
                    out,
                    r#"<path d="{}" fill="{tint}" stroke="none"/>"#,
                    escape_xml(&d)
                );
            }
            IconRenderStyle::Stroke => {
                let _ = write!(out, r#"<path d="{}" stroke="{tint}"/>"#, escape_xml(&d));
            }
        }
    }
    out.push_str("</svg>");
}

/// Resolve a glyph to its viewBox extent plus its `d` strings.
/// Mirrors the lookup ladder in `icons::paint_icon_font_node`:
/// Iconify catalog first, then the chrome's built-in lucide table for
/// the `lucide` collection, then the honest dot.
fn resolve_glyph(family: &str, name: &str) -> (f32, Vec<(String, IconRenderStyle)>) {
    if let Some(entry) = lookup_icon(family, name) {
        let viewbox = entry.width.max(entry.height).max(1.0);
        return (viewbox, vec![(entry.d.clone(), entry.style)]);
    }
    if family == "lucide" {
        if let Some(icon) = Icon::from_name(name) {
            let paths = icon
                .paths()
                .iter()
                .map(|d| ((*d).to_string(), IconRenderStyle::Stroke))
                .collect();
            return (LUCIDE_VIEWBOX, paths);
        }
    }
    (
        LUCIDE_VIEWBOX,
        vec![(FALLBACK_ICON_D.to_string(), IconRenderStyle::Stroke)],
    )
}

/// Emit a Line node as an inline SVG segment.
///
/// A line's `bounds` is a vector, not a box: the segment runs from the
/// origin to `origin + size`, and either component may be zero or
/// negative. The SVG box is therefore the normalised rect padded out by
/// the stroke, which also keeps a perfectly horizontal line (zero
/// height) from collapsing to an unrenderable zero-extent viewport.
pub fn emit_line(out: &mut String, n: &SceneNode, origin: Point2D) {
    let (color, width) = match n.stroke {
        Some(s) => (s.color, s.width),
        None => (n.fill.unwrap_or(Color::BLACK), 1.5),
    };
    let a = n.bounds.origin;
    let b = Point2D::new(
        n.bounds.origin.x + n.bounds.size.x,
        n.bounds.origin.y + n.bounds.size.y,
    );
    let doc_box = padded_box(
        Rect {
            origin: Point2D::new(a.x.min(b.x), a.y.min(b.y)),
            size: Point2D::new((b.x - a.x).abs(), (b.y - a.y).abs()),
        },
        width * 0.5,
    );
    open_svg(out, doc_box, origin);
    let _ = write!(
        out,
        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}"/>"#,
        css::num(a.x),
        css::num(a.y),
        css::num(b.x),
        css::num(b.y),
        css::color(color),
        css::num(width)
    );
    out.push_str("</svg>");
}

/// Emit a Polygon node as an inline SVG polygon over its regular-N-gon
/// vertices — the same `regular_polygon_points` the canvas paints.
pub fn emit_polygon(out: &mut String, n: &SceneNode, origin: Point2D) {
    let rect = normalize(n.bounds);
    if rect.size.x <= 0.0 || rect.size.y <= 0.0 {
        return;
    }
    let stroke_pad = n.stroke.map_or(0.0, |s| s.width * 0.5);
    open_svg(out, padded_box(rect, stroke_pad), origin);
    let mut points = String::new();
    for (i, p) in regular_polygon_points(rect, n.polygon_sides)
        .iter()
        .enumerate()
    {
        if i > 0 {
            points.push(' ');
        }
        let _ = write!(points, "{},{}", css::num(p.x), css::num(p.y));
    }
    let _ = write!(out, r#"<polygon points="{points}"{}/>"#, paint_attrs(n));
    out.push_str("</svg>");
}

/// Emit a Path node — either an imported SVG `d` fitted into its box,
/// or the pen tool's polyline through `points`.
pub fn emit_path(out: &mut String, n: &SceneNode, origin: Point2D) {
    let rect = normalize(n.bounds);
    let stroke_pad = n.stroke.map_or(0.0, |s| s.width * 0.5);
    let (d, transform) = match &n.svg_path {
        Some(authored) => (authored.clone(), fit_transform(authored, rect)),
        None => {
            if n.points.len() < 2 {
                return;
            }
            let mut d = String::with_capacity(n.points.len() * 16);
            let _ = write!(
                d,
                "M{} {}",
                css::num(n.points[0].x),
                css::num(n.points[0].y)
            );
            for p in &n.points[1..] {
                let _ = write!(d, " L{} {}", css::num(p.x), css::num(p.y));
            }
            if n.path_closed {
                d.push_str(" Z");
            }
            (d, String::new())
        }
    };
    let doc_box = if n.svg_path.is_some() {
        padded_box(rect, stroke_pad)
    } else {
        padded_box(points_bounds(&n.points), stroke_pad)
    };
    open_svg(out, doc_box, origin);
    // An open pen stroke with no authored stroke still has to be
    // visible; the canvas hairlines it in the fill colour, so the fill
    // is redirected to the stroke channel rather than flooding the
    // polyline's implied interior.
    let hairline_only = n.svg_path.is_none() && !n.path_closed && n.stroke.is_none();
    let attrs = if hairline_only {
        format!(
            r#" fill="none" stroke="{}" stroke-width="1.5""#,
            css::color(n.fill.unwrap_or(Color::BLACK))
        )
    } else {
        paint_attrs(n)
    };
    let fill_rule = if n.even_odd_fill {
        r#" fill-rule="evenodd""#
    } else {
        ""
    };
    let _ = write!(
        out,
        r#"<path d="{}"{transform}{fill_rule}{attrs}/>"#,
        escape_xml(&d)
    );
    out.push_str("</svg>");
}

/// Open an `<svg>` whose viewBox is stated in absolute doc coordinates,
/// positioned by subtracting the board origin. `overflow:visible` keeps
/// a stroke that straddles the box edge from being clipped.
fn open_svg(out: &mut String, doc_box: Rect, origin: Point2D) {
    let mut style = String::new();
    css::place(
        &mut style,
        Rect {
            origin: Point2D::new(doc_box.origin.x - origin.x, doc_box.origin.y - origin.y),
            size: doc_box.size,
        },
    );
    css::decl(&mut style, "overflow", "visible");
    let _ = write!(
        out,
        r#"<svg class="n" style="{style}" viewBox="{} {} {} {}">"#,
        css::num(doc_box.origin.x),
        css::num(doc_box.origin.y),
        css::num(doc_box.size.x),
        css::num(doc_box.size.y)
    );
}

/// Grow a rect by `pad` on every side, and guarantee a positive extent
/// so the resulting `<svg>` has a renderable viewport.
fn padded_box(rect: Rect, pad: f32) -> Rect {
    let pad = pad.max(0.0);
    Rect {
        origin: Point2D::new(rect.origin.x - pad, rect.origin.y - pad),
        size: Point2D::new(
            (rect.size.x + pad * 2.0).max(1.0),
            (rect.size.y + pad * 2.0).max(1.0),
        ),
    }
}

fn points_bounds(points: &[Point2D]) -> Rect {
    let Some(first) = points.first() else {
        return Rect::ZERO;
    };
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (first.x, first.y, first.x, first.y);
    for p in &points[1..] {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }
    Rect {
        origin: Point2D::new(min_x, min_y),
        size: Point2D::new(max_x - min_x, max_y - min_y),
    }
}

/// `fill` / `stroke` attributes for an SVG shape.
///
/// An absent fill is written as an explicit `fill="none"` rather than
/// left off: SVG's initial `fill` is opaque black, so omitting the
/// attribute would flood an unfilled shape solid.
fn paint_attrs(n: &SceneNode) -> String {
    let mut attrs = String::new();
    match n.fill {
        Some(fill) => {
            let _ = write!(attrs, r#" fill="{}""#, css::color(fill));
        }
        None => attrs.push_str(r#" fill="none""#),
    }
    if let Some(stroke) = n.stroke {
        let _ = write!(
            attrs,
            r#" stroke="{}" stroke-width="{}""#,
            css::color(stroke.color),
            css::num(stroke.width)
        );
    }
    attrs
}

/// Scale + translate an imported `d` (authored in its own coordinate
/// space) into the node's resolved box — the same fit
/// `svg_export::svg_path_fit_transform` applies.
fn fit_transform(d: &str, rect: Rect) -> String {
    let Some((source_x, source_y, source_w, source_h)) = op_editor_core::svg_path_data_bounds(d)
    else {
        return format!(
            r#" transform="translate({} {})""#,
            css::num(rect.origin.x),
            css::num(rect.origin.y)
        );
    };
    let sx = if source_w.abs() > 0.01 {
        rect.size.x / source_w
    } else {
        1.0
    };
    let sy = if source_h.abs() > 0.01 {
        rect.size.y / source_h
    } else {
        1.0
    };
    format!(
        r#" transform="matrix({} 0 0 {} {} {})""#,
        css::num(sx),
        css::num(sy),
        css::num(rect.origin.x - source_x * sx),
        css::num(rect.origin.y - source_y * sy)
    )
}

fn normalize(r: Rect) -> Rect {
    op_editor_ui::scene_bounds::normalize_rect(r)
}
