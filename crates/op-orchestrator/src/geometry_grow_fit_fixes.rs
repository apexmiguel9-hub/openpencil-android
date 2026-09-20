//! Grow-to-fit and absolute fill-image fix collectors.

use super::*;

/// A fixed-height frame whose resolved CHILDREN run a LITTLE past its
/// declared height (a card estimated 156 tall whose art + two text lines
/// resolve to 165 — the artist line's bottom half vanished under the next
/// section, measured test0711-2-ds). Small overshoots grow the frame to
/// fit; big overshoots are the inflation class (content must shrink) and
/// stay with the echo above.
pub(super) const GROW_TO_FIT_MIN: f64 = 4.0;
pub(super) const GROW_TO_FIT_MAX_FRACTION: f64 = 0.25;

pub(super) fn collect_grow_to_fit_fixes(
    v: &Value,
    rects: &HashMap<String, Rect>,
    cmds: &mut Vec<EditorCommand>,
) {
    if let Some(declared) = match v.get("height") {
        Some(Value::Number(n)) => n.as_f64(),
        _ => None,
    } {
        if v.get("clipContent").and_then(Value::as_bool) != Some(true) && declared > 0.0 {
            if let Some(pr) = v
                .get("id")
                .and_then(Value::as_str)
                .and_then(|id| rects.get(id))
            {
                let children_bottom = children(v)
                    .iter()
                    .filter_map(|c| {
                        c.get("id")
                            .and_then(Value::as_str)
                            .and_then(|id| rects.get(id))
                    })
                    .map(|cr| cr.y + cr.h)
                    .fold(f64::MIN, f64::max);
                let overshoot = children_bottom - (pr.y + declared);
                if overshoot > GROW_TO_FIT_MIN && overshoot <= declared * GROW_TO_FIT_MAX_FRACTION {
                    if let Some(id) = v.get("id").and_then(Value::as_str) {
                        cmds.push(EditorCommand::UpdateNode {
                            node_id: NodeId::new(id.to_string()),
                            x: None,
                            y: None,
                            width: None,
                            height: Some((declared + overshoot).ceil() as i32),
                            name: None,
                            fill_hex: None,
                            page_id: None,
                        });
                    }
                }
            }
        }
    }
    for c in children(v) {
        collect_grow_to_fit_fixes(c, rects, cmds);
    }
}

/// A `fill_container`-sized IMAGE inside a `layout: "none"` (absolute)
/// container — `fill_container` has no meaning without a flex parent, so
/// the engine falls back to the bitmap's own aspect and the "cover" paints
/// as a skewed strip (measured: every New Releases cover rendered as a
/// thin right-edge sliver, test0711-22 00:44). The image is pinned to its
/// parent's RESOLVED rect: x/y 0, numeric width/height.
pub(super) fn collect_absolute_fill_image_fixes(
    v: &Value,
    rects: &HashMap<String, Rect>,
    cmds: &mut Vec<EditorCommand>,
) {
    let absolute_parent = matches!(layout_str(v), Some("none"));
    if absolute_parent {
        if let Some(pr) = v
            .get("id")
            .and_then(Value::as_str)
            .and_then(|id| rects.get(id))
        {
            if pr.w > 1.0 && pr.h > 1.0 {
                for c in children(v) {
                    if c.get("type").and_then(Value::as_str) != Some("image") {
                        continue;
                    }
                    let fill_w = c.get("width").and_then(Value::as_str) == Some("fill_container");
                    let fill_h = c.get("height").and_then(Value::as_str) == Some("fill_container");
                    if !fill_w && !fill_h {
                        continue;
                    }
                    let Some(cid) = c.get("id").and_then(Value::as_str) else {
                        continue;
                    };
                    cmds.push(EditorCommand::UpdateNode {
                        node_id: NodeId::new(cid.to_string()),
                        x: Some(0),
                        y: Some(0),
                        width: Some(pr.w.round() as i32),
                        height: Some(pr.h.round() as i32),
                        name: None,
                        fill_hex: None,
                        page_id: None,
                    });
                }
            }
        }
    }
    for c in children(v) {
        collect_absolute_fill_image_fixes(c, rects, cmds);
    }
}
