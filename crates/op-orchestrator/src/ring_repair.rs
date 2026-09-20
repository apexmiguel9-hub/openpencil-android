//! Ring fragments authored as FLEX SIBLINGS — extract them into a dedicated
//! concentric wrapper.
//!
//! A progress ring is two same-sized arc ellipses stacked on one centre: a
//! full track underneath, a partial progress arc on top, optional centre
//! label. The corpus teaches that shape explicitly (`layout.md`, plus the
//! worked example in `knowledge/shapes-and-decks.md`): a fixed-size
//! `layout:"none"` wrapper holding both arcs at the same coordinates, "never
//! as flex siblings". Models violate it anyway — they emit the track and the
//! progress arc as two direct children of an ordinary flex container, so the
//! layout engine lays them out SIDE BY SIDE and the design shows two circles
//! in a row with the percentage label floating off to one edge.
//!
//! [`crate::radial_repair`] already repairs one version of this: when the
//! offending parent IS the ring's own wrapper, it converts that parent in
//! place to `layout:"none"` and centres the children. But it deliberately
//! declines the case where the parent is a GENERAL-PURPOSE container — a
//! padded card, a section carrying its own heading, a KPI tile — because
//! converting such a parent would absolutely-position all of its unrelated
//! content on top of the ring. That decline is correct, and it is also the
//! hole the defect keeps coming back through: measured on the reproduction
//! fixture, a 320×160 card with `padding:[16,16]` holding
//! `[text "75%", progress arc, track]` is left completely untouched by every
//! existing pass, on every generation path.
//!
//! This module closes that hole from the other side: instead of reshaping the
//! parent, it EXTRACTS the arcs into a new fixed-size `layout:"none"` wrapper
//! that takes the first arc's slot in the parent's flow. The parent keeps its
//! own layout, padding, and every other child. [`radial_repair`] then sees a
//! textbook wrapper and finishes the concentric geometry against the resolved
//! rects.
//!
//! [`crate::radial_repair::parent_is_dedicated_ring_wrapper`] is the seam
//! between the two passes, so exactly one of them ever owns a given ring and
//! neither can drift from the other's idea of what a ring is.
//!
//! Detection is type + geometry + content only — no node-name heuristics. The
//! violated rule ("two arc ellipses of the same ring are not flex siblings")
//! is a contract the corpus states in so many words, and whether a node is an
//! arc, how big it is, and whether a label reads `NN%` are all facts, so the
//! repair is automatic. What the ring MEANS stays the model's call: nothing
//! here invents arcs, changes sweep angles, or recolours anything.

use jian_ops_schema::node::PenNode;
use serde_json::{Map, Value};

use crate::radial_repair::{
    estimated_subtree_size, near_square, parent_is_dedicated_ring_wrapper, radial_layers,
    MIN_SAFE_ARC_DIAMETER_RATIO,
};

/// One detected "ring fragments authored as flex siblings" violation.
pub(crate) struct RingFragments {
    pub(crate) parent_id: String,
    pub(crate) parent_name: String,
    /// Child indices of the arcs in CANONICAL PAINT ORDER — progress arcs
    /// first, then track arcs. Lower index paints on top (the canvas walks
    /// children in reverse), so this puts the partial arc over the full ring.
    pub(crate) arc_indices: Vec<usize>,
    /// The percentage label to adopt as centre content, when the parent's
    /// only non-arc child is a `NN%` text. `None` in every ambiguous case —
    /// then only the arcs are wrapped and no text moves.
    pub(crate) centre_index: Option<usize>,
    /// Diameter of the largest arc; the wrapper's fixed side length.
    pub(crate) diameter: f64,
}

