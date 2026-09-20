//! Tests for [`collect_rail_width_collapse_fixes`] / [`fix_rail_width_collapse`] —
//! the geometry-driven repair for a card rail whose `fill_container` siblings
//! collapse beside a fixed-width reference card. Root-cause fixture is a
//! de-identified extraction of a real user report (a finance-dashboard
//! "Savings Goals" rail on a 375px mobile page, produced by the
//! `minimal_skills` last-ditch retry rung): card 1 declared `width: 200`,
//! cards 2 and 3 declared `width: "fill_container"` — on the 327px-inner
//! rail left after page padding, card 1 alone ate 200px + 24px of gaps,
//! leaving only ~103px to split between the two `fill_container` cards
//! (~51px each), well past the point their own icon tile + title + amount
//! content could fit — hence truncated titles ("New Car" → "Nev Car") and a
//! ballooned card height from all that forced text wrapping.

use super::*;
use serde_json::json;
use std::collections::HashMap;

fn rects(entries: &[(&str, f64, f64, f64, f64)]) -> HashMap<String, Rect> {
    entries
        .iter()
        .map(|(id, x, y, w, h)| {
            (
                (*id).to_string(),
                Rect {
                    x: *x,
                    y: *y,
                    w: *w,
                    h: *h,
                },
            )
        })
        .collect()
}

fn find_by_name<'a>(v: &'a serde_json::Value, name: &str) -> Option<&'a serde_json::Value> {
    if v.get("name").and_then(|x| x.as_str()) == Some(name) {
        return Some(v);
    }
    v.get("children")
        .and_then(|c| c.as_array())
        .into_iter()
        .flatten()
        .find_map(|c| find_by_name(c, name))
}

fn find_id_by_name(v: &serde_json::Value, name: &str) -> Option<String> {
    find_by_name(v, name)
        .and_then(|n| n.get("id"))
        .and_then(|x| x.as_str())
        .map(String::from)
}

fn resolved_widths_by_name(
    state: &op_editor_core::EditorState,
    rects: &HashMap<String, Rect>,
    names: &[&str],
) -> Vec<f64> {
    let v = serde_json::to_value(state.active_children()[0].clone()).unwrap();
    names
        .iter()
        .map(|name| {
            let id = find_id_by_name(&v, name).unwrap_or_else(|| panic!("{name} exists"));
            rects
                .get(&id)
                .unwrap_or_else(|| panic!("{name} resolved"))
                .w
        })
        .collect()
}

fn rail(cards: Vec<serde_json::Value>) -> serde_json::Value {
    json!({
        "type": "frame", "id": "rail", "name": "Rail", "layout": "horizontal",
        "width": "fill_container", "gap": 12, "children": cards
    })
}

fn card(id: &str, width: serde_json::Value) -> serde_json::Value {
    json!({
        "type": "frame", "id": id, "name": id, "layout": "vertical",
        "width": width, "height": "fit_content", "cornerRadius": 12,
        "fill": [{"type": "solid", "color": "#FFFFFF"}], "children": []
    })
}

#[test]
fn collapsed_fill_container_sibling_is_widened_to_match_fixed_reference() {
    let row = rail(vec![
        card("c1", json!(200)),
        card("c2", json!("fill_container")),
        card("c3", json!("fill_container")),
    ]);
    // Matches the real fixture's arithmetic: c1 keeps its declared 200, c2/c3
    // squeeze to ~51px in a 327px-inner rail (200 + 2×12 gap + 2×51 ≈ 327).
    let rects = rects(&[
        ("c1", 0.0, 0.0, 200.0, 140.0),
        ("c2", 212.0, 0.0, 51.0, 640.0),
        ("c3", 275.0, 0.0, 51.0, 640.0),
    ]);
    let mut cmds = Vec::new();

    collect_rail_width_collapse_fixes(&row, &rects, &mut cmds);

    assert_eq!(
        cmds.len(),
        2,
        "both squeezed siblings must be widened: {cmds:?}"
    );
    for cmd in &cmds {
        match cmd {
            EditorCommand::UpdateNode { node_id, width, .. } => {
                assert!(
                    node_id.as_str() == "c2" || node_id.as_str() == "c3",
                    "only the collapsed siblings are touched, not the reference: {node_id:?}"
                );
                assert_eq!(
                    *width,
                    Some(200),
                    "widened to the reference card's declared width"
                );
            }
            other => panic!("expected UpdateNode (numeric width), got {other:?}"),
        }
    }
}

