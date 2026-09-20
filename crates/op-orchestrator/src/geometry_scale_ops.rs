//! Table-overflow scaling: the scale-op collector and the per-table scale
//! factor it derives.

use super::*;

pub(super) fn collect_scale_ops(
    v: &Value,
    rects: &HashMap<String, Rect>,
    ops: &mut Vec<EditorCommand>,
) {
    if let Some(scale) = table_overflow_scale(v, rects) {
        // Apply the same scale to EVERY row's fixed cells (columns stay aligned)
        // and to each row's gap.
        for row in table_rows(v) {
            let cells = children(row);
            for cell in cells {
                if let (Some(w), Some(id)) =
                    (fixed_width(cell), cell.get("id").and_then(Value::as_str))
                {
                    ops.push(EditorCommand::UpdateNode {
                        node_id: NodeId::new(id.to_string()),
                        x: None,
                        y: None,
                        width: Some((w * scale).round() as i32),
                        height: None,
                        name: None,
                        fill_hex: None,
                        page_id: None,
                    });
                }
            }
            let gap = num(row, "gap");
            if gap > 0.0 {
                if let Some(id) = row.get("id").and_then(Value::as_str) {
                    ops.push(EditorCommand::SetNodeLayoutProp {
                        node_id: NodeId::new(id.to_string()),
                        property: "gap".to_string(),
                        value: LayoutPropValue::Number((gap * scale).round()),
                    });
                }
            }
        }
    }
    for c in children(v) {
        collect_scale_ops(c, rects, ops);
    }
}

/// If `v` is a table-shaped container (a repeated contiguous run of ≥2
/// horizontal rows with ≥3 cells and matching width modes — the STRUCTURE is
/// the gate, not the name; "VIP Client List" shipped a starved 6px email column
/// because a name gate only trusted `table`-named frames) whose fixed columns
/// crowd out the rows' RESOLVED inner width, return the scale factor (< 1.0) to
/// apply to its fixed columns + gap. Each row is measured against its own inner
/// width (rect minus padding) and each text-bearing flex column reserves a
/// readable floor; the WORST row decides, so uneven header/data column sets
/// can't hide the deficit. `None` when the shape isn't a table or everything
/// fits.
pub(super) fn table_overflow_scale(v: &Value, rects: &HashMap<String, Rect>) -> Option<f64> {
    let rows = table_rows(v);
    if rows.is_empty() {
        return None;
    }
    let mut worst: Option<f64> = None;
    for row in rows {
        let cells = children(row);
        let n_gaps = (cells.len() - 1) as f64;
        let gap = num(row, "gap");
        let mut fixed_sum = 0.0;
        let mut flex_floor = 0.0;
        for cell in cells {
            match fixed_width(cell) {
                Some(w) => fixed_sum += w,
                // fill_container / fit_content — reserve room for it.
                None => {
                    flex_floor += if bears_text(cell) {
                        MIN_FILL_TEXT_COL
                    } else {
                        MIN_FILL_COL
                    }
                }
            }
        }
        if fixed_sum <= 0.0 {
            continue; // all-flex row can't overflow via fixed widths
        }
        let Some(row_id) = row.get("id").and_then(Value::as_str) else {
            continue;
        };
        let Some(row_w) = rects.get(row_id).map(|r| r.w - horizontal_padding(row)) else {
            continue;
        };
        if row_w <= 1.0 {
            continue;
        }
        // Minimum width the row NEEDS: fixed columns + gaps + the flex floors.
        // If that already fits the resolved inner width, this row is fine.
        let needed = fixed_sum + gap * n_gaps + flex_floor;
        if needed <= row_w + OVERFLOW_EPS {
            continue;
        }
        // Scale the fixed budget (columns + gaps) to fit alongside the floors.
        let fixed_budget = (row_w - flex_floor) * FIT_MARGIN;
        let scalable = fixed_sum + gap * n_gaps;
        if scalable <= 0.0 {
            continue;
        }
        // UNSALVAGEABLE by scaling: even at MIN_SCALE the fixed budget can't
        // fit beside the flex floors (a 6-column table crammed into a
        // half-width pane — its five text-bearing fill columns alone need
        // more than the row offers). Scaling anyway is worse than useless:
        // the geometry loop re-applies the scale EVERY round, compounding
        // 0.35ⁿ and crushing the column gap to a sliver (24→3, measured).
        // Leave the row alone and let the too-many-columns diagnostic speak.
        if fixed_budget / scalable < MIN_SCALE {
            continue;
        }
        let scale = (fixed_budget / scalable).clamp(MIN_SCALE, 1.0);
        if scale < 1.0 - 0.001 {
            worst = Some(worst.map_or(scale, |w: f64| w.min(scale)));
        }
    }
    worst
}
