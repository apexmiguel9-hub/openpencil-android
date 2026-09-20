//! Read accessors over a node's fill / stroke / effect lists.
//!
//! Every helper here is a pure reader (plus the tiny `solid_fill`
//! constructor) — the writers live in the sibling modules.

use super::*;

/// Borrow a node's `fill` list, if the variant carries one. Frame /
/// Group fills live on `container.fill`; the leaf paintable variants
/// (Rectangle / Ellipse / Polygon / Path / Text / TextInput /
/// IconFont) carry their own `fill`. Line / Image / Ref have none.
pub fn node_fills(node: &PenNode) -> Option<&Vec<PenFill>> {
    match node {
        PenNode::Frame(n) => n.container.fill.as_ref(),
        PenNode::Group(n) => n.container.fill.as_ref(),
        PenNode::Rectangle(n) => n.container.fill.as_ref(),
        PenNode::Ellipse(n) => n.fill.as_ref(),
        PenNode::Polygon(n) => n.fill.as_ref(),
        PenNode::Path(n) => n.fill.as_ref(),
        PenNode::Text(n) => n.fill.as_ref(),
        PenNode::TextInput(n) => n.fill.as_ref(),
        PenNode::IconFont(n) => n.fill.as_ref(),
        PenNode::TextArea(n) => n.fill.as_ref(),
        PenNode::Select(n) => n.fill.as_ref(),
        PenNode::Switch(n) => n.fill.as_ref(),
        PenNode::Checkbox(n) => n.fill.as_ref(),
        PenNode::Slider(n) => n.fill.as_ref(),
        PenNode::RadioGroup(n) => n.fill.as_ref(),
        PenNode::NumberInput(n) => n.fill.as_ref(),
        PenNode::Progress(n) => n.fill.as_ref(),
        PenNode::Tabs(n) => n.fill.as_ref(),
        PenNode::Line(_) | PenNode::Image(_) | PenNode::Ref(_) => None,
    }
}

/// Mutably borrow a node's `fill` list, creating an empty one when
/// the variant supports fills but has none yet. `None` for the
/// variants that have no `fill` field at all.
pub fn node_fills_mut(node: &mut PenNode) -> Option<&mut Vec<PenFill>> {
    match node {
        PenNode::Frame(n) => Some(n.container.fill.get_or_insert_with(Vec::new)),
        PenNode::Group(n) => Some(n.container.fill.get_or_insert_with(Vec::new)),
        PenNode::Rectangle(n) => Some(n.container.fill.get_or_insert_with(Vec::new)),
        PenNode::Ellipse(n) => Some(n.fill.get_or_insert_with(Vec::new)),
        PenNode::Polygon(n) => Some(n.fill.get_or_insert_with(Vec::new)),
        PenNode::Path(n) => Some(n.fill.get_or_insert_with(Vec::new)),
        PenNode::Text(n) => Some(n.fill.get_or_insert_with(Vec::new)),
        PenNode::TextInput(n) => Some(n.fill.get_or_insert_with(Vec::new)),
        PenNode::IconFont(n) => Some(n.fill.get_or_insert_with(Vec::new)),
        PenNode::TextArea(n) => Some(n.fill.get_or_insert_with(Vec::new)),
        PenNode::Select(n) => Some(n.fill.get_or_insert_with(Vec::new)),
        PenNode::Switch(n) => Some(n.fill.get_or_insert_with(Vec::new)),
        PenNode::Checkbox(n) => Some(n.fill.get_or_insert_with(Vec::new)),
        PenNode::Slider(n) => Some(n.fill.get_or_insert_with(Vec::new)),
        PenNode::RadioGroup(n) => Some(n.fill.get_or_insert_with(Vec::new)),
        PenNode::NumberInput(n) => Some(n.fill.get_or_insert_with(Vec::new)),
        PenNode::Progress(n) => Some(n.fill.get_or_insert_with(Vec::new)),
        PenNode::Tabs(n) => Some(n.fill.get_or_insert_with(Vec::new)),
        PenNode::Line(_) | PenNode::Image(_) | PenNode::Ref(_) => None,
    }
}

