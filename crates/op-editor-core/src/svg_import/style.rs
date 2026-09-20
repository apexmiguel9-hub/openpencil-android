//! SVG presentation attributes: style-context inheritance, fill /
//! stroke resolution and the named-colour table.

use super::*;

/// Merge `parent` with the element's own `fill` / `stroke` /
/// `stroke-width` (inline `style="..."` takes precedence over the
/// matching attribute, matching `extractStyleOrAttr` in TS).
pub(super) fn merge_style_ctx(parent: &StyleCtx, attrs: &[(String, String)]) -> StyleCtx {
    StyleCtx {
        fill: extract_style_or_attr(attrs, "fill").or_else(|| parent.fill.clone()),
        stroke: extract_style_or_attr(attrs, "stroke").or_else(|| parent.stroke.clone()),
        stroke_width: extract_style_or_attr(attrs, "stroke-width")
            .and_then(|s| s.trim().parse::<f64>().ok())
            .unwrap_or(parent.stroke_width),
        fill_rule: extract_style_or_attr(attrs, "fill-rule")
            .as_deref()
            .and_then(parse_svg_fill_rule)
            .or(parent.fill_rule),
    }
}

pub(super) fn parse_svg_fill_rule(raw: &str) -> Option<PathFillRule> {
    match raw.trim() {
        value if value.eq_ignore_ascii_case("evenodd") => Some(PathFillRule::Evenodd),
        value if value.eq_ignore_ascii_case("nonzero") => Some(PathFillRule::Nonzero),
        _ => None,
    }
}

