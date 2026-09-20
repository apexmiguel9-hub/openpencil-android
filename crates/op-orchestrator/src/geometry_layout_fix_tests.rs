//! Text-overflow, sibling-jam / overlap reporting and the repair-harness +
//! corpus replay tests.

use super::*;

#[test]
fn text_overflow_fix_skips_absolute_positioned_parents() {
    // Under `layout: none` children are absolutely positioned — a text wider
    // than the parent is a positioning choice, not a flex overflow to repair.
    let block = json!({
        "type":"frame","id":"blk","name":"Canvas","layout":"none","width":200,"height":200,"children":[
            {"type":"text","id":"t","name":"T","content":"a very long decorative caption"}
        ]
    });
    let mut rects = std::collections::HashMap::new();
    rects.insert(
        "blk".to_string(),
        Rect {
            x: 0.0,
            y: 0.0,
            w: 200.0,
            h: 200.0,
        },
    );
    rects.insert(
        "t".to_string(),
        Rect {
            x: 0.0,
            y: 0.0,
            w: 380.0,
            h: 20.0,
        },
    ); // wider than parent
    let mut cmds = Vec::new();
    collect_text_overflow_fixes(&block, &rects, &mut cmds);
    assert!(
        cmds.is_empty(),
        "no wrap ops under a layout:none parent, got {cmds:?}"
    );
}