/// Borrow a node's `stroke`, if the variant carries one.
pub(crate) fn node_stroke_mut(node: &mut PenNode) -> Option<&mut Option<PenStroke>> {
    match node {
        PenNode::Frame(n) => Some(&mut n.container.stroke),
        PenNode::Group(n) => Some(&mut n.container.stroke),
        PenNode::Rectangle(n) => Some(&mut n.container.stroke),
        PenNode::Ellipse(n) => Some(&mut n.stroke),
        PenNode::Polygon(n) => Some(&mut n.stroke),
        PenNode::Path(n) => Some(&mut n.stroke),
        PenNode::Line(n) => Some(&mut n.stroke),
        PenNode::TextInput(n) => Some(&mut n.stroke),
        PenNode::IconFont(n) => Some(&mut n.stroke),
        PenNode::TextArea(n) => Some(&mut n.stroke),
        PenNode::Select(n) => Some(&mut n.stroke),
        PenNode::Switch(n) => Some(&mut n.stroke),
        PenNode::Checkbox(n) => Some(&mut n.stroke),
        PenNode::Slider(n) => Some(&mut n.stroke),
        PenNode::RadioGroup(n) => Some(&mut n.stroke),
        PenNode::NumberInput(n) => Some(&mut n.stroke),
        PenNode::Progress(n) => Some(&mut n.stroke),
        PenNode::Tabs(n) => Some(&mut n.stroke),
        PenNode::Text(_) | PenNode::Image(_) | PenNode::Ref(_) => None,
    }
}

/// Shared stroke accessor for reads.
fn node_stroke(node: &PenNode) -> Option<&PenStroke> {
    match node {
        PenNode::Frame(n) => n.container.stroke.as_ref(),
        PenNode::Group(n) => n.container.stroke.as_ref(),
        PenNode::Rectangle(n) => n.container.stroke.as_ref(),
        PenNode::Ellipse(n) => n.stroke.as_ref(),
        PenNode::Polygon(n) => n.stroke.as_ref(),
        PenNode::Path(n) => n.stroke.as_ref(),
        PenNode::Line(n) => n.stroke.as_ref(),
        PenNode::TextInput(n) => n.stroke.as_ref(),
        PenNode::IconFont(n) => n.stroke.as_ref(),
        PenNode::TextArea(n) => n.stroke.as_ref(),
        PenNode::Select(n) => n.stroke.as_ref(),
        PenNode::Switch(n) => n.stroke.as_ref(),
        PenNode::Checkbox(n) => n.stroke.as_ref(),
        PenNode::Slider(n) => n.stroke.as_ref(),
        PenNode::RadioGroup(n) => n.stroke.as_ref(),
        PenNode::NumberInput(n) => n.stroke.as_ref(),
        PenNode::Progress(n) => n.stroke.as_ref(),
        PenNode::Tabs(n) => n.stroke.as_ref(),
        PenNode::Text(_) | PenNode::Image(_) | PenNode::Ref(_) => None,
    }
}

/// Mutably borrow a node's `effects` list, creating an empty one
/// when the variant supports effects but has none yet.
/// Whether a node variant carries an `effects` list — mirrors the
/// `node_effects_mut` match. `false` for `IconFont` / `Ref`, so the
/// effect-add path can skip the history snapshot for a target it
/// can't mutate (avoids an empty undo + dirty state).
pub fn node_supports_effects(node: &PenNode) -> bool {
    !matches!(node, PenNode::IconFont(_) | PenNode::Ref(_))
}

