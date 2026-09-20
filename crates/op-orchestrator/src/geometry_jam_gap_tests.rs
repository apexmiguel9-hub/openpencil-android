//! Page-level jam exemptions, gap fixes, rigid-child shrink, text wrap and
//! the over-full row flexify cases.

use super::*;

#[test]
fn page_level_columns_touching_are_not_a_jam() {
    // An app-shell's [Sidebar | Main] columns legitimately touch — tall
    // page-level columns must never be reported as jammed text cells.
    let row = json!({
        "type":"frame","id":"root","name":"Page","layout":"horizontal","children":[
            {"type":"frame","id":"sb","name":"Sidebar","children":[{"type":"text","id":"a","content":"Nav"}]},
            {"type":"frame","id":"mc","name":"Main","children":[{"type":"text","id":"b","content":"Body"}]}
        ]
    });
    let mut rects = std::collections::HashMap::new();
    rects.insert(
        "root".into(),
        Rect {
            x: 0.0,
            y: 0.0,
            w: 1200.0,
            h: 900.0,
        },
    );
    rects.insert(
        "sb".into(),
        Rect {
            x: 0.0,
            y: 0.0,
            w: 260.0,
            h: 900.0,
        },
    );
    rects.insert(
        "mc".into(),
        Rect {
            x: 260.0,
            y: 0.0,
            w: 940.0,
            h: 900.0,
        },
    );
    let mut out = Vec::new();
    collect_sibling_jam_diagnostics(&row, &rects, &mut out);
    assert!(out.is_empty(), "page columns are not a jam: {out:?}");
}

