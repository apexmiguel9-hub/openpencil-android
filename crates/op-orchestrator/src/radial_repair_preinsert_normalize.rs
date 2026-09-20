//! Pre-insert normalization for radial shapes that the strict/force-center
//! passes cannot safely interpret as one compact `centre + progress + track`
//! stack.
//!
//! Two weak-model shapes are common:
//! - a real ring whose wrapper axis is `fill_container` / `fit_content`;
//! - an annotated sunrise/sunset arc whose track/progress ellipses are direct
//!   siblings of several independently positioned labels and icons.
//!
//! For the first shape the authored arc pair is the fixed geometry: infer only
//! missing wrapper axes from its bounds, then let the existing concentric pass
//! finish the repair. For the second, keep annotations on their existing
//! parent and wrap only the arc pair in a fixed `layout:none` stack. This
//! avoids treating every annotation as ambiguous centre content.

use serde_json::{Map, Value};

use super::{
    children, estimated_subtree_size, numeric, radial_layers, valid_size, ArcLayer, RadialLayers,
    MIN_SAFE_ARC_DIAMETER_RATIO,
};

#[derive(Clone, Copy)]
struct ArcGeometry {
    index: usize,
    layer: ArcLayer,
    x: Option<f64>,
    y: Option<f64>,
    width: f64,
    height: f64,
}

pub(super) fn normalize_extended_radial_stack(node: &mut Value) -> bool {
    let Some(layers) = radial_layers(node) else {
        return false;
    };
    let Some(arcs) = collect_arc_geometry(node, &layers) else {
        return false;
    };
    if !arcs_are_compatible(&arcs) || !explicit_arc_centres_are_compatible(&arcs) {
        return false;
    }

    if layers.centres.len() > 1 {
        if should_preserve_as_annotations(node, &layers) {
            return wrap_arc_layers(node, &arcs);
        }
        // Authored positions or solar/annotation semantics describe content
        // around the arc, not a centre stack. If that geometry is not already
        // an absolute annotated wrapper, keep the issue fatal so a retry can
        // rebuild it instead of silently collapsing the annotations.
        if centres_have_explicit_positions(node, &layers) || is_annotated_arc_semantic(node) {
            return false;
        }
        let grouped = group_centre_layers(node, &layers, &arcs);
        let sized = infer_fluid_wrapper_axes(node, &arcs);
        return grouped || sized;
    }

    infer_fluid_wrapper_axes(node, &arcs)
}

fn collect_arc_geometry(node: &Value, layers: &RadialLayers) -> Option<Vec<ArcGeometry>> {
    let kids = children(node);
    let mut arcs = Vec::with_capacity(layers.progress.len() + layers.tracks.len());
    for (index, layer) in layers
        .progress
        .iter()
        .copied()
        .map(|index| (index, ArcLayer::Progress))
        .chain(
            layers
                .tracks
                .iter()
                .copied()
                .map(|index| (index, ArcLayer::Track)),
        )
    {
        let child = kids.get(index)?;
        let estimate = estimated_subtree_size(child);
        let width = numeric(child, "width")
            .or_else(|| numeric(child, "height"))
            .or_else(|| estimate.map(|size| size.0))?;
        let height = numeric(child, "height")
            .or_else(|| numeric(child, "width"))
            .or_else(|| estimate.map(|size| size.1))?;
        if !valid_size(width, height) {
            return None;
        }
        arcs.push(ArcGeometry {
            index,
            layer,
            x: numeric(child, "x"),
            y: numeric(child, "y"),
            width,
            height,
        });
    }
    (!arcs.is_empty()).then_some(arcs)
}

fn arcs_are_compatible(arcs: &[ArcGeometry]) -> bool {
    let min_w = arcs
        .iter()
        .map(|arc| arc.width)
        .fold(f64::INFINITY, f64::min);
    let max_w = arcs.iter().map(|arc| arc.width).fold(0.0, f64::max);
    let min_h = arcs
        .iter()
        .map(|arc| arc.height)
        .fold(f64::INFINITY, f64::min);
    let max_h = arcs.iter().map(|arc| arc.height).fold(0.0, f64::max);
    min_w.is_finite()
        && min_h.is_finite()
        && max_w > 0.0
        && max_h > 0.0
        && min_w / max_w >= MIN_SAFE_ARC_DIAMETER_RATIO
        && min_h / max_h >= MIN_SAFE_ARC_DIAMETER_RATIO
}

fn should_preserve_as_annotations(node: &Value, layers: &RadialLayers) -> bool {
    node.get("layout").and_then(Value::as_str) == Some("none")
        && is_annotated_arc_semantic(node)
        && centres_have_explicit_positions(node, layers)
}