pub(super) fn node_effects_mut(node: &mut PenNode) -> Option<&mut Vec<PenEffect>> {
    match node {
        PenNode::Frame(n) => Some(n.container.effects.get_or_insert_with(Vec::new)),
        PenNode::Group(n) => Some(n.container.effects.get_or_insert_with(Vec::new)),
        PenNode::Rectangle(n) => Some(n.container.effects.get_or_insert_with(Vec::new)),
        PenNode::Ellipse(n) => Some(n.effects.get_or_insert_with(Vec::new)),
        PenNode::Polygon(n) => Some(n.effects.get_or_insert_with(Vec::new)),
        PenNode::Path(n) => Some(n.effects.get_or_insert_with(Vec::new)),
        PenNode::Line(n) => Some(n.effects.get_or_insert_with(Vec::new)),
        PenNode::Text(n) => Some(n.effects.get_or_insert_with(Vec::new)),
        PenNode::TextInput(n) => Some(n.effects.get_or_insert_with(Vec::new)),
        PenNode::Image(n) => Some(n.effects.get_or_insert_with(Vec::new)),
        PenNode::TextArea(n) => Some(n.effects.get_or_insert_with(Vec::new)),
        PenNode::Select(n) => Some(n.effects.get_or_insert_with(Vec::new)),
        PenNode::Switch(n) => Some(n.effects.get_or_insert_with(Vec::new)),
        PenNode::Checkbox(n) => Some(n.effects.get_or_insert_with(Vec::new)),
        PenNode::Slider(n) => Some(n.effects.get_or_insert_with(Vec::new)),
        PenNode::RadioGroup(n) => Some(n.effects.get_or_insert_with(Vec::new)),
        PenNode::NumberInput(n) => Some(n.effects.get_or_insert_with(Vec::new)),
        PenNode::Progress(n) => Some(n.effects.get_or_insert_with(Vec::new)),
        PenNode::Tabs(n) => Some(n.effects.get_or_insert_with(Vec::new)),
        PenNode::IconFont(_) | PenNode::Ref(_) => None,
    }
}

/// Read-only view of a node's stroke fill list, when present — for
/// token-detection walks that must not materialize anything.
pub(crate) fn node_stroke_fills(node: &PenNode) -> Option<&Vec<PenFill>> {
    node_stroke(node)?.fill.as_ref()
}

/// Mutably borrow a node's existing `fill` list WITHOUT materializing
/// an empty one — for tree walks (e.g. `$ref` resolution) that must
/// not change which nodes carry a `fill` field.
pub(crate) fn node_fills_opt_mut(node: &mut PenNode) -> Option<&mut Vec<PenFill>> {
    match node {
        PenNode::Frame(n) => n.container.fill.as_mut(),
        PenNode::Group(n) => n.container.fill.as_mut(),
        PenNode::Rectangle(n) => n.container.fill.as_mut(),
        PenNode::Ellipse(n) => n.fill.as_mut(),
        PenNode::Polygon(n) => n.fill.as_mut(),
        PenNode::Path(n) => n.fill.as_mut(),
        PenNode::Text(n) => n.fill.as_mut(),
        PenNode::TextInput(n) => n.fill.as_mut(),
        PenNode::IconFont(n) => n.fill.as_mut(),
        PenNode::TextArea(n) => n.fill.as_mut(),
        PenNode::Select(n) => n.fill.as_mut(),
        PenNode::Switch(n) => n.fill.as_mut(),
        PenNode::Checkbox(n) => n.fill.as_mut(),
        PenNode::Slider(n) => n.fill.as_mut(),
        PenNode::RadioGroup(n) => n.fill.as_mut(),
        PenNode::NumberInput(n) => n.fill.as_mut(),
        PenNode::Progress(n) => n.fill.as_mut(),
        PenNode::Tabs(n) => n.fill.as_mut(),
        PenNode::Line(_) | PenNode::Image(_) | PenNode::Ref(_) => None,
    }
}