#[test]
fn evenly_split_rail_is_left_untouched() {
    // A healthy 3-up equal-share rail (no fixed reference at all): every
    // sibling gets a comfortable resolved width, nothing collapsed.
    let row = rail(vec![
        card("c1", json!("fill_container")),
        card("c2", json!("fill_container")),
        card("c3", json!("fill_container")),
    ]);
    let rects = rects(&[
        ("c1", 0.0, 0.0, 101.0, 140.0),
        ("c2", 113.0, 0.0, 101.0, 140.0),
        ("c3", 226.0, 0.0, 101.0, 140.0),
    ]);
    let mut cmds = Vec::new();

    collect_rail_width_collapse_fixes(&row, &rects, &mut cmds);

    assert!(
        cmds.is_empty(),
        "no fixed reference to normalize against — an equal-share rail is not this failure mode: {cmds:?}"
    );
}

#[test]
fn mild_width_variance_below_ratio_and_floor_is_not_flagged() {
    // A fixed reference IS present, but the fill_container sibling still
    // resolves to a comfortable width — normal design variety, not collapse.
    let row = rail(vec![
        card("c1", json!(160)),
        card("c2", json!("fill_container")),
    ]);
    let rects = rects(&[
        ("c1", 0.0, 0.0, 160.0, 140.0),
        ("c2", 172.0, 0.0, 140.0, 140.0),
    ]);
    let mut cmds = Vec::new();

    collect_rail_width_collapse_fixes(&row, &rects, &mut cmds);

    assert!(
        cmds.is_empty(),
        "140px is a perfectly readable card width, not a collapse: {cmds:?}"
    );
}

#[test]
fn all_fixed_width_row_is_left_untouched() {
    // Every card in the row already carries its own explicit fixed width —
    // deliberately different sizes (e.g. a wide "featured" card beside
    // smaller ones), not a fill_container squeeze.
    let row = rail(vec![
        card("c1", json!(200)),
        card("c2", json!(80)),
        card("c3", json!(80)),
    ]);
    let rects = rects(&[
        ("c1", 0.0, 0.0, 200.0, 140.0),
        ("c2", 212.0, 0.0, 80.0, 140.0),
        ("c3", 304.0, 0.0, 80.0, 140.0),
    ]);
    let mut cmds = Vec::new();

    collect_rail_width_collapse_fixes(&row, &rects, &mut cmds);

    assert!(
        cmds.is_empty(),
        "a deliberately smaller FIXED card is left alone, only fill_container is a candidate: {cmds:?}"
    );
}

#[test]
fn compact_badge_dot_does_not_become_a_card_width_reference() {
    // 0723-2-gm regression: the AQI "Good" badge is a compact horizontal
    // pill with a 6px status dot and a fill text label. The old generic row
    // matcher treated the ellipse as a fixed card and rewrote the text width
    // to 6px, while text-overflow repair changed it back on the next round.
    let row = json!({
        "type":"frame","id":"badge","name":"Good Badge","layout":"horizontal",
        "children":[
            {"type":"ellipse","id":"dot","width":6,"height":6},
            {"type":"text","id":"label","content":"Good","width":"fill_container"}
        ]
    });
    let rects = rects(&[
        ("badge", 0.0, 0.0, 52.0, 24.0),
        ("dot", 8.0, 9.0, 6.0, 6.0),
        ("label", 18.0, 4.0, 2.0, 16.0),
    ]);
    let mut cmds = Vec::new();
    let mut diagnostics = Vec::new();

    collect_rail_width_collapse_fixes(&row, &rects, &mut cmds);
    collect_rail_width_collapse_diagnostics(&row, &rects, &mut diagnostics);

    assert!(
        cmds.is_empty(),
        "leaf adornments and text are not card siblings: {cmds:?}"
    );
    assert!(
        diagnostics.is_empty(),
        "detect-only echo must share the same card-only gate: {diagnostics:?}"
    );
}

#[test]
fn compact_status_badge_is_not_a_collapsed_card_candidate() {
    // The inverse 0723-2-gm failure: a fixed 96px title wrapper could look
    // like the rail reference while the already-damaged status BADGE was a
    // 17px fill_container frame. Both are containers, but neither is a card
    // rail. The status-dot anatomy must keep the badge out of both fix and
    // detect modes.
    let row = json!({
        "type":"frame","id":"header","layout":"horizontal",
        "justifyContent":"space_between","children":[
            {
                "type":"frame","id":"title","width":96,"layout":"horizontal",
                "children":[{"type":"text","id":"title-text","content":"AIR QUALITY"}]
            },
            {
                "type":"frame","id":"badge","width":"fill_container",
                "layout":"horizontal","gap":4,"padding":[3,8],
                "fill":[{"type":"solid","color":"#C4F82A20"}],
                "children":[
                    {"type":"ellipse","id":"dot","width":6,"height":6},
                    {"type":"text","id":"label","content":"Good","width":"fill_container"}
                ]
            }
        ]
    });
    let rects = rects(&[
        ("header", 0.0, 0.0, 129.0, 22.0),
        ("title", 0.0, 0.0, 96.0, 14.0),
        ("badge", 112.0, 0.0, 17.0, 22.0),
        ("dot", 120.0, 8.0, 6.0, 6.0),
        ("label", 130.0, 4.0, 1.0, 14.0),
    ]);
    let mut cmds = Vec::new();
    let mut diagnostics = Vec::new();

    collect_rail_width_collapse_fixes(&row, &rects, &mut cmds);
    collect_rail_width_collapse_diagnostics(&row, &rects, &mut diagnostics);

    assert!(
        cmds.is_empty(),
        "compact status badge is not a card candidate: {cmds:?}"
    );
    assert!(
        diagnostics.is_empty(),
        "detect-only mode must share the status-badge exclusion: {diagnostics:?}"
    );
}

