//! Conservative recovery for unnamed partial track/progress pairs.

use op_editor_core::{EditorCommand, NodeId};
use serde_json::Value;

use super::{near_square, numeric, semantic_arc_layer};

const GEOMETRY_EPSILON: f64 = 0.5;

/// Infer the larger partial sweep as the track only when two otherwise
/// unnamed arcs already prove they are the same authored ring: equal square
/// bounds, centre, start angle, and inner radius inside `layout:none`.
pub(super) fn infer_unnamed_partial_pair(
    parent: &Value,
    kids: &[Value],
    progress: &[usize],
    tracks: &[usize],
) -> Option<(usize, usize)> {
    if parent.get("layout").and_then(Value::as_str) != Some("none")
        || !tracks.is_empty()
        || progress.len() != 2
    {
        return None;
    }
    let first_index = progress[0];
    let second_index = progress[1];
    let first = kids.get(first_index)?;
    let second = kids.get(second_index)?;
    if semantic_arc_layer(first).is_some() || semantic_arc_layer(second).is_some() {
        return None;
    }

    let (first_w, first_h) = (numeric(first, "width")?, numeric(first, "height")?);
    let (second_w, second_h) = (numeric(second, "width")?, numeric(second, "height")?);
    if !near_square(first_w, first_h)
        || !near_square(second_w, second_h)
        || !near(first_w, second_w)
        || !near(first_h, second_h)
        || !same_number(first, second, "x")
        || !same_number(first, second, "y")
        || !same_number(first, second, "innerRadius")
        || !same_angle(first, second, "startAngle")
    {
        return None;
    }

    let first_sweep = numeric(first, "sweepAngle")?.abs();
    let second_sweep = numeric(second, "sweepAngle")?.abs();
    if !(0.01..359.5).contains(&first_sweep)
        || !(0.01..359.5).contains(&second_sweep)
        || near(first_sweep, second_sweep)
    {
        return None;
    }
    if first_sweep < second_sweep {
        Some((first_index, second_index))
    } else {
        Some((second_index, first_index))
    }
}

/// Late sink repair works on an already inserted tree, so mirror the pure
/// authored repair's canonical centre/progress/track permutation with moves.
pub(super) fn canonical_reorder_commands(
    parent_id: &str,
    kids: &[Value],
    order: &[usize],
) -> Vec<EditorCommand> {
    if order.iter().copied().eq(0..kids.len()) {
        return Vec::new();
    }
    let desired: Option<Vec<&str>> = order
        .iter()
        .map(|index| kids.get(*index)?.get("id")?.as_str())
        .collect();
    desired
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .map(|(index, node_id)| EditorCommand::MoveNode {
            node_id: NodeId::new(node_id.to_string()),
            target_parent: NodeId::new(parent_id.to_string()),
            page_id: None,
            index: Some(index),
        })
        .collect()
}

fn same_number(first: &Value, second: &Value, key: &str) -> bool {
    numeric(first, key)
        .zip(numeric(second, key))
        .is_some_and(|(left, right)| near(left, right))
}

fn same_angle(first: &Value, second: &Value, key: &str) -> bool {
    numeric(first, key)
        .zip(numeric(second, key))
        .is_some_and(|(left, right)| {
            let delta = (left - right).rem_euclid(360.0);
            delta.min(360.0 - delta) <= GEOMETRY_EPSILON
        })
}

fn near(left: f64, right: f64) -> bool {
    left.is_finite() && right.is_finite() && (left - right).abs() <= GEOMETRY_EPSILON
}