/// Mutably borrow a node's existing `effects` list WITHOUT
/// materializing an empty one — companion to [`node_fills_opt_mut`].
pub(crate) fn node_effects_opt_mut(node: &mut PenNode) -> Option<&mut Vec<PenEffect>> {
    match node {
        PenNode::Frame(n) => n.container.effects.as_mut(),
        PenNode::Group(n) => n.container.effects.as_mut(),
        PenNode::Rectangle(n) => n.container.effects.as_mut(),
        PenNode::Ellipse(n) => n.effects.as_mut(),
        PenNode::Polygon(n) => n.effects.as_mut(),
        PenNode::Path(n) => n.effects.as_mut(),
        PenNode::Line(n) => n.effects.as_mut(),
        PenNode::Text(n) => n.effects.as_mut(),
        PenNode::TextInput(n) => n.effects.as_mut(),
        PenNode::Image(n) => n.effects.as_mut(),
        PenNode::TextArea(n) => n.effects.as_mut(),
        PenNode::Select(n) => n.effects.as_mut(),
        PenNode::Switch(n) => n.effects.as_mut(),
        PenNode::Checkbox(n) => n.effects.as_mut(),
        PenNode::Slider(n) => n.effects.as_mut(),
        PenNode::RadioGroup(n) => n.effects.as_mut(),
        PenNode::NumberInput(n) => n.effects.as_mut(),
        PenNode::Progress(n) => n.effects.as_mut(),
        PenNode::Tabs(n) => n.effects.as_mut(),
        PenNode::IconFont(_) | PenNode::Ref(_) => None,
    }
}

/// First `Solid` fill's hex string, when the node has one.
pub fn first_solid_fill_hex(node: &PenNode) -> Option<&str> {
    let fills = node_fills(node)?;
    fills.iter().find_map(|f| match f {
        PenFill::Solid(body) => Some(body.color.as_str()),
        _ => None,
    })
}

/// Read-only view of a node's visual effects. Frame / Group /
/// Rectangle carry them on `container`; the leaf shapes carry them
/// directly. Returns an empty slice for a node with no effects (or a
/// kind — IconFont / Ref — with no effects field).
pub fn node_effects(node: &PenNode) -> &[jian_ops_schema::style::PenEffect] {
    let slot = match node {
        PenNode::Frame(n) => n.container.effects.as_deref(),
        PenNode::Group(n) => n.container.effects.as_deref(),
        PenNode::Rectangle(n) => n.container.effects.as_deref(),
        PenNode::Ellipse(n) => n.effects.as_deref(),
        PenNode::Polygon(n) => n.effects.as_deref(),
        PenNode::Path(n) => n.effects.as_deref(),
        PenNode::Line(n) => n.effects.as_deref(),
        PenNode::Text(n) => n.effects.as_deref(),
        PenNode::TextInput(n) => n.effects.as_deref(),
        PenNode::Image(n) => n.effects.as_deref(),
        PenNode::TextArea(n) => n.effects.as_deref(),
        PenNode::Select(n) => n.effects.as_deref(),
        PenNode::Switch(n) => n.effects.as_deref(),
        PenNode::Checkbox(n) => n.effects.as_deref(),
        PenNode::Slider(n) => n.effects.as_deref(),
        PenNode::RadioGroup(n) => n.effects.as_deref(),
        PenNode::NumberInput(n) => n.effects.as_deref(),
        PenNode::Progress(n) => n.effects.as_deref(),
        PenNode::Tabs(n) => n.effects.as_deref(),
        PenNode::IconFont(_) | PenNode::Ref(_) => None,
    };
    slot.unwrap_or(&[])
}

/// First `Solid` fill's hex string on the node's stroke.
pub fn first_solid_stroke_hex(node: &PenNode) -> Option<&str> {
    let stroke = node_stroke(node)?;
    stroke.fill.as_ref()?.iter().find_map(|f| match f {
        PenFill::Solid(body) => Some(body.color.as_str()),
        _ => None,
    })
}

