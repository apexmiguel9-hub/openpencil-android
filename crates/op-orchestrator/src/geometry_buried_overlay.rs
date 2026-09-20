//! Buried-overlay repair — a `layout:none` overlay painted over by an opaque
//! sibling that sits EARLIER in the child array.
//!
//! The canvas paints absolute-stack siblings in reverse index order
//! (`canvas_viewport_paint_mask::paint_child_siblings`, `.enumerate().rev()`),
//! so **`children[0]` is topmost**. `skills/phases/agent/design-agent.md`
//! states this to the model — "Put badges, labels, controls, scrims, and other
//! overlays BEFORE the full-bleed image/background they must cover; repair a
//! hidden overlay with `M(overlayId, stackId, 0)`" — but it is the one
//! convention that is inverted from every other tool a model has seen, and a
//! model that authors the base first and the overlays after gets a page whose
//! controls silently vanish under the artwork.
//!
//! Measured on `0808-k3-2.op`'s 星图 screen: a `layout:none` starfield
//! container holding `[circle, compass label, zoom control, gyro pill]`. The
//! 297px circle is `children[0]` — topmost — with an opaque radial gradient,
//! so it paints over all three controls. Only the slivers falling outside the
//! circle survived; the gyro pill showed its icon and half a glyph and nothing
//! else, which is what the user saw and reported as "cut in half".
//!
//! **Why this is contract, not taste.** A node carrying an icon or a label,
//! given an explicit position and its own shadow, and then painted over by an
//! opaque sibling, is not a composition — the author cannot see it. The
//! deliberate version of "a later sibling is covered" is the DECK: a back
//! layer peeking behind a front card. The corpus requires those back layers to
//! be decorative and EMPTY ("NEVER text/icon/content children",
//! `layout.md`), and the ring/gauge stacks are `ellipse`s with no children at
//! all. So "the buried node bears content" is exactly the line between the
//! two, and it is the only gate this pass needs beyond the geometry.
//!
//! Repair: move the buried overlay ahead of whatever covers it — the same
//! `M(overlayId, stackId, 0)` the corpus prescribes, expressed as
//! `EditorCommand::MoveNode { index: Some(0) }`.

use std::collections::HashMap;

use op_editor_core::{EditorCommand, NodeId};
use serde_json::Value;

use super::{children, layout_str, Rect};

/// The covering sibling must hide at least this fraction of the overlay's own
/// area. A corner badge deliberately half-tucked behind a card edge is a
/// composition; a control with three quarters of itself gone is not.
const MIN_BURIED_FRACTION: f64 = 0.6;

/// An OVERLAY is a small thing placed ON a large surface: a badge, a label, a
/// control. It must be at most this fraction of the area of whatever covers it.
///
/// This is the gate that separates the two shapes, and `bears_content` alone is
/// not enough for it. A deck's peeked back card is a PEER of the front card —
/// near-identical size — and the measured `0724-1-gm-2` deck puts an example
/// sentence on that back layer, so "carries content" is true of both. Their
/// areas are not: the gyro pill is 3.6% of the starfield circle it vanished
/// under, while the back card is 92% of the front card that peeks over it.
/// Comparable size means peers in a stack, and their order is a composition.
const OVERLAY_MAX_AREA_RATIO: f64 = 0.5;

/// Does this node paint an opaque surface — something that can actually hide
/// what is behind it? A fill array with any entry counts; a translucent hex
/// (8-digit with a low alpha) does not.
fn paints_opaque(v: &Value) -> bool {
    let Some(first) = v
        .get("fill")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
    else {
        return false;
    };
    match first.get("type").and_then(Value::as_str) {
        Some("linear_gradient" | "radial_gradient" | "mesh_gradient" | "image") => true,
        Some("solid") => first
            .get("color")
            .and_then(Value::as_str)
            .map(|color| {
                // `#RRGGBBAA` — treat anything under ~0.8 alpha as see-through.
                let hex = color.trim();
                if hex.len() != 9 || !hex.starts_with('#') {
                    return true; // token or 6-digit hex: opaque
                }
                u8::from_str_radix(&hex[7..9], 16)
                    .map(|a| a >= 0xCC)
                    .unwrap_or(true)
            })
            .unwrap_or(false),
        _ => false,
    }
}

/// Does this subtree carry something a reader is meant to SEE — a glyph, an
/// icon, an image? The deck's decorative back layers and the ring stacks'
/// bare ellipses deliberately carry nothing.
fn bears_content(v: &Value) -> bool {
    if matches!(
        v.get("type").and_then(Value::as_str),
        Some("text" | "icon_font" | "image")
    ) {
        return true;
    }
    children(v).iter().any(bears_content)
}

/// Fraction of `overlay` hidden by `cover`.
fn covered_fraction(overlay: &Rect, cover: &Rect) -> f64 {
    let w = (overlay.x + overlay.w).min(cover.x + cover.w) - overlay.x.max(cover.x);
    let h = (overlay.y + overlay.h).min(cover.y + cover.h) - overlay.y.max(cover.y);
    if w <= 0.0 || h <= 0.0 {
        return 0.0;
    }
    let area = overlay.w * overlay.h;
    if area <= 0.0 {
        return 0.0;
    }
    (w * h) / area
}

fn rect_of<'a>(v: &Value, rects: &'a HashMap<String, Rect>) -> Option<&'a Rect> {
    v.get("id")
        .and_then(Value::as_str)
        .and_then(|id| rects.get(id))
}

/// Emit a `MoveNode` to index 0 for every content-bearing overlay buried under
/// an opaque earlier sibling of the same `layout:none` stack.
pub(super) fn collect_buried_overlay_fixes(
    v: &Value,
    rects: &HashMap<String, Rect>,
    cmds: &mut Vec<EditorCommand>,
) {
    if layout_str(v) == Some("none") {
        let kids = children(v);
        // Later index = painted EARLIER = further back. Walk from the back
        // forward so the rescued overlays keep their relative order once each
        // lands at index 0.
        for (index, overlay) in kids.iter().enumerate().rev() {
            if index == 0 || !bears_content(overlay) {
                continue;
            }
            let Some(overlay_rect) = rect_of(overlay, rects) else {
                continue;
            };
            let overlay_area = overlay_rect.w * overlay_rect.h;
            let buried = kids[..index].iter().any(|cover| {
                paints_opaque(cover)
                    && rect_of(cover, rects).is_some_and(|c| {
                        covered_fraction(overlay_rect, c) >= MIN_BURIED_FRACTION
                            && overlay_area <= c.w * c.h * OVERLAY_MAX_AREA_RATIO
                    })
            });
            if !buried {
                continue;
            }
            let (Some(stack_id), Some(overlay_id)) = (
                v.get("id").and_then(Value::as_str),
                overlay.get("id").and_then(Value::as_str),
            ) else {
                continue;
            };
            cmds.push(EditorCommand::MoveNode {
                node_id: NodeId::new(overlay_id.to_string()),
                target_parent: NodeId::new(stack_id.to_string()),
                page_id: None,
                index: Some(0),
            });
        }
    }
    for c in children(v) {
        collect_buried_overlay_fixes(c, rects, cmds);
    }
}

#[cfg(test)]
#[path = "geometry_buried_overlay_tests.rs"]
mod tests;