/// Detect ring fragments sitting in `parent`'s FLEX flow.
///
/// Shared by the repair pass and the geometry diagnostic so the fix and the
/// echo can never disagree about what counts as a violation.
pub(crate) fn detect_ring_fragments(parent: &Value) -> Option<RingFragments> {
    if !matches!(
        parent.get("type").and_then(Value::as_str),
        Some("frame" | "group" | "rectangle")
    ) {
        return None;
    }
    // `layout:"none"` is already an overlay stack — the arcs are not being
    // flowed side by side, so there is nothing here to repair.
    if parent.get("layout").and_then(Value::as_str) == Some("none") {
        return None;
    }
    // `radial_layers` is the shared definition of "these children really are
    // one ring": it requires a track + partial-progress pair (or a segmented
    // donut) and refuses when any arc's layer is unclassifiable.
    let layers = radial_layers(parent)?;
    let arc_indices: Vec<usize> = layers
        .progress
        .iter()
        .chain(layers.tracks.iter())
        .copied()
        .collect();
    if arc_indices.len() < 2 {
        return None;
    }
    // Leave the whole-parent conversion case to `radial_repair`.
    if parent_is_dedicated_ring_wrapper(parent) {
        return None;
    }

    let kids = children(parent);
    let mut diameters = Vec::with_capacity(arc_indices.len());
    for index in &arc_indices {
        let child = kids.get(*index)?;
        let width = numeric(child, "width").or_else(|| numeric(child, "height"))?;
        let height = numeric(child, "height").or_else(|| numeric(child, "width"))?;
        if !near_square(width, height) {
            return None;
        }
        diameters.push(width.max(height));
    }
    let min_arc = diameters.iter().copied().fold(f64::INFINITY, f64::min);
    let max_arc = diameters.iter().copied().fold(0.0, f64::max);
    // Wildly different diameters are two unrelated gauges, not one ring.
    if !min_arc.is_finite() || max_arc <= 0.0 || min_arc / max_arc < MIN_SAFE_ARC_DIAMETER_RATIO {
        return None;
    }

    // Adopt a centre label only in the unambiguous shape: the arcs plus
    // exactly one other child, and that child reads as a percentage. Anything
    // richer (a heading, a caption, an icon) is content whose placement is the
    // model's intent, so it stays put and only the arcs are wrapped.
    let centre_index = match layers.centres.as_slice() {
        [only] => kids
            .get(*only)
            .is_some_and(is_percentage_label)
            .then_some(*only),
        _ => None,
    };

    Some(RingFragments {
        parent_id: parent
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        parent_name: parent
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("container")
            .to_string(),
        arc_indices,
        centre_index,
        diameter: max_arc,
    })
}

/// A short `NN%` / `NN.N%` label. This reads the node's own CONTENT — the
/// rendered characters — rather than its name, so it cannot misfire on a node
/// merely called "percent" nor miss one called "t3".
fn is_percentage_label(v: &Value) -> bool {
    if v.get("type").and_then(Value::as_str) != Some("text") {
        return false;
    }
    let Some(content) = v.get("content").and_then(Value::as_str).map(str::trim) else {
        return false;
    };
    let Some(number) = content.strip_suffix('%') else {
        return false;
    };
    let number = number.trim();
    !number.is_empty()
        && number.chars().count() <= 6
        && number.chars().any(|c| c.is_ascii_digit())
        && number.chars().all(|c| c.is_ascii_digit() || c == '.')
        && number.chars().filter(|c| *c == '.').count() <= 1
}

/// Extract every flex-sibling ring in `root`'s subtree into its own concentric
/// wrapper. Shaped as a `fn(&mut PenNode) -> bool` so the cleanup driver can
/// run it through `apply_root_transform` like the other structural repairs.
pub(crate) fn wrap_ring_fragments(root: &mut PenNode) -> bool {
    let Ok(mut value) = serde_json::to_value(&*root) else {
        return false;
    };
    if !wrap_in_value(&mut value) {
        return false;
    }
    match serde_json::from_value::<PenNode>(value) {
        Ok(rebuilt) => {
            *root = rebuilt;
            true
        }
        Err(_) => false,
    }
}

fn wrap_in_value(v: &mut Value) -> bool {
    let mut changed = false;
    if let Some(fragments) = detect_ring_fragments(v) {
        changed |= apply_wrapper(v, fragments);
    }
    // Recurse AFTER wrapping: the wrapper this level may have just created is
    // `layout:"none"`, so `detect_ring_fragments` declines it immediately and
    // the arcs are not re-wrapped.
    if let Some(kids) = v.get_mut("children").and_then(Value::as_array_mut) {
        for child in kids.iter_mut() {
            changed |= wrap_in_value(child);
        }
    }
    changed
}

