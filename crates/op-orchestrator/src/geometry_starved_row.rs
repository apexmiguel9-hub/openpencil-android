//! Starved rigid-row repair — a `fill_container` row of FIXED columns that
//! the flex solver squeezed below its own content width.
//!
//! `collect_row_overfull_fixes` repairs an overfull row from the INSIDE, by
//! retargeting its widest rigid child to `fill_container` so flex min-size 0
//! absorbs the deficit. That has two dead ends, and a footer hit both at once
//! (measured, `0808-gm-2.op`):
//!
//! * every column was 96px — under `MIN_FLEXIFY_W`, so there was no candidate
//!   to flexify; and
//! * the fallback (`stack_overfull_value_chip_row`) only handles a 2-child
//!   value+chip pair, and the row had 4 children.
//!
//! Squeezing from the inside would have been the wrong move anyway. The row's
//! children were not too wide — the ROW was too narrow, because it declared
//! `fill_container` next to a second `fill_container` sibling and the solver
//! split the free space evenly (340px each) with no regard for the 528px its
//! rigid columns actually need. The 188px difference did not clip: it spilled
//! straight across the sibling column, and the newsletter block painted on top
//! of the legal links. Four text pairs rendered on the same pixels.
//!
//! The contract this restores: **a container whose children are all rigid
//! cannot be narrower than their sum.** Asking such a box to `fill_container`
//! alongside a genuinely flexible sibling is unsatisfiable, and honouring the
//! content is the only reading that does not produce unreadable output. So the
//! repair demotes the rigid row to `fit_content` — it takes exactly what it
//! needs, and the sibling that CAN flex absorbs the remainder.
//!
//! Deliberately not clipping: `clipContent` on the row would hide the tail
//! columns, which is no more readable than the overlap it replaces.

use super::*;

/// A row must miss its content width by more than this before it counts as
/// starved — keeps sub-pixel and rounding differences out.
const STARVED_ROW_EPS: f64 = 8.0;

/// Is this child rigid — i.e. incapable of giving width back under pressure?
/// A numeric width, `fit_content`, or an absent width all resolve to a size
/// the solver will not shrink below on our behalf. Only `fill_container`
/// children can absorb a deficit.
fn is_rigid_child(v: &Value) -> bool {
    match v.get("width") {
        None | Some(Value::Null) => true,
        Some(Value::Number(_)) => true,
        Some(Value::String(s)) => s != "fill_container",
        Some(_) => false,
    }
}

fn is_flexible(v: &Value) -> bool {
    v.get("width").and_then(Value::as_str) == Some("fill_container")
}

/// Width this row needs to lay its children out without overflow: the sum of
/// their resolved widths, plus its gaps, plus its own horizontal padding.
fn required_width(v: &Value, rects: &HashMap<String, Rect>) -> Option<f64> {
    let kids: Vec<&Value> = children(v)
        .iter()
        .filter(|c| !super::geometry_row_fixes::has_authored_position(c))
        .collect();
    if kids.len() < 2 {
        return None;
    }
    let mut sum = 0.0;
    for kid in &kids {
        let rect = kid
            .get("id")
            .and_then(Value::as_str)
            .and_then(|id| rects.get(id))?;
        sum += rect.w;
    }
    sum += num(v, "gap") * (kids.len() - 1) as f64;
    Some(sum + horizontal_padding(v))
}

/// Is `v` a horizontal row of exclusively rigid children — a box whose width
/// is dictated by its content and cannot be negotiated down?
fn is_rigid_row(v: &Value) -> bool {
    if layout_str(v) != Some("horizontal") {
        return false;
    }
    if v.get("clipContent").and_then(Value::as_bool) == Some(true) {
        return false; // an author-declared scroller may narrow on purpose
    }
    let kids = children(v);
    kids.len() >= 2 && kids.iter().all(is_rigid_child)
}

/// Demote a starved rigid row from `fill_container` to `fit_content`.
///
/// Runs from the PARENT's vantage point — the sibling set is what proves the
/// space is recoverable, and a per-node walk cannot see it.
pub(super) fn collect_starved_rigid_row_fixes(
    v: &Value,
    rects: &HashMap<String, Rect>,
    cmds: &mut Vec<EditorCommand>,
) {
    if layout_str(v) == Some("horizontal") {
        let kids = children(v);
        // Total width currently handed to the flexible children. Demoting one
        // of them can only redistribute space that already belongs to this
        // group, so it is also the ceiling on what the repair can recover.
        let flexible_budget: f64 = kids
            .iter()
            .filter(|c| is_flexible(c))
            .filter_map(|c| {
                c.get("id")
                    .and_then(Value::as_str)
                    .and_then(|id| rects.get(id))
            })
            .map(|r| r.w)
            .sum();
        for kid in kids {
            if !is_flexible(kid) || !is_rigid_row(kid) {
                continue;
            }
            let Some(resolved) = kid
                .get("id")
                .and_then(Value::as_str)
                .and_then(|id| rects.get(id))
                .map(|r| r.w)
            else {
                continue;
            };
            let Some(required) = required_width(kid, rects) else {
                continue;
            };
            if required <= resolved + STARVED_ROW_EPS {
                continue; // not starved
            }
            // A second flexible sibling must exist to absorb what this row
            // takes back; without one, `fit_content` would simply move the
            // overflow up a level instead of resolving it.
            if flexible_budget <= resolved + STARVED_ROW_EPS {
                continue;
            }
            // …and the space must actually be enough. When even the whole
            // flexible budget cannot seat this row, the design needs FEWER
            // columns — an intent call the diagnostics already report to the
            // model rather than a width this pass can invent.
            if required > flexible_budget {
                continue;
            }
            if let Some(id) = kid.get("id").and_then(Value::as_str) {
                cmds.push(EditorCommand::SetNodeLayoutProp {
                    node_id: NodeId::new(id.to_string()),
                    property: "width".to_string(),
                    value: LayoutPropValue::Keyword("fit_content".to_string()),
                });
            }
        }
    }
    for c in children(v) {
        collect_starved_rigid_row_fixes(c, rects, cmds);
    }
}

#[cfg(test)]
#[path = "geometry_starved_row_tests.rs"]
mod tests;
