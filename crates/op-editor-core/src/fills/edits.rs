//! Indexed fill list edits (add / remove / set type / hex / opacity),
//! the primary stroke hex writer and the effect-list appenders.

use super::*;

/// Set the node's primary fill kind to `kind`. The canonical model
/// encodes fill type as the first `PenFill` variant, so this converts
/// the first fill to the requested variant via [`convert_fill`] —
/// preserving as much of the existing body (gradient stops, opacity,
/// blend mode) as the target variant allows — or prepends a default
/// body when the node has no fills. Non-first fills are left untouched.
/// `false` for variants that carry no `fill` field at all.
pub fn set_primary_fill_type(node: &mut PenNode, kind: FillType) -> bool {
    let Some(fills) = node_fills_mut(node) else {
        return false;
    };
    if fills.is_empty() {
        fills.push(default_fill_of_type(kind, "#000000"));
    } else {
        let existing = fills.remove(0);
        fills.insert(0, convert_fill(existing, kind));
    }
    true
}

pub fn clear_primary_fills(node: &mut PenNode) -> bool {
    let Some(fills) = node_fills_mut(node) else {
        return false;
    };
    fills.clear();
    true
}

// ── Multi-fill (indexed) ops ───────────────────────────────────────
//
// The property panel's Fill section stacks one editable row per `PenFill`
// (header "+" appends, each row's "×" removes). These mirror the
// `set_primary_*` ops but address a fill by index so every row edits its
// own fill. `false` when the node carries no `fill` field or the index is
// out of range.

/// Number of fills on the node (0 for variants without a `fill` field).
pub fn fill_count(node: &PenNode) -> usize {
    node_fills(node).map(|f| f.len()).unwrap_or(0)
}

/// `FillType` of the fill at `index`, if present.
pub fn fill_type_at(node: &PenNode, index: usize) -> Option<FillType> {
    node_fills(node)
        .and_then(|fills| fills.get(index))
        .map(fill_type_of)
}

/// Append a new default solid fill (the TS new-fill default `#d1d5db`).
pub fn add_fill(node: &mut PenNode) -> bool {
    let Some(fills) = node_fills_mut(node) else {
        return false;
    };
    fills.push(default_fill_of_type(FillType::Solid, "#d1d5db"));
    true
}

/// Remove the fill at `index`.
pub fn remove_fill(node: &mut PenNode, index: usize) -> bool {
    let Some(fills) = node_fills_mut(node) else {
        return false;
    };
    if index >= fills.len() {
        return false;
    }
    fills.remove(index);
    true
}

/// Convert the fill at `index` to `kind`, preserving as much of the body
/// as the target variant allows (mirrors [`set_primary_fill_type`]).
pub fn set_fill_type_at(node: &mut PenNode, index: usize, kind: FillType) -> bool {
    let Some(fills) = node_fills_mut(node) else {
        return false;
    };
    let Some(existing) = (index < fills.len()).then(|| fills.remove(index)) else {
        return false;
    };
    fills.insert(index, convert_fill(existing, kind));
    true
}

/// Write `hex` (`#RRGGBB` or `#RRGGBBAA`) into the solid fill at `index`.
/// No-op (returns `false`) if
/// that fill isn't a solid — type changes go through [`set_fill_type_at`].
pub fn set_fill_hex_at(node: &mut PenNode, index: usize, hex: &str) -> bool {
    let Some(fills) = node_fills_mut(node) else {
        return false;
    };
    match fills.get_mut(index) {
        Some(PenFill::Solid(body)) => {
            body.color = hex.to_string();
            true
        }
        _ => false,
    }
}

/// Set the opacity (0.0..=1.0) of the fill at `index`.
pub fn set_fill_opacity_at(node: &mut PenNode, index: usize, opacity: f32) -> bool {
    let Some(fills) = node_fills_mut(node) else {
        return false;
    };
    let opacity = opacity.clamp(0.0, 1.0);
    match fills.get_mut(index) {
        Some(PenFill::Solid(b)) => {
            b.opacity = Some(opacity);
            true
        }
        Some(PenFill::LinearGradient(b)) => {
            b.opacity = Some(opacity);
            true
        }
        Some(PenFill::RadialGradient(b)) => {
            b.opacity = Some(opacity);
            true
        }
        Some(PenFill::MeshGradient(b)) => {
            b.opacity = Some(opacity);
            true
        }
        Some(PenFill::Shader(b)) => {
            b.opacity = Some(opacity);
            true
        }
        Some(PenFill::Image(b)) => {
            b.opacity = Some(opacity);
            true
        }
        _ => false,
    }
}

/// Stroke parallel to [`set_primary_fill_hex`]. Creates a default
/// 1-px stroke when the node has none, so a colour write always
/// lands a visible stroke. `false` for variants without a stroke.
pub fn set_primary_stroke_hex(node: &mut PenNode, hex: &str) -> bool {
    let Some(slot) = node_stroke_mut(node) else {
        return false;
    };
    let stroke = slot.get_or_insert_with(|| PenStroke {
        thickness: StrokeThickness::Uniform(1.0),
        align: None,
        join: None,
        cap: None,
        dash_pattern: None,
        dash_offset: None,
        fill: None,
    });
    let fills = stroke.fill.get_or_insert_with(Vec::new);
    if let Some(body) = fills.iter_mut().find_map(|f| match f {
        PenFill::Solid(body) => Some(body),
        _ => None,
    }) {
        body.color = hex.to_string();
    } else {
        fills.insert(0, solid_fill(hex.to_string()));
    }
    true
}

/// Append a default drop-shadow effect — mirrors a common CSS card
/// shadow (`0 4px 8px rgba(0,0,0,0.25)`). `false` for variants that
/// carry no `effects` field.
pub fn push_drop_shadow(node: &mut PenNode) -> bool {
    let Some(effects) = node_effects_mut(node) else {
        return false;
    };
    effects.push(PenEffect::Shadow(ShadowBody {
        inner: None,
        visible: None,
        offset_x: 0.0,
        offset_y: 4.0,
        blur: 8.0,
        spread: 0.0,
        color: "#00000040".to_string(),
    }));
    true
}

/// Append a default Gaussian layer-blur effect (Figma "Layer blur").
/// `false` for variants that carry no `effects` field.
pub fn push_layer_blur(node: &mut PenNode) -> bool {
    let Some(effects) = node_effects_mut(node) else {
        return false;
    };
    effects.push(PenEffect::Blur(jian_ops_schema::style::BlurBody {
        radius: 4.0,
        visible: None,
    }));
    true
}

/// Append a default Gaussian background blur. The optional visibility
/// field stays absent because absence is the schema's semantic "visible".
pub fn push_background_blur(node: &mut PenNode) -> bool {
    let Some(effects) = node_effects_mut(node) else {
        return false;
    };
    effects.push(PenEffect::BackgroundBlur(
        jian_ops_schema::style::BlurBody {
            radius: 10.0,
            visible: None,
        },
    ));
    true
}