#[test]
fn real_layout_gap_fix_reaches_doubly_wrapped_jammed_rows() {
    use crate::test_support::VecDocSink;
    use crate::types::DocSink;
    use jian_ops_schema::node::PenNode;
    use op_editor_core::PenNodeExt;

    // p01's verbatim shape: table-named frame > unnamed vertical > unnamed
    // vertical > gap-less 4-cell text rows. The NAME gate never sees the rows;
    // the geometry gap fixer must prove the jam from resolved rects and inject
    // a gap regardless of nesting.
    let mkrow = |id: &str| {
        json!({"type":"frame","id":id,"name":null,"layout":"horizontal","width":"fill_container","height":48,"children":[
            {"type":"frame","id":format!("{id}a"),"width":200,"height":40,"children":[{"type":"text","id":format!("{id}at"),"content":"James Wilson","fontSize":14}]},
            {"type":"frame","id":format!("{id}b"),"width":130,"height":40,"children":[{"type":"text","id":format!("{id}bt"),"content":"Oct 24, 2024","fontSize":13}]},
            {"type":"frame","id":format!("{id}c"),"width":110,"height":40,"children":[{"type":"text","id":format!("{id}ct"),"content":"42","fontSize":13}]},
            {"type":"frame","id":format!("{id}d"),"width":100,"height":40,"children":[{"type":"text","id":format!("{id}dt"),"content":"VIP","fontSize":12}]}
        ]})
    };
    let root: PenNode = serde_json::from_value(json!({
        "type":"frame","id":"root","name":"Client Directory Data Table","width":800,"height":"fit_content","layout":"vertical","children":[
            {"type":"frame","id":"w1","layout":"vertical","width":"fill_container","height":"fit_content","children":[
                {"type":"frame","id":"w2","layout":"vertical","width":"fill_container","height":"fit_content","children":[
                    mkrow("r1"), mkrow("r2"), mkrow("r3")
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
    assert!(geometry_validate_and_fix(&mut sink, &root_id) >= 1);
    let v = serde_json::to_value(sink.state().active_children()[0].clone()).unwrap();
    fn rows_with_gap(v: &serde_json::Value, n: &mut usize) {
        if v.get("layout").and_then(|l| l.as_str()) == Some("horizontal")
            && v.get("gap").and_then(|g| g.as_f64()).unwrap_or(0.0) > 0.0
        {
            *n += 1;
        }
        for c in v
            .get("children")
            .and_then(|c| c.as_array())
            .into_iter()
            .flatten()
        {
            rows_with_gap(c, n);
        }
    }
    let mut n = 0;
    rows_with_gap(&v, &mut n);
    assert!(n >= 3, "all three buried rows got a gap, found {n}");
}

#[test]
fn real_layout_shrinks_rigid_fit_child_overflowing_a_narrow_card() {
    use crate::test_support::VecDocSink;
    use crate::types::DocSink;
    use jian_ops_schema::node::PenNode;
    use op_editor_core::PenNodeExt;

    // p02's verbatim shape: an 80px card whose fit_content icon+text pair is
    // rigid at max-content (~150px) and paints over siblings. The fixer must
    // retarget it to fill_container (shrinkable); the text inside then wraps
    // via the text-overflow fixer on the next loop round.
    let root: PenNode = serde_json::from_value(json!({
        "type":"frame","id":"root","name":"Hero Card","width":80,"height":"fit_content","layout":"vertical","children":[
            {"type":"frame","id":"row","layout":"horizontal","gap":8,"width":"fill_container","height":"fit_content","children":[
                {"type":"frame","id":"pair","layout":"horizontal","gap":6,"width":"fit_content","height":"fit_content","children":[
                    {"type":"icon_font","id":"ic","iconFontName":"coffee","width":14,"height":14},
                    {"type":"text","id":"t","content":"Ethiopian Yirgacheffe pour-over","fontSize":13}
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
    geometry_validate_and_fix(&mut sink, &root_id);
    let v = serde_json::to_value(sink.state().active_children()[0].clone()).unwrap();
    fn find<'a>(v: &'a serde_json::Value, name: &str) -> Option<&'a serde_json::Value> {
        if v.get("name").and_then(|x| x.as_str()) == Some(name) {
            return Some(v);
        }
        v.get("children")
            .and_then(|c| c.as_array())
            .into_iter()
            .flatten()
            .find_map(|c| find(c, name))
    }
    // The pair was renamed by id remap; find the frame that HOLDS the icon.
    fn find_pair(v: &serde_json::Value) -> Option<&serde_json::Value> {
        let kids = v.get("children").and_then(|c| c.as_array())?;
        if kids
            .iter()
            .any(|c| c.get("type").and_then(|t| t.as_str()) == Some("icon_font"))
        {
            return Some(v);
        }
        kids.iter().find_map(find_pair)
    }
    let _ = find; // silence potential unused in future edits
    let pair = find_pair(&v).expect("icon+text pair survives");
    assert_eq!(
        pair.get("width").and_then(|w| w.as_str()),
        Some("fill_container"),
        "rigid fit pair retargeted to fill, got {:?}",
        pair.get("width")
    );
}

#[test]
fn real_layout_wraps_text_pushed_past_the_row_edge_by_a_sibling() {
    use crate::test_support::VecDocSink;
    use crate::types::DocSink;
    use jian_ops_schema::node::PenNode;
    use op_editor_core::PenNodeExt;

    // p44's verbatim shape: a 116px centered row holding [36px ellipse, fit
    // text] — the text alone fits the row, but the PAIR overflows and the
    // text's right edge lands past the row edge. The width-only check missed
    // this; the right-edge check must wrap the text.
    let root: PenNode = serde_json::from_value(json!({
        "type":"frame","id":"root","name":"Step Card","width":400,"height":"fit_content","layout":"vertical","children":[
            {"type":"frame","id":"row","name":"Avatar Row","layout":"horizontal","width":116,"height":"fit_content","justifyContent":"center","alignItems":"center","children":[
                {"type":"ellipse","id":"av","width":36,"height":36},
                {"type":"text","id":"nm","name":"Name","content":"Personalize your workspace","fontSize":14}
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
    geometry_validate_and_fix(&mut sink, &root_id);
    let v = serde_json::to_value(sink.state().active_children()[0].clone()).unwrap();
    fn find<'a>(v: &'a serde_json::Value, name: &str) -> Option<&'a serde_json::Value> {
        if v.get("name").and_then(|x| x.as_str()) == Some(name) {
            return Some(v);
        }
        v.get("children")
            .and_then(|c| c.as_array())
            .into_iter()
            .flatten()
            .find_map(|c| find(c, name))
    }
    let nm = find(&v, "Name").expect("text survives");
    assert_eq!(
        nm.get("width").and_then(|w| w.as_str()),
        Some("fill_container"),
        "pair-overflowed text wrapped, got {:?}",
        nm.get("width")
    );
}

#[test]
fn ring_badge_overlay_is_not_reported_as_an_overlap() {
    // A step-ring: ellipse + a short number stacked ON it (center inside) —
    // an intentional overlay, not an overflow accident.
    let row = json!({
        "type":"frame","id":"row","name":"Ring","layout":"horizontal","children":[
            {"type":"ellipse","id":"e","width":36,"height":36},
            {"type":"text","id":"t","content":"2","fontSize":15}
        ]
    });
    let mut rects = std::collections::HashMap::new();
    rects.insert(
        "row".into(),
        Rect {
            x: 0.0,
            y: 0.0,
            w: 116.0,
            h: 36.0,
        },
    );
    rects.insert(
        "e".into(),
        Rect {
            x: 40.0,
            y: 0.0,
            w: 36.0,
            h: 36.0,
        },
    );
    rects.insert(
        "t".into(),
        Rect {
            x: 53.0,
            y: 9.0,
            w: 10.0,
            h: 18.0,
        },
    ); // centered on the ring
    let mut out = Vec::new();
    collect_sibling_jam_diagnostics(&row, &rects, &mut out);
    assert!(out.is_empty(), "overlay must not be reported: {out:?}");
}

#[test]
fn real_layout_overfull_top_bar_flexifies_until_it_fits() {
    use crate::test_support::VecDocSink;
    use crate::types::DocSink;
    use jian_ops_schema::node::PenNode;
    use op_editor_core::PenNodeExt;

    // test0703.op (MAISON) verbatim shape: a space_between top bar whose fit
    // title block + fit actions cluster (280px search + date + button) sum
    // wider than the row — the title ran into the search box and the button
    // clipped at the page edge. No single child is wider than the row, so
    // the per-child fixers are blind; the overfull fixer must flexify the
    // widest rigid child (the cluster, then its search) until everything's
    // right edge is back inside the row.
    let doc = r##"{
        "type":"frame","id":"root","name":"Page","width":700,"height":"fit_content","layout":"vertical","children":[
            {"type":"frame","id":"bar","name":"Top Bar","layout":"horizontal","width":"fill_container","height":"fit_content",
             "justifyContent":"space_between","alignItems":"center","children":[
                {"type":"frame","id":"title","name":"Title Block","layout":"horizontal","gap":12,"width":"fit_content","height":"fit_content","children":[
                    {"type":"text","id":"t1","content":"MANAGEMENT","fontSize":12},
                    {"type":"text","id":"t2","content":"Client Management Suite","fontSize":34}
                ]},
                {"type":"frame","id":"cluster","name":"Right Cluster","layout":"horizontal","gap":24,"width":"fit_content","height":"fit_content","alignItems":"center","children":[
                    {"type":"frame","id":"search","name":"Global Search","layout":"horizontal","gap":8,"width":280,"height":40,"children":[
                        {"type":"text","id":"ph","content":"Search clients...","fontSize":13}
                    ]},
                    {"type":"text","id":"date","content":"Wed, Oct 25","fontSize":13},
                    {"type":"frame","id":"cta","name":"Add Client","layout":"horizontal","width":120,"height":40,"children":[]}
                ]}
            ]}
        ]
    }"##;
    let root: PenNode = serde_json::from_str(doc).expect("valid root");
    let mut sink = VecDocSink::new();
    sink.apply(EditorCommand::InsertSubtree {
        nodes: vec![root],
        parent_id: NodeId::NONE,
        page_id: None,
    });
    let root_id = sink.state().active_children()[0].id_str().to_string();
    let rounds = geometry_validate_and_fix(&mut sink, &root_id);
    assert!(rounds >= 1, "overfull bar must trigger at least one round");

    // Geometry proof: every descendant's right edge sits inside the bar's.
    let rects = resolved_rects(sink.state());
    let v = serde_json::to_value(sink.state().active_children()[0].clone()).unwrap();
    let bar = v["children"][0]["id"].as_str().unwrap();
    let bar_right = {
        let r = &rects[bar];
        r.x + r.w
    };
    fn walk(v: &serde_json::Value, out: &mut Vec<String>) {
        if let Some(id) = v.get("id").and_then(|x| x.as_str()) {
            out.push(id.to_string());
        }
        for c in v
            .get("children")
            .and_then(|c| c.as_array())
            .into_iter()
            .flatten()
        {
            walk(c, out);
        }
    }
    let mut ids = Vec::new();
    walk(&v["children"][0], &mut ids);
    for id in ids {
        if let Some(r) = rects.get(&id) {
            assert!(
                r.x + r.w <= bar_right + 2.0,
                "{id} still hangs past the bar: right={} bar_right={bar_right}",
                r.x + r.w
            );
        }
    }
}

#[test]
fn real_layout_wraps_a_stack_pushed_past_its_cell_by_an_avatar() {
    use crate::test_support::VecDocSink;
    use crate::types::DocSink;
    use jian_ops_schema::node::PenNode;
    use op_editor_core::PenNodeExt;

    // ATELIER's verbatim shape: a 120px name cell holding [36px avatar, fit
    // name stack]. The stack alone (93px) fits the cell, but avatar + gap
    // push its tail 21px into the NEXT column — the width-only check
    // acquitted it. The right-edge check must flexify the stack; the text
    // inside then wraps on the following round.
    let doc = r##"{
        "type":"frame","id":"page","name":"Page","width":400,"height":300,"layout":"vertical","children":[
            {"type":"frame","id":"row","name":"Row","width":"fill_container","height":"fit_content","layout":"horizontal","gap":24,"children":[
                {"type":"frame","id":"cell","name":"Cell Client","width":120,"height":"fit_content","layout":"horizontal","gap":12,"alignItems":"center","children":[
                    {"type":"frame","id":"av","name":"Avatar","width":36,"height":36,"children":[]},
                    {"type":"frame","id":"stack","name":"Name Stack","width":"fit_content","height":"fit_content","layout":"vertical","gap":2,"children":[
                        {"type":"text","id":"nm","content":"Maximilian Thornebury-Ashworth","fontSize":14,"fontWeight":600},
                        {"type":"text","id":"tier","content":"VIP Member","fontSize":11}
                    ]}
                ]},
                {"type":"frame","id":"contact","name":"Cell Contact","width":"fill_container","height":"fit_content","layout":"vertical","children":[
                    {"type":"text","id":"em","content":"j.thorne@mail.com","fontSize":13}
                ]}
            ]}
        ]
    }"##;
    let root: PenNode = serde_json::from_str(doc).expect("valid root");
    let mut sink = VecDocSink::new();
    sink.apply(EditorCommand::InsertSubtree {
        nodes: vec![root],
        parent_id: NodeId::NONE,
        page_id: None,
    });
    let root_id = sink.state().active_children()[0].id_str().to_string();
    let rounds = geometry_validate_and_fix(&mut sink, &root_id);
    assert!(rounds >= 1, "pushed-out stack must trigger a fix round");

    // Geometry proof: the name stack's right edge is back inside the cell.
    let rects = resolved_rects(sink.state());
    let v = serde_json::to_value(sink.state().active_children()[0].clone()).unwrap();
    fn find_id(v: &serde_json::Value, name: &str) -> Option<String> {
        if v.get("name").and_then(|x| x.as_str()) == Some(name) {
            return v.get("id").and_then(|x| x.as_str()).map(String::from);
        }
        v.get("children")
            .and_then(|c| c.as_array())
            .into_iter()
            .flatten()
            .find_map(|c| find_id(c, name))
    }
    let cell = &rects[&find_id(&v, "Cell Client").unwrap()];
    let stack = &rects[&find_id(&v, "Name Stack").unwrap()];
    assert!(
        stack.x + stack.w <= cell.x + cell.w + 2.0,
        "stack tail back inside the cell: stack_right={} cell_right={}",
        stack.x + stack.w,
        cell.x + cell.w
    );
}