fn centres_have_explicit_positions(node: &Value, layers: &RadialLayers) -> bool {
    let kids = children(node);
    layers.centres.iter().all(|index| {
        kids.get(*index).is_some_and(|child| {
            numeric(child, "x").is_some() && numeric(child, "y").is_some() && measurable(child)
        })
    })
}

fn measurable(node: &Value) -> bool {
    let estimate = estimated_subtree_size(node);
    let width = numeric(node, "width").or_else(|| estimate.map(|size| size.0));
    let height = numeric(node, "height").or_else(|| estimate.map(|size| size.1));
    width
        .zip(height)
        .is_some_and(|(width, height)| valid_size(width, height))
}

fn is_annotated_arc_semantic(node: &Value) -> bool {
    let semantic = ["name", "role"]
        .into_iter()
        .filter_map(|key| node.get(key).and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    [
        "sunrise",
        "sunset",
        "sun arc",
        "solar",
        "daylight",
        "annotated",
    ]
    .iter()
    .any(|needle| semantic.contains(needle))
}

fn infer_fluid_wrapper_axes(node: &mut Value, arcs: &[ArcGeometry]) -> bool {
    let arc_w = arcs.iter().map(|arc| arc.width).fold(0.0, f64::max);
    let arc_h = arcs.iter().map(|arc| arc.height).fold(0.0, f64::max);
    if !valid_size(arc_w, arc_h) {
        return false;
    }

    let mut changed = false;
    if numeric(node, "width").is_none() {
        node["width"] = Value::from(arc_w.round());
        changed = true;
    }
    if numeric(node, "height").is_none() {
        node["height"] = Value::from(arc_h.round());
        changed = true;
    }
    changed
}

fn group_centre_layers(node: &mut Value, layers: &RadialLayers, arcs: &[ArcGeometry]) -> bool {
    let kids = children(node);
    let centre_sizes: Option<Vec<(f64, f64)>> = layers
        .centres
        .iter()
        .map(|index| {
            let child = kids.get(*index)?;
            let estimate = estimated_subtree_size(child);
            let width = numeric(child, "width").or_else(|| estimate.map(|size| size.0))?;
            let height = numeric(child, "height").or_else(|| estimate.map(|size| size.1))?;
            valid_size(width, height).then_some((width, height))
        })
        .collect();
    let Some(centre_sizes) = centre_sizes else {
        return false;
    };
    let gap = 2.0;
    let centre_w = centre_sizes.iter().map(|size| size.0).fold(0.0, f64::max);
    let centre_h = centre_sizes.iter().map(|size| size.1).sum::<f64>()
        + gap * centre_sizes.len().saturating_sub(1) as f64;
    let arc_w = arcs.iter().map(|arc| arc.width).fold(0.0, f64::max);
    let arc_h = arcs.iter().map(|arc| arc.height).fold(0.0, f64::max);
    if !valid_size(centre_w, centre_h) || centre_w > arc_w + 0.5 || centre_h > arc_h + 0.5 {
        return false;
    }

    let centre_id = unique_descendant_id(node, "radial-centre");
    let mut old = node
        .get_mut("children")
        .and_then(Value::as_array_mut)
        .map(std::mem::take)
        .unwrap_or_default();
    let mut slots: Vec<Option<Value>> = old.iter_mut().map(Value::take).map(Some).collect();
    let mut centre_children = Vec::with_capacity(layers.centres.len());
    for index in &layers.centres {
        if let Some(child) = slots.get_mut(*index).and_then(Option::take) {
            centre_children.push(child);
        }
    }

    let mut centre = Map::new();
    centre.insert("type".into(), Value::String("frame".into()));
    centre.insert("id".into(), Value::String(centre_id));
    centre.insert("name".into(), Value::String("Radial Centre".into()));
    centre.insert("width".into(), Value::from(centre_w.round()));
    centre.insert("height".into(), Value::from(centre_h.round()));
    centre.insert("layout".into(), Value::String("vertical".into()));
    centre.insert("gap".into(), Value::from(gap));
    centre.insert("justifyContent".into(), Value::String("center".into()));
    centre.insert("alignItems".into(), Value::String("center".into()));
    centre.insert("children".into(), Value::Array(centre_children));

    let mut rebuilt = vec![Value::Object(centre)];
    for arc in arcs
        .iter()
        .filter(|arc| same_layer(arc.layer, ArcLayer::Progress))
        .chain(
            arcs.iter()
                .filter(|arc| same_layer(arc.layer, ArcLayer::Track)),
        )
    {
        if let Some(child) = slots.get_mut(arc.index).and_then(Option::take) {
            rebuilt.push(child);
        }
    }
    node["children"] = Value::Array(rebuilt);
    true
}

fn wrap_arc_layers(node: &mut Value, arcs: &[ArcGeometry]) -> bool {
    let Some((anchor_x, anchor_y, stack_w, stack_h)) = stack_bounds(arcs) else {
        return false;
    };
    if !explicit_arc_centres_are_compatible(arcs) {
        return false;
    }

    let stack_id = unique_descendant_id(node, "radial-stack");
    let original_layout_none = node.get("layout").and_then(Value::as_str) == Some("none");
    let mut old = node
        .get_mut("children")
        .and_then(Value::as_array_mut)
        .map(std::mem::take)
        .unwrap_or_default();

    let mut arc_slots: Vec<Option<Value>> = old.iter_mut().map(Value::take).map(Some).collect();
    let mut stack_children = Vec::with_capacity(arcs.len());
    for layer in [ArcLayer::Progress, ArcLayer::Track] {
        for arc in arcs.iter().filter(|arc| same_layer(arc.layer, layer)) {
            let Some(mut child) = arc_slots.get_mut(arc.index).and_then(Option::take) else {
                continue;
            };
            child["x"] = Value::from(((stack_w - arc.width) / 2.0).round());
            child["y"] = Value::from(((stack_h - arc.height) / 2.0).round());
            if numeric(&child, "width").is_none() {
                child["width"] = Value::from(arc.width.round());
            }
            if numeric(&child, "height").is_none() {
                child["height"] = Value::from(arc.height.round());
            }
            stack_children.push(child);
        }
    }

    let mut stack = Map::new();
    stack.insert("type".into(), Value::String("frame".into()));
    stack.insert("id".into(), Value::String(stack_id));
    stack.insert("name".into(), Value::String("Radial Stack".into()));
    if original_layout_none {
        stack.insert("x".into(), Value::from(anchor_x.round()));
        stack.insert("y".into(), Value::from(anchor_y.round()));
    }
    stack.insert("width".into(), Value::from(stack_w.round()));
    stack.insert("height".into(), Value::from(stack_h.round()));
    stack.insert("layout".into(), Value::String("none".into()));
    stack.insert("gap".into(), Value::from(0.0));
    stack.insert("justifyContent".into(), Value::String("start".into()));
    stack.insert("alignItems".into(), Value::String("start".into()));
    stack.insert("children".into(), Value::Array(stack_children));

    // Lower child indexes paint in front. Keep every positioned annotation
    // above the arc stack so markers that overlap the stroke remain visible.
    let mut rebuilt = Vec::with_capacity(old.len() - arcs.len() + 1);
    for index in 0..old.len() {
        if let Some(value) = arc_slots.get_mut(index).and_then(Option::take) {
            rebuilt.push(value);
        }
    }
    rebuilt.push(Value::Object(stack));
    node["children"] = Value::Array(rebuilt);
    true
}

fn stack_bounds(arcs: &[ArcGeometry]) -> Option<(f64, f64, f64, f64)> {
    let stack_w = arcs.iter().map(|arc| arc.width).fold(0.0, f64::max);
    let stack_h = arcs.iter().map(|arc| arc.height).fold(0.0, f64::max);
    if !valid_size(stack_w, stack_h) {
        return None;
    }
    let anchor_x = arcs
        .iter()
        .filter_map(|arc| arc.x)
        .fold(f64::INFINITY, f64::min);
    let anchor_y = arcs
        .iter()
        .filter_map(|arc| arc.y)
        .fold(f64::INFINITY, f64::min);
    Some((
        if anchor_x.is_finite() { anchor_x } else { 0.0 },
        if anchor_y.is_finite() { anchor_y } else { 0.0 },
        stack_w,
        stack_h,
    ))
}

fn explicit_arc_centres_are_compatible(arcs: &[ArcGeometry]) -> bool {
    let centres: Vec<(f64, f64)> = arcs
        .iter()
        .filter_map(|arc| Some((arc.x? + arc.width / 2.0, arc.y? + arc.height / 2.0)))
        .collect();
    if centres.len() < 2 {
        return true;
    }
    let first = centres[0];
    let tolerance = arcs
        .iter()
        .map(|arc| arc.width.min(arc.height))
        .fold(f64::INFINITY, f64::min)
        * 0.1;
    centres.iter().skip(1).all(|centre| {
        (centre.0 - first.0).abs() <= tolerance && (centre.1 - first.1).abs() <= tolerance
    })
}

fn same_layer(left: ArcLayer, right: ArcLayer) -> bool {
    matches!(
        (left, right),
        (ArcLayer::Progress, ArcLayer::Progress) | (ArcLayer::Track, ArcLayer::Track)
    )
}

fn unique_descendant_id(node: &Value, suffix: &str) -> String {
    let base = node
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .unwrap_or("radial");
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

#[cfg(test)]
#[path = "radial_repair_preinsert_normalize_tests.rs"]
mod tests;
