//! `collect_starved_rigid_row_fixes` — the footer-overlap repair.
//!
//! The fixture is `0808-gm-2.op`'s footer verbatim, reduced to the three
//! columns that produce the failure.

use super::*;
use serde_json::json;

fn rect(x: f64, w: f64) -> Rect {
    Rect {
        x,
        y: 0.0,
        w,
        h: 100.0,
    }
}

/// `0808-gm-2.op` footer: a 1040px row holding a fixed 280px brand column, a
/// `fill_container` nav block of four 96px columns (gap 48 → needs 528), and a
/// `fill_container` newsletter block. The two flex siblings split the 680px of
/// free space evenly, so the nav block resolved 340 and its last two columns
/// spilled across the newsletter.
fn footer_row() -> (Value, HashMap<String, Rect>) {
    let col = |id: &str| {
        json!({"type":"frame","id":id,"layout":"vertical","width":96,
                                "children":[{"type":"text","id":format!("{id}t"),"content":"条款与隐私"}]})
    };
    let row = json!({
        "type":"frame","id":"content","layout":"horizontal","width":"fill_container","gap":40,
        "children":[
            {"type":"frame","id":"brand","layout":"vertical","width":280,"children":[]},
            {"type":"frame","id":"nav","layout":"horizontal","width":"fill_container","gap":48,
             "children":[col("c1"), col("c2"), col("c3"), col("c4")]},
            {"type":"frame","id":"news","layout":"vertical","width":"fill_container","children":[]}
        ]
    });
    let rects = HashMap::from([
        ("content".to_string(), rect(160.0, 1040.0)),
        ("brand".to_string(), rect(160.0, 280.0)),
        ("nav".to_string(), rect(480.0, 340.0)),
        ("c1".to_string(), rect(480.0, 96.0)),
        ("c2".to_string(), rect(624.0, 96.0)),
        ("c3".to_string(), rect(768.0, 96.0)),
        ("c4".to_string(), rect(912.0, 96.0)),
        ("news".to_string(), rect(860.0, 340.0)),
    ]);
    (row, rects)
}