fn apply_wrapper(parent: &mut Value, fragments: RingFragments) -> bool {
    let Some(insert_at) = fragments.arc_indices.iter().copied().min() else {
        return false;
    };
    let side = fragments.diameter;
    let wrapper_id = unique_descendant_id(parent, "ring-stack");

    let old = parent
        .get_mut("children")
        .and_then(Value::as_array_mut)
        .map(std::mem::take)
        .unwrap_or_default();
    let mut slots: Vec<Option<Value>> = old.into_iter().map(Some).collect();

    let mut wrapper_children = Vec::with_capacity(fragments.arc_indices.len() + 1);
    // Centre content first — index 0 is the TOP of the paint order, so the
    // label sits above both arcs.
    if let Some(centre) = fragments.centre_index {
        if let Some(mut label) = slots.get_mut(centre).and_then(Option::take) {
            let (width, height) = estimated_subtree_size(&label).unwrap_or((side, side));
            // Under `layout:"none"` a child has no flex parent to resolve
            // against — `fill_container` collapses to zero — so the centred
            // label needs explicit pixels on both axes.
            if numeric(&label, "width").is_none() {
                label["width"] = Value::from(width.round());
            }
            if numeric(&label, "height").is_none() {
                label["height"] = Value::from(height.round());
            }
            label["x"] = Value::from(((side - width) / 2.0).round());
            label["y"] = Value::from(((side - height) / 2.0).round());
            wrapper_children.push(label);
        }
    }
    for index in &fragments.arc_indices {
        let Some(mut arc) = slots.get_mut(*index).and_then(Option::take) else {
            continue;
        };
        let width = numeric(&arc, "width")
            .or_else(|| numeric(&arc, "height"))
            .unwrap_or(side);
        let height = numeric(&arc, "height")
            .or_else(|| numeric(&arc, "width"))
            .unwrap_or(side);
        arc["width"] = Value::from(width.round());
        arc["height"] = Value::from(height.round());
        arc["x"] = Value::from(((side - width) / 2.0).round());
        arc["y"] = Value::from(((side - height) / 2.0).round());
        wrapper_children.push(arc);
    }
    if wrapper_children.is_empty() {
        return false;
    }

    let mut wrapper = Map::new();
    wrapper.insert("type".into(), Value::String("frame".into()));
    wrapper.insert("id".into(), Value::String(wrapper_id));
    wrapper.insert("name".into(), Value::String("Ring Stack".into()));
    wrapper.insert("width".into(), Value::from(side.round()));
    wrapper.insert("height".into(), Value::from(side.round()));
    wrapper.insert("layout".into(), Value::String("none".into()));
    wrapper.insert("gap".into(), Value::from(0.0));
    wrapper.insert("justifyContent".into(), Value::String("start".into()));
    wrapper.insert("alignItems".into(), Value::String("start".into()));
    wrapper.insert("children".into(), Value::Array(wrapper_children));

    // The wrapper takes the first arc's slot, so the ring keeps its position
    // among the parent's remaining children.
    let mut rebuilt = Vec::with_capacity(slots.len());
    let mut wrapper = Some(Value::Object(wrapper));
    for (index, slot) in slots.iter_mut().enumerate() {
        if index == insert_at {
            if let Some(wrapper) = wrapper.take() {
                rebuilt.push(wrapper);
            }
        }
        if let Some(value) = slot.take() {
            rebuilt.push(value);
        }
    }
    if let Some(wrapper) = wrapper.take() {
        rebuilt.push(wrapper);
    }
    parent["children"] = Value::Array(rebuilt);
    true
}

/// Report — without repairing — every flex-sibling ring under `root`, so the
/// agentic loop's geometry echo shows the same fact the repair pass acts on.
/// Appends at most `remaining` entries, matching the shared diagnostics budget.
pub(crate) fn push_ring_fragment_diagnostics(root: &Value, out: &mut Vec<String>, max: usize) {
    collect_ring_fragment_diagnostics(root, out, max);
}

fn collect_ring_fragment_diagnostics(v: &Value, out: &mut Vec<String>, max: usize) {
    if out.len() >= max {
        return;
    }
    if let Some(fragments) = detect_ring_fragments(v) {
        out.push(format_fragments(&fragments));
    }
    for child in children(v) {
        collect_ring_fragment_diagnostics(child, out, max);
    }
}

fn format_fragments(fragments: &RingFragments) -> String {
    format!(
        "{} ({}): {} ring ellipses (~{}px across) are FLEX SIBLINGS in this container — a \
         progress ring's track and progress arc must be stacked concentrically, so they render \
         side by side as separate circles instead of as one ring; put both arcs at the same \
         coordinates inside a fixed {}x{} layout:\"none\" wrapper (paint order, index 0 on top: \
         centre label, progress arc, track){}",
        fragments.parent_name,
        fragments.parent_id,
        fragments.arc_indices.len(),
        fragments.diameter.round(),
        fragments.diameter.round(),
        fragments.diameter.round(),
        match fragments.centre_index {
            Some(_) => ", and move the percentage label inside that wrapper, centred",
            None => "",
        }
    )
}

fn unique_descendant_id(node: &Value, suffix: &str) -> String {
    let base = node
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .unwrap_or("ring");
    let mut candidate = format!("{base}__{suffix}");
    let mut serial = 2usize;
    while contains_id(node, &candidate) {
        candidate = format!("{base}__{suffix}-{serial}");
        serial += 1;
    }
    candidate
}

fn contains_id(node: &Value, id: &str) -> bool {
    node.get("id").and_then(Value::as_str) == Some(id)
        || children(node).iter().any(|child| contains_id(child, id))
}

fn children(v: &Value) -> &[Value] {
    v.get("children")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn numeric(v: &Value, key: &str) -> Option<f64> {
    match v.get(key) {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => s.parse().ok(),
        _ => None,
    }
}

#[cfg(test)]
#[path = "ring_repair_tests.rs"]
mod tests;