#[test]
fn starved_fill_table_cell_is_not_treated_as_a_card_rail() {
    let table = json!({
        "type":"frame","id":"table","layout":"vertical","children":[
            {
                "type":"frame","id":"row-1","layout":"horizontal","gap":8,
                "children":[
                    {"type":"frame","id":"r1-name","width":166,"children":[]},
                    {"type":"frame","id":"r1-email","width":100,"children":[]},
                    {"type":"frame","id":"r1-contact","width":"fill_container","children":[]}
                ]
            },
            {
                "type":"frame","id":"row-2","layout":"horizontal","gap":8,
                "children":[
                    {"type":"frame","id":"r2-name","width":166,"children":[]},
                    {"type":"frame","id":"r2-email","width":100,"children":[]},
                    {"type":"frame","id":"r2-contact","width":"fill_container","children":[]}
                ]
            }
        ]
    });
    let rects = rects(&[
        ("table", 0.0, 0.0, 300.0, 80.0),
        ("row-1", 0.0, 0.0, 300.0, 32.0),
        ("r1-name", 0.0, 0.0, 166.0, 32.0),
        ("r1-email", 174.0, 0.0, 100.0, 32.0),
        ("r1-contact", 282.0, 0.0, 18.0, 32.0),
        ("row-2", 0.0, 40.0, 300.0, 32.0),
        ("r2-name", 0.0, 40.0, 166.0, 32.0),
        ("r2-email", 174.0, 40.0, 100.0, 32.0),
        ("r2-contact", 282.0, 40.0, 18.0, 32.0),
    ]);
    let row = &table["children"][0];

    // Prove the row would satisfy the generic ratio/floor detector without
    // its table context.
    let mut context_free = Vec::new();
    collect_rail_width_collapse_fixes(row, &rects, &mut context_free);
    assert!(
        !context_free.is_empty(),
        "fixture must exercise the raw rail-width ratio"
    );

    let mut cmds = Vec::new();
    collect_rail_width_collapse_fixes(&table, &rects, &mut cmds);
    assert!(
        cmds.is_empty(),
        "table column scaling owns starved cells; card-rail repair must abstain: {cmds:?}"
    );

    let mut diagnostics = Vec::new();
    collect_rail_width_collapse_diagnostics_with_context(row, &rects, &mut diagnostics, true);
    assert!(
        diagnostics.is_empty(),
        "detect-only mode must share the table exclusion: {diagnostics:?}"
    );
}

/// 0718-1-k3-1 postmortem (Bug A) — exact-number regression. Measured on
/// the real file: a 342px-inner rail, one 232px fixed-width reference card,
/// two `fill_container` siblings squeezed to 41px each (232 + 2×14 gap +
/// 2×41 = 342). Confirmed via a fresh `apply_loop_finalize` run against the
/// raw file that the fix's logic correctly widens both squeezed cards to
/// 232 when it runs — the postmortem's actual root cause (see
/// `unify_shared_nav`'s D-bug sibling postmortem) was that finalize never
/// ran at all during the real generation, not a defect in this pass. This
/// locks the exact measured numbers down so a future regression in the
/// detection thresholds (ratio / floor) doesn't silently stop catching
/// this exact shape.
#[test]
fn k3_exact_numbers_232_41_41_rail_is_widened_to_match_reference() {
    let row = rail(vec![
        card("c1", json!(232)),
        card("c2", json!("fill_container")),
        card("c3", json!("fill_container")),
    ]);
    let rects = rects(&[
        ("c1", 0.0, 0.0, 232.0, 140.0),
        ("c2", 246.0, 0.0, 41.0, 140.0),
        ("c3", 301.0, 0.0, 41.0, 140.0),
    ]);
    let mut cmds = Vec::new();

    collect_rail_width_collapse_fixes(&row, &rects, &mut cmds);

    assert_eq!(
        cmds.len(),
        2,
        "both 41px-squeezed siblings must be widened: {cmds:?}"
    );
    for cmd in &cmds {
        match cmd {
            EditorCommand::UpdateNode { node_id, width, .. } => {
                assert!(
                    node_id.as_str() == "c2" || node_id.as_str() == "c3",
                    "only the collapsed siblings are touched, not the 232px reference: {node_id:?}"
                );
                assert_eq!(
                    *width,
                    Some(232),
                    "widened to the reference card's exact measured width (232px)"
                );
            }
            other => panic!("expected UpdateNode (numeric width), got {other:?}"),
        }
    }
}