#[test]
fn real_layout_reins_in_numeric_child_wider_than_its_row() {
    use crate::test_support::VecDocSink;
    use crate::types::DocSink;
    use jian_ops_schema::node::PenNode;
    use op_editor_core::PenNodeExt;

    // glm7's authored defect verbatim: an 800px-wide avatar bar inside a
    // ~550px appointment row — it spilled across the whole design and past
    // every tree-shape pass. The geometry loop must retarget it to
    // fill_container so it stays inside the row.
    let root: PenNode = serde_json::from_value(json!({
        "type":"frame","id":"root","name":"Root","width":560,"height":"fit_content","layout":"vertical","children":[
            {"type":"frame","id":"row","name":"Appt Row","layout":"horizontal","gap":14,"width":"fill_container","height":"fit_content","children":[
                {"type":"frame","id":"time","name":"Time","width":52,"height":36,"children":[]},
                {"type":"frame","id":"bar","name":"Avatar Bar","width":800,"height":36,"children":[]}
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

    let rounds = geometry_validate_and_fix(&mut sink, &root_id);
    assert!(rounds >= 1, "the overflow must trigger a fix round");

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
    let bar = find(&v, "Avatar Bar").expect("bar survives");
    assert_eq!(
        bar.get("width").and_then(|w| w.as_str()),
        Some("fill_container"),
        "oversized numeric child reined in, got {:?}",
        bar.get("width")
    );
    let time = find(&v, "Time").expect("time survives");
    assert_eq!(
        time.get("width").and_then(|w| w.as_f64()),
        Some(52.0),
        "the fitting fixed column is untouched"
    );
}

#[test]
fn jammed_text_columns_are_reported_but_flush_icons_are_not() {
    // Row of three cells: [date-cell][visits-cell] touch (0px apart, both carry
    // text → jam), while [icon][icon] flush contact is fine.
    let row = json!({
        "type":"frame","id":"row","name":"Row","layout":"horizontal","children":[
            {"type":"frame","id":"date","name":"Date","children":[{"type":"text","id":"dt","content":"Oct 24, 2024"}]},
            {"type":"frame","id":"visits","name":"Visits","children":[{"type":"text","id":"vt","content":"42"}]},
            {"type":"frame","id":"ic1","name":"Icon A","children":[]},
            {"type":"frame","id":"ic2","name":"Icon B","children":[]}
        ]
    });
    let mut rects = std::collections::HashMap::new();
    rects.insert(
        "row".into(),
        Rect {
            x: 0.0,
            y: 0.0,
            w: 400.0,
            h: 40.0,
        },
    );
    rects.insert(
        "date".into(),
        Rect {
            x: 0.0,
            y: 0.0,
            w: 110.0,
            h: 40.0,
        },
    );
    rects.insert(
        "visits".into(),
        Rect {
            x: 110.0,
            y: 0.0,
            w: 30.0,
            h: 40.0,
        },
    ); // touches date
    rects.insert(
        "ic1".into(),
        Rect {
            x: 200.0,
            y: 0.0,
            w: 18.0,
            h: 18.0,
        },
    );
    rects.insert(
        "ic2".into(),
        Rect {
            x: 218.0,
            y: 0.0,
            w: 18.0,
            h: 18.0,
        },
    ); // flush icons
    let mut out = Vec::new();
    collect_sibling_jam_diagnostics(&row, &rects, &mut out);
    assert_eq!(out.len(), 1, "exactly the text jam is reported: {out:?}");
    assert!(out[0].contains("Date") && out[0].contains("Visits"));
}

#[test]
fn overlapping_siblings_are_reported_regardless_of_content() {
    let row = json!({
        "type":"frame","id":"row","name":"Row","layout":"horizontal","children":[
            {"type":"frame","id":"a","name":"Left","children":[]},
            {"type":"frame","id":"b","name":"Right","children":[]}
        ]
    });
    let mut rects = std::collections::HashMap::new();
    rects.insert(
        "row".into(),
        Rect {
            x: 0.0,
            y: 0.0,
            w: 300.0,
            h: 40.0,
        },
    );
    rects.insert(
        "a".into(),
        Rect {
            x: 0.0,
            y: 0.0,
            w: 200.0,
            h: 40.0,
        },
    );
    rects.insert(
        "b".into(),
        Rect {
            x: 150.0,
            y: 0.0,
            w: 100.0,
            h: 40.0,
        },
    ); // 50px overlap
    let mut out = Vec::new();
    collect_sibling_jam_diagnostics(&row, &rects, &mut out);
    assert_eq!(out.len(), 1);
    assert!(out[0].contains("OVERLAP"), "got {out:?}");
}

#[test]
fn vertical_overlapping_siblings_are_reported() {
    let stack = json!({
        "type":"frame","id":"stack","name":"Stack","layout":"vertical","children":[
            {"type":"frame","id":"a","name":"Contact Block","children":[]},
            {"type":"frame","id":"b","name":"Footer","children":[]}
        ]
    });
    let mut rects = std::collections::HashMap::new();
    rects.insert(
        "a".into(),
        Rect {
            x: 0.0,
            y: 10.0,
            w: 240.0,
            h: 60.0,
        },
    );
    rects.insert(
        "b".into(),
        Rect {
            x: 0.0,
            y: 65.0,
            w: 240.0,
            h: 40.0,
        },
    ); // 5px vertical overlap
    let mut out = Vec::new();
    collect_sibling_jam_diagnostics(&stack, &rects, &mut out);
    assert_eq!(out.len(), 1);
    assert!(
        out[0].contains("Contact Block") && out[0].contains("Footer"),
        "got {out:?}"
    );
    assert!(out[0].contains("OVERLAP"), "got {out:?}");
}

#[test]
fn vertical_ring_badge_overlay_is_not_reported_as_an_overlap() {
    let stack = json!({
        "type":"frame","id":"stack","name":"Ring","layout":"vertical","children":[
            {"type":"ellipse","id":"e","width":36,"height":36},
            {"type":"text","id":"t","content":"2","fontSize":15}
        ]
    });
    let mut rects = std::collections::HashMap::new();
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
    );
    let mut out = Vec::new();
    collect_sibling_jam_diagnostics(&stack, &rects, &mut out);
    assert!(out.is_empty(), "overlay must not be reported: {out:?}");
}

/// Manual repair harness (not part of the suite): load OP_REPAIR_IN, run the
/// whole-doc loop finalize (Class-A + cleanup + geometry), save OP_REPAIR_OUT.
/// `OP_REPAIR_IN=/path/in.op OP_REPAIR_OUT=/path/out.op cargo test -p
/// op-orchestrator repair_harness -- --ignored --nocapture`
#[test]
#[ignore]
fn repair_harness_finalizes_an_op_file() {
    let inp = std::env::var("OP_REPAIR_IN").expect("OP_REPAIR_IN");
    let out = std::env::var("OP_REPAIR_OUT").expect("OP_REPAIR_OUT");
    let text = std::fs::read_to_string(&inp).expect("read input");
    let doc: jian_ops_schema::PenDocument = serde_json::from_str(&text).expect("parse .op");
    let mut state = op_editor_core::EditorState::from_document(doc);
    crate::loop_finalize::apply_loop_finalize(&mut state);
    std::fs::write(&out, serde_json::to_string_pretty(&state.doc).unwrap()).expect("write output");
    eprintln!("repaired {inp} -> {out}");
}

/// Manual harness variant: run ONLY the orchestrator's `finalize_design`
/// (no whole-doc Class-A prelude) — for bisecting orchestrator-vs-loop
/// finalize differences on a real file.
#[test]
#[ignore]
fn finalize_only_harness() {
    let inp = std::env::var("OP_REPAIR_IN").expect("OP_REPAIR_IN");
    let out = std::env::var("OP_REPAIR_OUT").expect("OP_REPAIR_OUT");
    let text = std::fs::read_to_string(&inp).expect("read input");
    let doc: jian_ops_schema::PenDocument = serde_json::from_str(&text).expect("parse .op");
    let mut state = op_editor_core::EditorState::from_document(doc);
    use op_editor_core::PenNodeExt;
    let root_id = state.active_children()[0].id_str().to_string();
    let plan: crate::plan::OrchestratorPlan = serde_json::from_value(serde_json::json!({
        "rootFrame": {"id":"root","name":"Page","width":1200,"height":800,"layout":"vertical"},
        "subtasks": []
    }))
    .expect("stub plan");
    let mut sink = crate::loop_finalize::StateDocSink { state: &mut state };
    crate::cleanup::finalize_design(&mut sink, &plan, &[&root_id]);
    std::fs::write(&out, serde_json::to_string_pretty(&state.doc).unwrap()).expect("write");
    eprintln!("finalized {inp} -> {out}");
}

/// Manual probe: print resolved rects for nodes whose name matches
/// OP_PROBE_NAME inside OP_REPAIR_IN. Set OP_PROBE_UNDER=1 to print the
/// whole resolved subtree of every match (rows/cells are usually unnamed,
/// so matching the named ancestor and dumping under it is the useful mode).
#[test]
#[ignore]
fn resolved_rect_probe() {
    let inp = std::env::var("OP_REPAIR_IN").expect("OP_REPAIR_IN");
    let pat = std::env::var("OP_PROBE_NAME")
        .unwrap_or_default()
        .to_lowercase();
    let under = std::env::var("OP_PROBE_UNDER").is_ok();
    let text = std::fs::read_to_string(&inp).expect("read input");
    let doc: jian_ops_schema::PenDocument = serde_json::from_str(&text).expect("parse .op");
    let state = op_editor_core::EditorState::from_document(doc);
    let rects = resolved_rects(&state);
    for root in state.active_children() {
        let v = serde_json::to_value(root).unwrap();
        #[allow(clippy::too_many_arguments)]
        fn walk(
            v: &serde_json::Value,
            rects: &HashMap<String, Rect>,
            pat: &str,
            under: bool,
            in_match: bool,
            depth: usize,
        ) {
            let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("");
            let nid = v.get("id").and_then(|x| x.as_str()).unwrap_or("");
            let hit = !pat.is_empty()
                && (name.to_lowercase().contains(pat) || nid.eq_ignore_ascii_case(pat));
            if hit || (under && in_match) {
                let kind = v.get("type").and_then(|x| x.as_str()).unwrap_or("?");
                let label = if name.is_empty() { nid } else { name };
                if let Some(r) = rects.get(nid) {
                    eprintln!(
                        "{:indent$}{label} [{kind}]: x={:.2} y={:.2} w={:.2} h={:.2}",
                        "",
                        r.x,
                        r.y,
                        r.w,
                        r.h,
                        indent = depth
                    );
                } else {
                    eprintln!("{:indent$}{label} [{kind}]: <no rect>", "", indent = depth);
                }
            }
            for c in v
                .get("children")
                .and_then(|c| c.as_array())
                .into_iter()
                .flatten()
            {
                walk(c, rects, pat, under, in_match || hit, depth + 1);
            }
        }
        walk(&v, &rects, &pat, under, false, 0);
    }
}

/// Manual corpus replay: load p01.op..p52.op from OP_GEOMETRY_REPLAY_DIR, run
/// geometry_validate_and_fix on every active root, and assert the parsed doc is
/// unchanged. This is the dirty-diff gate for geometry-only replay.
#[test]
#[ignore]
fn replay_geometry_validate_corpus() {
    let dir = std::env::var("OP_GEOMETRY_REPLAY_DIR").expect("OP_GEOMETRY_REPLAY_DIR");
    let out_dir = std::env::var("OP_GEOMETRY_REPLAY_OUT_DIR").ok();
    if let Some(out_dir) = &out_dir {
        std::fs::create_dir_all(out_dir).expect("create replay out dir");
    }
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .expect("read replay dir")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(is_numbered_corpus_op)
        })
        .collect();
    files.sort();

    let mut dirty = Vec::new();
    let mut baseline_input_dirty = Vec::new();
    let mut baseline_rounds = 0usize;
    let mut current_rounds = 0usize;
    for path in &files {
        let text = std::fs::read_to_string(path).expect("read corpus op");
        let doc: jian_ops_schema::PenDocument = serde_json::from_str(&text).expect("parse op");
        let mut baseline_state = op_editor_core::EditorState::from_document(doc.clone());
        let mut current_state = op_editor_core::EditorState::from_document(doc);
        let before = serde_json::to_value(&current_state.doc).expect("before value");
        let root_ids: Vec<String> = current_state
            .active_children()
            .iter()
            .map(|root| {
                use op_editor_core::PenNodeExt;
                root.id_str().to_string()
            })
            .collect();
        let baseline_root_ids = root_ids.clone();
        {
            let mut sink = crate::loop_finalize::StateDocSink {
                state: &mut baseline_state,
            };
            for root_id in baseline_root_ids {
                baseline_rounds += geometry_validate_and_fix_without_card_rows(&mut sink, &root_id);
            }
        }
        {
            let mut sink = crate::loop_finalize::StateDocSink {
                state: &mut current_state,
            };
            for root_id in root_ids {
                current_rounds += geometry_validate_and_fix(&mut sink, &root_id);
            }
        }
        let baseline_after = serde_json::to_value(&baseline_state.doc).expect("baseline value");
        let current_after = serde_json::to_value(&current_state.doc).expect("current value");
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("?")
            .to_string();
        if before != baseline_after {
            baseline_input_dirty.push(name.clone());
        }
        if baseline_after != current_after {
            if let Some(out_dir) = &out_dir {
                let out_path = std::path::Path::new(out_dir).join(format!("{name}.current.op"));
                std::fs::write(
                    out_path,
                    serde_json::to_string_pretty(&current_state.doc).expect("serialize dirty doc"),
                )
                .expect("write dirty doc");
            }
            dirty.push(name);
        }
    }

    eprintln!(
        "[GEOMETRY-REPLAY] checked={} baseline_input_dirty={} dirty={} baseline_rounds={} current_rounds={} dirty_files={:?} baseline_input_dirty_files={:?}",
        files.len(),
        baseline_input_dirty.len(),
        dirty.len(),
        baseline_rounds,
        current_rounds,
        dirty,
        baseline_input_dirty
    );
    assert_eq!(files.len(), 52, "expected p01.op..p52.op corpus");
    assert!(
        dirty.is_empty(),
        "dirty geometry replay files versus baseline: {dirty:?}"
    );
}