/// Look up `name` in an inline `style="..."` first, then fall back
/// to the named attribute. Mirrors `extractStyleOrAttr` from the TS
/// regex parser.
pub(super) fn extract_style_or_attr(attrs: &[(String, String)], name: &str) -> Option<String> {
    if let Some(style) = attrs.iter().find(|(k, _)| k == "style").map(|(_, v)| v) {
        // Naive CSS-ish split — values can't contain `;` because
        // SVG inline styles forbid it.
        for pair in style.split(';') {
            let trimmed = pair.trim();
            if let Some((k, v)) = trimmed.split_once(':') {
                if k.trim().eq_ignore_ascii_case(name) {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    attrs
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.to_string())
}

pub(super) fn resolve_svg_fill_hex(attrs: &[(String, String)], ctx: &StyleCtx) -> Option<String> {
    let raw = extract_style_or_attr(attrs, "fill").or_else(|| ctx.fill.clone());
    match raw.as_deref().map(str::trim) {
        None => Some("#000000".to_string()),
        Some(v) if v.eq_ignore_ascii_case("none") || v.eq_ignore_ascii_case("transparent") => None,
        Some(v) if v.to_ascii_lowercase().starts_with("url(") => Some("#000000".to_string()),
        Some(v) => parse_svg_color(v),
    }
}

pub(super) fn resolve_svg_stroke(
    attrs: &[(String, String)],
    ctx: &StyleCtx,
    scale: f64,
) -> Option<PenStroke> {
    let raw = extract_style_or_attr(attrs, "stroke").or_else(|| ctx.stroke.clone())?;
    let raw = raw.trim();
    if raw.eq_ignore_ascii_case("none") || raw.to_ascii_lowercase().starts_with("url(") {
        return None;
    }
    let hex = parse_svg_color(raw).unwrap_or_else(|| "#000000".to_string());
    let width = (ctx.stroke_width * scale).max(0.0) as f32;
    if width <= 0.0 {
        return None;
    }
    Some(PenStroke {
        thickness: StrokeThickness::Uniform(width),
        align: None,
        join: None,
        cap: None,
        dash_pattern: None,
        dash_offset: None,
        fill: Some(vec![solid_fill(&hex)]),
    })
}

pub(super) fn solid_fill(hex: &str) -> PenFill {
    PenFill::Solid(SolidFillBody {
        color: hex.to_string(),
        explain: None,
        opacity: None,
        blend_mode: None,
    })
}

pub(super) fn set_node_stroke(node: &mut PenNode, stroke: PenStroke) {
    match node {
        PenNode::Frame(n) => n.container.stroke = Some(stroke),
        PenNode::Group(n) => n.container.stroke = Some(stroke),
        PenNode::Rectangle(n) => n.container.stroke = Some(stroke),
        PenNode::Ellipse(n) => n.stroke = Some(stroke),
        PenNode::Polygon(n) => n.stroke = Some(stroke),
        PenNode::Path(n) => n.stroke = Some(stroke),
        PenNode::Line(n) => n.stroke = Some(stroke),
        PenNode::TextInput(n) => n.stroke = Some(stroke),
        PenNode::TextArea(n) => n.stroke = Some(stroke),
        PenNode::Select(n) => n.stroke = Some(stroke),
        PenNode::Switch(n) => n.stroke = Some(stroke),
        PenNode::Checkbox(n) => n.stroke = Some(stroke),
        PenNode::Slider(n) => n.stroke = Some(stroke),
        PenNode::RadioGroup(n) => n.stroke = Some(stroke),
        PenNode::NumberInput(n) => n.stroke = Some(stroke),
        PenNode::Progress(n) => n.stroke = Some(stroke),
        PenNode::Tabs(n) => n.stroke = Some(stroke),
        PenNode::Text(_) | PenNode::IconFont(_) | PenNode::Image(_) | PenNode::Ref(_) => {}
    }
}

pub(super) fn clear_node_fill(node: &mut PenNode) {
    match node {
        PenNode::Frame(n) => n.container.fill = None,
        PenNode::Group(n) => n.container.fill = None,
        PenNode::Rectangle(n) => n.container.fill = None,
        PenNode::Ellipse(n) => n.fill = None,
        PenNode::Polygon(n) => n.fill = None,
        PenNode::Path(n) => n.fill = None,
        PenNode::Text(n) => n.fill = None,
        PenNode::TextInput(n) => n.fill = None,
        PenNode::IconFont(n) => n.fill = None,
        PenNode::TextArea(n) => n.fill = None,
        PenNode::Select(n) => n.fill = None,
        PenNode::Switch(n) => n.fill = None,
        PenNode::Checkbox(n) => n.fill = None,
        PenNode::Slider(n) => n.fill = None,
        PenNode::RadioGroup(n) => n.fill = None,
        PenNode::NumberInput(n) => n.fill = None,
        PenNode::Progress(n) => n.fill = None,
        PenNode::Tabs(n) => n.fill = None,
        PenNode::Line(_) | PenNode::Image(_) | PenNode::Ref(_) => {}
    }
}

/// Parse an SVG `fill` value into a `#rrggbb` hex string. `none` /
/// `transparent` and unparseable values return `None` (no fill).
pub(super) fn parse_svg_color(raw: &str) -> Option<String> {
    let v = raw.trim().to_ascii_lowercase();
    if v.is_empty() || v == "none" || v == "transparent" {
        return None;
    }
    if let Some(hex) = v.strip_prefix('#') {
        return match hex.len() {
            3 => {
                let mut out = String::with_capacity(7);
                out.push('#');
                for ch in hex.chars() {
                    out.push(ch);
                    out.push(ch);
                }
                Some(out)
            }
            6 | 8 => Some(format!("#{}", &hex[..6])),
            _ => None,
        };
    }
    // Minimal named-colour table — the common SVG presentation names.
    let named = match v.as_str() {
        "black" => "#000000",
        "white" => "#ffffff",
        "red" => "#ff0000",
        "green" => "#008000",
        "lime" => "#00ff00",
        "blue" => "#0000ff",
        "yellow" => "#ffff00",
        "cyan" | "aqua" => "#00ffff",
        "magenta" | "fuchsia" => "#ff00ff",
        "gray" | "grey" => "#808080",
        "silver" => "#c0c0c0",
        "orange" => "#ffa500",
        "purple" => "#800080",
        "navy" => "#000080",
        "teal" => "#008080",
        _ => return None,
    };
    Some(named.to_string())
}