/// Display stroke width (doc-px) for the node, when it carries a
/// stroke. A side-specific thickness reports the widest edge so the
/// legacy scalar width input still reflects the visible maximum.
/// `None` when the variant carries no stroke or the node has none set.
pub fn node_stroke_width(node: &PenNode) -> Option<f64> {
    use jian_ops_schema::style::StrokeThickness;
    match &node_stroke(node)?.thickness {
        StrokeThickness::Uniform(w) => Some(*w as f64),
        StrokeThickness::PerSide(sides) => {
            Some(sides.iter().copied().fold(0.0_f32, f32::max) as f64)
        }
        StrokeThickness::Sided(s) => Some(
            [s.top, s.right, s.bottom, s.left]
                .into_iter()
                .flatten()
                .fold(0.0_f32, f32::max) as f64,
        ),
    }
}

/// Per-side stroke widths in `[top, right, bottom, left]` order.
pub fn node_stroke_side_widths(node: &PenNode) -> Option<[f32; 4]> {
    use jian_ops_schema::style::StrokeThickness;
    match &node_stroke(node)?.thickness {
        StrokeThickness::Uniform(w) => Some([*w; 4]),
        StrokeThickness::PerSide(sides) => Some(*sides),
        StrokeThickness::Sided(s) => Some([
            s.top.unwrap_or(0.0),
            s.right.unwrap_or(0.0),
            s.bottom.unwrap_or(0.0),
            s.left.unwrap_or(0.0),
        ]),
    }
}

/// Build a bare `Solid` fill from a hex string.
pub(super) fn solid_fill(hex: String) -> PenFill {
    PenFill::Solid(SolidFillBody {
        color: hex,
        explain: None,
        opacity: None,
        blend_mode: None,
    })
}

/// Opacity of the node's **primary** fill — whatever kind it is
/// (Solid / LinearGradient / RadialGradient / Image). `1.0` when
/// the node has no fill at all or the body's opacity is `None`.
/// The Fill section's `100 %` input drives this regardless of
/// fill kind, so a gradient / image fill reports its own opacity.
pub fn first_solid_fill_opacity(node: &PenNode) -> f32 {
    node_fills(node)
        .and_then(|fills| fills.first())
        .map(|fill| match fill {
            PenFill::Solid(b) => b.opacity.unwrap_or(1.0),
            PenFill::LinearGradient(b) => b.opacity.unwrap_or(1.0),
            PenFill::RadialGradient(b) => b.opacity.unwrap_or(1.0),
            PenFill::MeshGradient(b) => b.opacity.unwrap_or(1.0),
            PenFill::Shader(b) => b.opacity.unwrap_or(1.0),
            PenFill::Image(b) => b.opacity.unwrap_or(1.0),
        })
        .unwrap_or(1.0)
}

/// Opacity of the node's **stroke** paint — the stroke's first fill
/// body's opacity. `1.0` when the node has no stroke, no stroke fill, or
/// the body's opacity is `None`. The stroke channel of
/// [`first_solid_fill_opacity`]: the loader bakes this into the resolved
/// scene stroke alpha (`stroke_to_payload` → `first_solid_color` →
/// `apply_alpha`), so a live paint patch must reproduce it.
pub fn first_solid_stroke_opacity(node: &PenNode) -> f32 {
    node_stroke(node)
        .and_then(|s| s.fill.as_ref())
        .and_then(|fills| fills.first())
        .map(|fill| match fill {
            PenFill::Solid(b) => b.opacity.unwrap_or(1.0),
            PenFill::LinearGradient(b) => b.opacity.unwrap_or(1.0),
            PenFill::RadialGradient(b) => b.opacity.unwrap_or(1.0),
            PenFill::MeshGradient(b) => b.opacity.unwrap_or(1.0),
            PenFill::Shader(b) => b.opacity.unwrap_or(1.0),
            PenFill::Image(b) => b.opacity.unwrap_or(1.0),
        })
        .unwrap_or(1.0)
}