#[test]
fn real_layout_widens_collapsed_savings_goals_rail_end_to_end() {
    use crate::test_support::VecDocSink;
    use crate::types::DocSink;
    use jian_ops_schema::node::PenNode;
    use op_editor_core::PenNodeExt;

    // De-identified extraction of the real user .op's "Savings Goals Rail"
    // subtree: a 375px mobile page, [0,24] section padding (327px inner
    // width), a horizontal rail of 3 cards where only the first carries a
    // fixed width. Enough child structure (icon tile + title + amount row)
    // to reproduce genuine overshoot on the squeezed cards, matching what
    // drove `collect_card_overflow_clips` to clip them in production.
    let goal_card = |id: &str, name: &str, width: serde_json::Value| {
        json!({
            "type": "frame", "id": id, "name": name, "layout": "vertical",
            "width": width, "height": "fit_content", "gap": 16, "padding": 20,
            "cornerRadius": 12, "fill": [{"type": "solid", "color": "#FFFFFF"}],
            "children": [
                {"type": "frame", "id": format!("{id}-top"), "name": "Top", "layout": "horizontal",
                 "width": "fill_container", "gap": 10, "alignItems": "center", "children": [
                    {"type": "frame", "id": format!("{id}-icon"), "name": "IconTile",
                     "width": 36, "height": 36, "children": []},
                    {"type": "text", "id": format!("{id}-title"), "name": "Title",
                     "content": name, "fontSize": 14, "width": "fill_container",
                     "textGrowth": "fixed-width", "children": []}
                 ]},
                {"type": "frame", "id": format!("{id}-amounts"), "name": "Amounts", "layout": "horizontal",
                 "width": "fill_container", "justifyContent": "space_between", "children": [
                    {"type": "text", "id": format!("{id}-saved"), "name": "Saved",
                     "content": "$3,150", "fontSize": 16, "width": "fit_content", "children": []},
                    {"type": "text", "id": format!("{id}-pct"), "name": "Pct",
                     "content": "17%", "fontSize": 12, "width": "fit_content", "children": []}
                 ]}
            ]
        })
    };

    let root: PenNode = serde_json::from_value(json!({
        "type": "frame", "id": "page", "name": "Finance Overview", "width": 375,
        "height": "fit_content", "layout": "vertical", "children": [
            {"type": "frame", "id": "section", "name": "Savings Goals Rail", "layout": "vertical",
             "width": "fill_container", "height": "fit_content", "gap": 12, "padding": [0, 24],
             "children": [
                {"type": "frame", "id": "rail", "name": "Rail", "layout": "horizontal",
                 "width": "fill_container", "gap": 12, "children": [
                    goal_card("c1", "Emergency Fund", json!(200)),
                    goal_card("c2", "New Car", json!("fill_container")),
                    goal_card("c3", "Vacation", json!("fill_container"))
                 ]}
             ]}
        ]
    }))
    .expect("valid root");

    let mut sink = VecDocSink::new();
    sink.apply(EditorCommand::InsertSubtree {
        nodes: vec![root],
        parent_id: NodeId::NONE,
        page_id: None,
    });
    let root_id = sink.state().active_children()[0].id_str().to_string();

    let before_rects = resolved_rects(sink.state());
    let before = resolved_widths_by_name(
        sink.state(),
        &before_rects,
        &["Emergency Fund", "New Car", "Vacation"],
    );
    assert!(
        before[0] - before[1] > 100.0,
        "fixture must reproduce the real collapse before repair, got {before:?}"
    );

    assert!(
        fix_rail_width_collapse(&mut sink, &root_id),
        "the collapsed siblings must be widened"
    );

    let v = serde_json::to_value(sink.state().active_children()[0].clone()).unwrap();
    for name in ["New Car", "Vacation"] {
        let node = find_by_name(&v, name).unwrap_or_else(|| panic!("{name} exists"));
        assert_eq!(
            node.get("width").and_then(|w| w.as_f64()),
            Some(200.0),
            "{name} must now declare the reference card's fixed width"
        );
    }

    let after_rects = resolved_rects(sink.state());
    let after = resolved_widths_by_name(
        sink.state(),
        &after_rects,
        &["Emergency Fund", "New Car", "Vacation"],
    );
    let max = after.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let min = after.iter().copied().fold(f64::INFINITY, f64::min);
    assert!(
        max - min <= 1.0,
        "all three cards resolve to the same width after repair, got {after:?}"
    );
}
