//! Writers targeting a node's *primary* (first) fill: opacity, hex and
//! the gradient angle / stop list.

use super::*;

/// Write the primary fill's `opacity` (clamped to `[0.0, 1.0]`),
/// matching on whatever variant the first fill is. Touches no
/// other field — a gradient / image fill keeps its stops, image
/// url, etc. `false` (no-op) when the variant carries no `fill`
/// field or the node has no fills.
pub fn set_primary_fill_opacity(node: &mut PenNode, opacity: f32) -> bool {
    // Read-only probe first: `node_fills_mut` would `get_or_insert_with`
    // an empty Vec, silently mutating `fill: None` into
    // `fill: Some([])`. Bail before touching the document when
    // there's nothing to update.
    if node_fills(node).map(|f| f.is_empty()).unwrap_or(true) {
        return false;
    }
    let opacity = opacity.clamp(0.0, 1.0);
    let Some(fills) = node_fills_mut(node) else {
        return false;
    };
    let Some(first) = fills.first_mut() else {
        return false;
    };
    match first {
        PenFill::Solid(b) => b.opacity = Some(opacity),
        PenFill::LinearGradient(b) => b.opacity = Some(opacity),
        PenFill::RadialGradient(b) => b.opacity = Some(opacity),
        PenFill::MeshGradient(b) => b.opacity = Some(opacity),
        PenFill::Shader(b) => b.opacity = Some(opacity),
        PenFill::Image(b) => b.opacity = Some(opacity),
    }
    true
}

/// Set the LinearGradient body's `angle` (degrees, canonical
/// `.op` convention — 0° = bottom→top). No-op when the first fill
/// isn't a linear gradient; returns `false` so callers can detect
/// the silent rejection (panel input clears without mutation).
pub fn set_primary_gradient_angle(node: &mut PenNode, angle_deg: f32) -> bool {
    if node_fills(node).map(|f| f.is_empty()).unwrap_or(true) {
        return false;
    }
    let Some(fills) = node_fills_mut(node) else {
        return false;
    };
    let Some(first) = fills.first_mut() else {
        return false;
    };
    match first {
        PenFill::LinearGradient(b) => {
            b.angle = Some(angle_deg);
            true
        }
        _ => false,
    }
}

/// Replace gradient stop `index`'s colour with `hex` (already
/// validated `#RRGGBB`). Linear + Radial both accepted. No-op when
/// the first fill isn't a gradient or `index` is out of range.
pub fn set_primary_gradient_stop_hex(node: &mut PenNode, index: usize, hex: &str) -> bool {
    if node_fills(node).map(|f| f.is_empty()).unwrap_or(true) {
        return false;
    }
    let Some(fills) = node_fills_mut(node) else {
        return false;
    };
    let Some(first) = fills.first_mut() else {
        return false;
    };
    let stops = match first {
        PenFill::LinearGradient(b) => &mut b.stops,
        PenFill::RadialGradient(b) => &mut b.stops,
        _ => return false,
    };
    let Some(stop) = stops.get_mut(index) else {
        return false;
    };
    stop.color = hex.to_string();
    true
}

/// Replace gradient stop `index`'s offset with `frac` (0.0..=1.0).
/// Linear + Radial both accepted. Same no-op rules as the hex
/// setter; offset is clamped before write so the canonical schema's
/// invariant (`0 ≤ offset ≤ 1`) holds.
pub fn set_primary_gradient_stop_offset(node: &mut PenNode, index: usize, frac: f32) -> bool {
    if node_fills(node).map(|f| f.is_empty()).unwrap_or(true) {
        return false;
    }
    let Some(fills) = node_fills_mut(node) else {
        return false;
    };
    let Some(first) = fills.first_mut() else {
        return false;
    };
    let stops = match first {
        PenFill::LinearGradient(b) => &mut b.stops,
        PenFill::RadialGradient(b) => &mut b.stops,
        _ => return false,
    };
    let Some(stop) = stops.get_mut(index) else {
        return false;
    };
    stop.offset = frac.clamp(0.0, 1.0);
    true
}

fn primary_gradient_stops_mut(node: &mut PenNode) -> Option<&mut Vec<GradientStop>> {
    if node_fills(node).map(|f| f.is_empty()).unwrap_or(true) {
        return None;
    }
    let fills = node_fills_mut(node)?;
    match fills.first_mut()? {
        PenFill::LinearGradient(b) => Some(&mut b.stops),
        PenFill::RadialGradient(b) => Some(&mut b.stops),
        _ => None,
    }
}

pub fn add_primary_gradient_stop(node: &mut PenNode) -> bool {
    let Some(stops) = primary_gradient_stops_mut(node) else {
        return false;
    };
    let last_offset = stops.last().map(|s| s.offset).unwrap_or(0.5);
    stops.push(GradientStop {
        offset: (last_offset + 0.1).min(1.0),
        color: "#888888".to_string(),
    });
    true
}

pub fn remove_primary_gradient_stop(node: &mut PenNode, index: usize) -> bool {
    let Some(stops) = primary_gradient_stops_mut(node) else {
        return false;
    };
    if stops.len() <= 2 || index >= stops.len() {
        return false;
    }
    stops.remove(index);
    true
}

/// Replace the first `Solid` fill's colour with `hex`, leaving any
/// gradient / image fills untouched. When the node has no solid fill,
/// a fresh one is prepended so it paints on top. `false` when the
/// variant carries no `fill` field at all.
pub fn set_primary_fill_hex(node: &mut PenNode, hex: &str) -> bool {
    let Some(fills) = node_fills_mut(node) else {
        return false;
    };
    if let Some(slot) = fills.iter_mut().find_map(|f| match f {
        PenFill::Solid(body) => Some(body),
        _ => None,
    }) {
        slot.color = hex.to_string();
    } else {
        fills.insert(0, solid_fill(hex.to_string()));
    }
    true
}
