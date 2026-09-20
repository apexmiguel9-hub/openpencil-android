//! Collapse-detector and card-row equalization tests.

use super::*;

#[test]
fn collapsed_fill_container_is_demoted() {
    // A card declaring fill_container height that RESOLVED to ~0 while its 44px
    // value text still has real height → collapse → hug + top-pack.
    let card = json!({
        "type":"frame","id":"card","name":"Card","layout":"vertical","height":"fill_container",
        "justifyContent":"space_between","children":[
            {"type":"text","id":"v","content":"1,248"},{"type":"text","id":"l","content":"TOTAL"}
        ]
    });
    let mut rects = std::collections::HashMap::new();
    rects.insert(
        "card".to_string(),
        Rect {
            x: 0.0,
            y: 0.0,
            w: 200.0,
            h: 0.0,
        },
    ); // collapsed
    rects.insert(
        "v".to_string(),
        Rect {
            x: 0.0,
            y: 0.0,
            w: 200.0,
            h: 44.0,
        },
    ); // child HAS height
    rects.insert(
        "l".to_string(),
        Rect {
            x: 0.0,
            y: 0.0,
            w: 200.0,
            h: 14.0,
        },
    );
    let mut cmds = Vec::new();
    collect_collapse_fixes(&card, &rects, &mut cmds);
    assert!(
        kw_op(&cmds, "height", "fit_content"),
        "height demoted to hug"
    );
    assert!(
        kw_op(&cmds, "justifyContent", "start"),
        "distribution neutralized"
    );
}

#[test]
fn fit_content_zero_height_is_not_a_collapse() {
    // A fit_content container at 0 resolved height is intentionally empty — only
    // a fill_container that collapsed is broken.
    let c = json!({"type":"frame","id":"c","name":"C","layout":"vertical","height":"fit_content","children":[{"type":"text","id":"t","content":"x"}]});
    let mut rects = std::collections::HashMap::new();
    rects.insert(
        "c".to_string(),
        Rect {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 0.0,
        },
    );
    rects.insert(
        "t".to_string(),
        Rect {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 20.0,
        },
    );
    let mut cmds = Vec::new();
    collect_collapse_fixes(&c, &rects, &mut cmds);
    assert!(cmds.is_empty());
}

#[test]
fn healthy_fill_container_is_not_flagged() {
    // fill_container that resolved to a real height (it filled its ancestor) → ok.
    let c = json!({"type":"frame","id":"c","name":"C","layout":"vertical","height":"fill_container","children":[{"type":"text","id":"t","content":"x"}]});
    let mut rects = std::collections::HashMap::new();
    rects.insert(
        "c".to_string(),
        Rect {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 300.0,
        },
    );
    rects.insert(
        "t".to_string(),
        Rect {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 20.0,
        },
    );
    let mut cmds = Vec::new();
    collect_collapse_fixes(&c, &rects, &mut cmds);
    assert!(cmds.is_empty());
}

#[test]
fn loop_entry_fixes_overflowing_table() {
    use crate::test_support::VecDocSink;
    use crate::types::DocSink;
    use jian_ops_schema::node::PenNode;
    use op_editor_core::PenNodeExt;

    let mkcells = |p: &str| -> Vec<serde_json::Value> {
        (0..5)
            .map(|i| json!({"type":"frame","id":format!("{p}-c{i}"),"name":"Cell","width":240,"height":20,"children":[]}))
            .collect()
    };
    let mkrow = |id: &str| json!({"type":"frame","id":id,"name":"Row","layout":"horizontal","gap":16,"width":"fill_container","height":24,"children":mkcells(id)});
    let root: PenNode = serde_json::from_value(json!({
        "type":"frame","id":"root","name":"Root","width":800,"height":"fit_content","layout":"vertical","children":[
            {"type":"frame","id":"tbl","name":"Client Table","layout":"vertical","width":"fill_container","children":[
                mkrow("hdr"), mkrow("r1"), mkrow("r2")
            ]}
        ]
    })).expect("valid root");

    let mut sink = VecDocSink::new();
    sink.apply(EditorCommand::InsertSubtree {
        nodes: vec![root],
        parent_id: NodeId::NONE,
        page_id: None,
    });
    let root_id = sink.state().active_children()[0].id_str().to_string();

    let rounds = geometry_validate_and_fix(&mut sink, &root_id);
    assert!(rounds >= 1, "the loop applied at least one fix round");
}