#[test]
fn a_starved_rigid_row_is_demoted_to_fit_content() {
    let (row, rects) = footer_row();
    let mut cmds = Vec::new();
    collect_starved_rigid_row_fixes(&row, &rects, &mut cmds);
    assert_eq!(cmds.len(), 1, "one fix, on the row itself: {cmds:?}");
    match &cmds[0] {
        EditorCommand::SetNodeLayoutProp {
            node_id,
            property,
            value,
        } => {
            assert_eq!(node_id.as_str(), "nav");
            assert_eq!(property, "width");
            assert_eq!(value, &LayoutPropValue::Keyword("fit_content".to_string()));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn the_columns_themselves_are_left_alone() {
    // The repair must not touch the columns: they are not too wide, the row
    // was too narrow. Squeezing them is what would have crushed the labels.
    let (row, rects) = footer_row();
    let mut cmds = Vec::new();
    collect_starved_rigid_row_fixes(&row, &rects, &mut cmds);
    for cmd in &cmds {
        if let EditorCommand::SetNodeLayoutProp { node_id, .. } = cmd {
            assert!(
                !node_id.as_str().starts_with('c'),
                "a column was retargeted: {cmd:?}"
            );
        }
    }
}

#[test]
fn a_row_that_already_fits_is_untouched() {
    let (row, mut rects) = footer_row();
    // Same tree, but the solver seated the nav block at its full 528.
    rects.insert("nav".to_string(), rect(480.0, 528.0));
    let mut cmds = Vec::new();
    collect_starved_rigid_row_fixes(&row, &rects, &mut cmds);
    assert!(cmds.is_empty(), "{cmds:?}");
}

#[test]
fn a_row_with_a_flexible_column_is_left_to_the_overfull_repair() {
    // One column can absorb the deficit on its own, so the row is NOT rigid
    // and the existing inside-out repair owns the case.
    let mut row = footer_row().0;
    row["children"][1]["children"][3]["width"] = json!("fill_container");
    let rects = footer_row().1;
    let mut cmds = Vec::new();
    collect_starved_rigid_row_fixes(&row, &rects, &mut cmds);
    assert!(cmds.is_empty(), "{cmds:?}");
}

#[test]
fn a_declared_scroller_may_stay_narrow() {
    // `clipContent` is an authored "this row scrolls" contract; narrowing it
    // below its content is the point, not a defect.
    let mut row = footer_row().0;
    row["children"][1]["clipContent"] = json!(true);
    let rects = footer_row().1;
    let mut cmds = Vec::new();
    collect_starved_rigid_row_fixes(&row, &rects, &mut cmds);
    assert!(cmds.is_empty(), "{cmds:?}");
}

#[test]
fn a_row_too_wide_for_the_whole_budget_is_left_to_the_model() {
    // Six 200px columns need 1440 — more than the 680 the flexible siblings
    // hold between them. No width this pass can pick makes that fit; the
    // design needs fewer columns, which is an intent call.
    let col =
        |id: &str| json!({"type":"frame","id":id,"layout":"vertical","width":200,"children":[]});
    let row = json!({
        "type":"frame","id":"content","layout":"horizontal","width":"fill_container","gap":40,
        "children":[
            {"type":"frame","id":"brand","layout":"vertical","width":280,"children":[]},
            {"type":"frame","id":"nav","layout":"horizontal","width":"fill_container","gap":48,
             "children":[col("c1"), col("c2"), col("c3"), col("c4"), col("c5"), col("c6")]},
            {"type":"frame","id":"news","layout":"vertical","width":"fill_container","children":[]}
        ]
    });
    let mut rects = HashMap::from([
        ("content".to_string(), rect(160.0, 1040.0)),
        ("brand".to_string(), rect(160.0, 280.0)),
        ("nav".to_string(), rect(480.0, 340.0)),
        ("news".to_string(), rect(860.0, 340.0)),
    ]);
    for (i, id) in ["c1", "c2", "c3", "c4", "c5", "c6"].iter().enumerate() {
        rects.insert(id.to_string(), rect(480.0 + i as f64 * 248.0, 200.0));
    }
    let mut cmds = Vec::new();
    collect_starved_rigid_row_fixes(&row, &rects, &mut cmds);
    assert!(cmds.is_empty(), "{cmds:?}");
}

#[test]
fn a_lone_flexible_row_has_nobody_to_take_space_from() {
    // Only ONE flexible child: demoting it to fit_content would push the
    // overflow up a level instead of resolving it.
    let col =
        |id: &str| json!({"type":"frame","id":id,"layout":"vertical","width":96,"children":[]});
    let row = json!({
        "type":"frame","id":"content","layout":"horizontal","width":"fill_container","gap":40,
        "children":[
            {"type":"frame","id":"brand","layout":"vertical","width":280,"children":[]},
            {"type":"frame","id":"nav","layout":"horizontal","width":"fill_container","gap":48,
             "children":[col("c1"), col("c2"), col("c3"), col("c4")]}
        ]
    });
    let rects = HashMap::from([
        ("content".to_string(), rect(160.0, 1040.0)),
        ("brand".to_string(), rect(160.0, 280.0)),
        ("nav".to_string(), rect(480.0, 340.0)),
        ("c1".to_string(), rect(480.0, 96.0)),
        ("c2".to_string(), rect(624.0, 96.0)),
        ("c3".to_string(), rect(768.0, 96.0)),
        ("c4".to_string(), rect(912.0, 96.0)),
    ]);
    let mut cmds = Vec::new();
    collect_starved_rigid_row_fixes(&row, &rects, &mut cmds);
    assert!(cmds.is_empty(), "{cmds:?}");
}