/// ENGINE-CONTRACT sentinel: a `fill_container`-height child of a HUGGING parent
/// must resolve to a real size, not collapse — vertical main axis via grow,
/// horizontal cross axis via stretch (to the tallest sibling). The retirement of
/// the tree-shape `fix_circular_fill_height` demoter rests on this contract; if
/// jian regresses it, this fires long before a corpus render does.
#[test]
fn real_layout_fill_of_hug_parent_resolves_to_content_not_collapse() {
    use crate::test_support::VecDocSink;
    use crate::types::DocSink;
    use jian_ops_schema::node::PenNode;

    // Shape A: vertical hug parent + fill-height child (space_between, real content)
    // Shape B: horizontal hug row + fill-height KPI cards
    let root: PenNode = serde_json::from_value(json!({
        "type":"frame","id":"root","name":"Root","width":800,"height":"fit_content","layout":"vertical","gap":24,"children":[
            {"type":"frame","id":"vparent","name":"VParent","layout":"vertical","width":"fill_container","height":"fit_content","children":[
                {"type":"frame","id":"vchild","name":"VChild","layout":"vertical","width":"fill_container","height":"fill_container","justifyContent":"space_between","children":[
                    {"type":"text","id":"t1","name":"T1","content":"Value 42","fontSize":28},
                    {"type":"text","id":"t2","name":"T2","content":"Label","fontSize":13}
                ]}
            ]},
            {"type":"frame","id":"hrow","name":"HRow","layout":"horizontal","gap":16,"width":"fill_container","height":"fit_content","children":[
                {"type":"frame","id":"card1","name":"Card1","layout":"vertical","width":"fill_container","height":"fill_container","justifyContent":"space_between","children":[
                    {"type":"text","id":"c1a","name":"C1A","content":"98.7%","fontSize":28},
                    {"type":"text","id":"c1b","name":"C1B","content":"Uptime","fontSize":13}
                ]},
                {"type":"frame","id":"card2","name":"Card2","layout":"vertical","width":"fill_container","height":120,"children":[
                    {"type":"text","id":"c2a","name":"C2A","content":"1,284","fontSize":28}
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
    let rects = resolved_rects(sink.state());
    let v = serde_json::to_value(sink.state().active_children()[0].clone()).unwrap();
    fn rect_of<'a>(
        v: &serde_json::Value,
        rects: &'a HashMap<String, Rect>,
        name: &str,
    ) -> Option<&'a Rect> {
        if v.get("name").and_then(|x| x.as_str()) == Some(name) {
            return v
                .get("id")
                .and_then(|x| x.as_str())
                .and_then(|id| rects.get(id));
        }
        v.get("children")
            .and_then(|c| c.as_array())
            .into_iter()
            .flatten()
            .find_map(|c| rect_of(c, rects, name))
    }
    // Shape A: the fill child of a hugging vertical parent hugs its two text
    // lines (28px + 13px) instead of collapsing to ~0.
    let vchild = rect_of(&v, &rects, "VChild").expect("VChild resolved");
    assert!(
        vchild.h >= 40.0,
        "fill-of-hug vertical child must hug content, got h={}",
        vchild.h
    );
    // Shape B: the fill card cross-axis-stretches to its 120px numeric sibling.
    let card1 = rect_of(&v, &rects, "Card1").expect("Card1 resolved");
    assert!(
        (card1.h - 120.0).abs() < 1.0,
        "fill card must stretch to the tallest sibling, got h={}",
        card1.h
    );
    // Its stacked children must not overlap (the old percent-mapping collapse).
    let c1a = rect_of(&v, &rects, "C1A").expect("C1A resolved");
    let c1b = rect_of(&v, &rects, "C1B").expect("C1B resolved");
    assert!(
        c1a.h > 0.0 && c1b.h > 0.0,
        "card children carry real height"
    );
}

#[test]
fn real_layout_equalizes_luxe_cut_metric_card_row_heights() {
    use crate::test_support::VecDocSink;
    use crate::types::DocSink;
    use jian_ops_schema::node::PenNode;
    use op_editor_core::PenNodeExt;

    let stroke = || json!({"thickness": 1, "fill": [{"type": "solid", "color": "#E5E7EB"}]});
    let card = |id: &str, name: &str, title: &str| {
        json!({
            "type":"frame","id":id,"name":name,"layout":"vertical","gap":8,
            "width":"fill_container","height":"fit_content","padding":[16,16],
            "stroke": stroke(),
            "children":[
                {"type":"text","id":format!("{id}-title"),"name":format!("{name} Title"),
                 "content":title,"fontSize":15,"width":"fill_container","textGrowth":"fixed-width"},
                {"type":"text","id":format!("{id}-value"),"name":format!("{name} Value"),
                 "content":"$48,920","fontSize":28},
                {"type":"text","id":format!("{id}-label"),"name":format!("{name} Label"),
                 "content":"vs last month","fontSize":12}
            ]
        })
    };
    let root: PenNode = serde_json::from_value(json!({
        "type":"frame","id":"root","name":"LUXE CUT Dashboard","width":960,"height":"fit_content","layout":"vertical","gap":24,"children":[
            {"type":"frame","id":"metrics","name":"Key Metrics","layout":"horizontal","gap":16,
             "width":"fill_container","height":"fit_content","alignItems":"stretch","children":[
                card("card1", "Metric Card 1", "Revenue"),
                card("card2", "Metric Card 2", "Average revenue per client visit this month"),
                card("card3", "Metric Card 3", "Bookings"),
                card("card4", "Metric Card 4", "Retention")
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
    let before = resolved_heights_by_name(
        sink.state(),
        &before_rects,
        &[
            "Metric Card 1",
            "Metric Card 2",
            "Metric Card 3",
            "Metric Card 4",
        ],
    );
    let before_min = before.iter().copied().fold(f64::INFINITY, f64::min);
    let before_max = before.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    assert!(
        before_max - before_min > 6.0,
        "fixture must be ragged before repair, got {before:?}"
    );

    let rounds = geometry_validate_and_fix(&mut sink, &root_id);
    assert!(rounds >= 1, "ragged card row must trigger a fix round");

    let after_rects = resolved_rects(sink.state());
    let after = resolved_heights_by_name(
        sink.state(),
        &after_rects,
        &[
            "Metric Card 1",
            "Metric Card 2",
            "Metric Card 3",
            "Metric Card 4",
        ],
    );
    let after_min = after.iter().copied().fold(f64::INFINITY, f64::min);
    let after_max = after.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    assert!(
        after_max - after_min <= 1.0,
        "cards must resolve to equal heights after repair, got {after:?}"
    );
}

#[test]
fn ragged_card_row_without_explicit_stretch_keeps_hug_height() {
    let row = json!({
        "type":"frame","id":"row","name":"Cards","layout":"horizontal","children":[
            stroked_card_json("c1", json!("fit_content")),
            stroked_card_json("c2", json!("fit_content")),
            stroked_card_json("c3", json!("fit_content"))
        ]
    });
    let rects = card_row_rects([140.0, 180.0, 142.0]);
    let mut cmds = Vec::new();

    collect_card_row_height_fixes(&row, &rects, &mut cmds, false);

    assert!(
        cmds.is_empty(),
        "Hug is the default without explicit stretch: {cmds:?}"
    );
}

#[test]
fn card_row_with_authored_numeric_child_height_is_left_untouched() {
    let row = json!({
        "type":"frame","id":"row","name":"Cards","layout":"horizontal","children":[
            stroked_card_json("c1", json!("fit_content")),
            stroked_card_json("c2", json!(180)),
            stroked_card_json("c3", json!("fit_content"))
        ]
    });
    let rects = card_row_rects([140.0, 180.0, 142.0]);
    let mut cmds = Vec::new();

    collect_card_row_height_fixes(&row, &rects, &mut cmds, false);

    assert!(
        cmds.is_empty(),
        "numeric child height is deliberate: {cmds:?}"
    );
}

#[test]
fn transparent_wrapper_row_is_not_equalized_as_cards() {
    let row = json!({
        "type":"frame","id":"row","name":"Wrappers","layout":"horizontal","children":[
            transparent_card_json("c1"),
            transparent_card_json("c2"),
            transparent_card_json("c3")
        ]
    });
    let rects = card_row_rects([140.0, 180.0, 142.0]);
    let mut cmds = Vec::new();

    collect_card_row_height_fixes(&row, &rects, &mut cmds, false);

    assert!(
        cmds.is_empty(),
        "transparent wrappers are not colored cards: {cmds:?}"
    );
}

#[test]
fn card_rows_inside_table_context_are_left_untouched() {
    let table = json!({
        "type":"frame","id":"table","name":"Table","layout":"vertical","children":[
            {"type":"frame","id":"row1","layout":"horizontal","children":[
                stroked_card_json("r1c1", json!("fit_content")),
                stroked_card_json("r1c2", json!("fit_content")),
                stroked_card_json("r1c3", json!("fit_content"))
            ]},
            {"type":"frame","id":"row2","layout":"horizontal","children":[
                stroked_card_json("r2c1", json!("fit_content")),
                stroked_card_json("r2c2", json!("fit_content")),
                stroked_card_json("r2c3", json!("fit_content"))
            ]}
        ]
    });
    let mut rects = std::collections::HashMap::new();
    for (id, h) in [
        ("row1", 180.0),
        ("row2", 180.0),
        ("r1c1", 140.0),
        ("r1c2", 180.0),
        ("r1c3", 142.0),
        ("r2c1", 140.0),
        ("r2c2", 180.0),
        ("r2c3", 142.0),
    ] {
        rects.insert(
            id.to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h,
            },
        );
    }
    let mut cmds = Vec::new();

    collect_card_row_height_fixes(&table, &rects, &mut cmds, false);

    assert!(
        cmds.is_empty(),
        "table rows belong to table repair: {cmds:?}"
    );
}
