//! Command-application, state-hoisting and input-selection tests for
//! `mcp::batch_design::BatchDesign`.
//!
//! Covers `EditorCommand::BatchInsert` application, `$app` state hoisting,
//! the `script` input route, the empty-placeholder input-selection rules,
//! and container layout inference.
//!
//! Split out of `batch_design_tests.rs` to stay under the 800-line cap.

use super::test_fixtures::sample;
use super::{BatchInsertItem, EditorCommand, McpTool, ToolErrorCode, ToolOutcome};
use crate::batch_design_snapshot;
use std::collections::BTreeMap;

#[test]
fn batch_insert_command_adds_all_nodes() {
    let mut s = sample();
    let pre_root_len = s.active_children().len();
    assert!(s.apply(EditorCommand::BatchInsert {
        items: vec![
            BatchInsertItem {
                kind: "rect".into(),
                name: "A".into(),
                x: 0,
                y: 0,
                width: 10,
                height: 20,
                fill_hex: None,
                fill: None,
            },
            BatchInsertItem {
                kind: "ellipse".into(),
                name: "B".into(),
                x: 40,
                y: 50,
                width: 30,
                height: 30,
                fill_hex: Some("#00ff00".into()),
                fill: None,
            },
        ],
        page_id: None,
    }));
    assert_eq!(s.active_children().len(), pre_root_len + 2);
}

#[test]
fn batch_insert_command_atomic_on_bad_descriptor() {
    let mut s = sample();
    let pre_root_len = s.active_children().len();
    assert!(!s.apply(EditorCommand::BatchInsert {
        items: vec![
            BatchInsertItem {
                kind: "rect".into(),
                name: "A".into(),
                x: 0,
                y: 0,
                width: 10,
                height: 10,
                fill_hex: None,
                fill: None,
            },
            BatchInsertItem {
                kind: "blob".into(),
                name: "B".into(),
                x: 0,
                y: 0,
                width: 10,
                height: 10,
                fill_hex: None,
                fill: None,
            },
        ],
        page_id: None,
    }));
    assert_eq!(
        s.active_children().len(),
        pre_root_len,
        "no partial insertion"
    );
}

#[test]
fn batch_insert_command_rejects_empty_items() {
    let mut s = sample();
    assert!(!s.apply(EditorCommand::BatchInsert {
        items: vec![],
        page_id: None,
    }));
}

#[test]
fn batch_design_drops_ambiguous_auto_sizing_and_recovers_text_growth_words() {
    // `width:"auto"` is ambiguous in CSS (fill for a block's width, hug for its
    // height) — forcing either direction inverts intent half the time, so the
    // key must be DROPPED (schema default wins) while the node itself survives.
    // A misspelled `textGrowth` carrying clear words ("fixed_width_and_height")
    // must recover its meaning instead of silently reverting to the default.
    let tool = batch_design_snapshot(&sample());
    let mut args = BTreeMap::new();
    args.insert(
        "operations".into(),
        r##"root=I(null, {"type":"frame","name":"Block","width":"auto","height":"fit_content","children":[{"type":"text","name":"T","content":"hello","textGrowth":"fixed_width_and_height"}]})"##
            .into(),
    );
    let ToolOutcome::OkJsonWithCommand(_, EditorCommand::InsertAuthoredSubtree { nodes, .. }) =
        tool.call(&args)
    else {
        panic!("expected InsertSubtree command");
    };
    let value = serde_json::to_value(&nodes[0]).expect("node json");
    assert!(
        value.get("width").map(|w| !w.is_string()).unwrap_or(true),
        "ambiguous auto width dropped, got {:?}",
        value.get("width")
    );
    assert_eq!(value["height"], "fit_content", "valid keyword untouched");
    let text = &value["children"][0];
    assert_eq!(
        text["textGrowth"], "fixed-width-height",
        "word-based textGrowth spelling recovered"
    );
}

#[test]
fn batch_design_falls_back_to_root_for_phantom_parent_binding() {
    // A weak model copies the `sec` example binding as its FIRST line's
    // parent. The phantom parent used to ride into `InsertAuthoredSubtree`
    // unvalidated and the host rejected the WHOLE otherwise-valid program
    // (an orchestrator stats subtask retried its complete 4-card section
    // away). The tool must fall back to a root insert and surface a warning.
    let state = op_editor_core::EditorState::new();
    let tool = batch_design_snapshot(&state);
    let mut args = BTreeMap::new();
    args.insert(
        "operations".into(),
        "row=I(sec, {\"type\":\"frame\",\"name\":\"Stat Row\",\"layout\":\"horizontal\",\"gap\":24})\nc1=I(row, {\"type\":\"frame\",\"name\":\"Card\",\"layout\":\"vertical\"})".to_string(),
    );
    let ToolOutcome::OkJsonWithCommand(json, cmd) = tool.call(&args) else {
        panic!("expected a command outcome");
    };
    assert!(json.contains("warnings"), "phantom parent surfaced: {json}");
    let mut s2 = op_editor_core::EditorState::new();
    assert!(s2.apply(cmd), "root-fallback insert must apply cleanly");
    assert_eq!(s2.active_children().len(), 1, "one root landed");
    use op_editor_core::PenNodeExt;
    let root = &s2.active_children()[0];
    assert_eq!(
        root.children().map(|c| c.len()),
        Some(1),
        "card nested in row"
    );
}

#[test]
fn batch_design_operations_hoists_node_state() {
    // An I()-program insert whose root frame declares node-level
    // `state` must yield TWO sibling commands — MergeAppState(unplanned)
    // then the insert — batched by the program finisher's existing
    // 0/1/many wrap, with the node's `state` stripped.
    let tool = batch_design_snapshot(&sample());
    let mut args = BTreeMap::new();
    args.insert(
        "operations".into(),
        r##"root=I(null, {"type":"frame","name":"Card","width":320,"height":240,"state":{"count":{"type":"int","default":1}}})"##
            .into(),
    );
    match tool.call(&args) {
        ToolOutcome::OkJsonWithCommand(_, EditorCommand::Batch { commands }) => {
            assert_eq!(commands.len(), 2);
            match &commands[0] {
                EditorCommand::MergeAppState { plan_idx, state } => {
                    assert_eq!(*plan_idx, usize::MAX);
                    assert!(state.contains_key("count"));
                }
                other => panic!("expected MergeAppState first, got {other:?}"),
            }
            match &commands[1] {
                EditorCommand::InsertAuthoredSubtree { nodes, .. } => {
                    let v = serde_json::to_value(&nodes[0]).expect("json");
                    assert!(v.get("state").is_none(), "node state must be stripped");
                }
                other => panic!("expected InsertAuthoredSubtree second, got {other:?}"),
            }
        }
        other => panic!("expected Batch command, got {other:?}"),
    }
}

#[test]
fn batch_design_without_node_state_keeps_plain_command() {
    // No node-level state → the command shape is unchanged (no Batch).
    let tool = batch_design_snapshot(&sample());
    let mut args = BTreeMap::new();
    args.insert(
        "operations".into(),
        r##"root=I(null, {"type":"frame","name":"Plain","width":320,"height":240})"##.into(),
    );
    match tool.call(&args) {
        ToolOutcome::OkJsonWithCommand(_, EditorCommand::InsertAuthoredSubtree { .. }) => {}
        other => panic!("expected plain InsertAuthoredSubtree, got {other:?}"),
    }
}

#[test]
fn batch_design_noop_state_merge_still_lands_the_insert() {
    // Regression for the codex BLOCKER: regenerating a section into a
    // document whose root state ALREADY carries the declared key is a
    // completely normal flow (the merge is a legitimate additive
    // no-op), not a failure. Before the fix, `merge_app_state` returned
    // `false` for the fully-skipped-keys case, so the sim-validated
    // `ctx.emit` in `batch_program.rs` treated the merge as a failed
    // line — misreporting an `errors[]` entry for a line whose insert
    // had already landed — and the SAME `merge_app_state` bug would
    // sink the whole `Batch` at HOST apply time on the five other
    // `with_hoisted_state` producers (insert_node / replace_node /
    // design_content / design_skeleton / batch_design), since none of
    // them sim-validate before batching.
    use jian_ops_schema::state::{PrimitiveType, StateEntry, StateType};
    let mut state = sample();
    let mut existing: BTreeMap<String, StateEntry> = BTreeMap::new();
    existing.insert(
        "count".into(),
        StateEntry {
            kind: StateType::Primitive(PrimitiveType::Int),
            default: Some(serde_json::json!(1)),
            description: None,
            persist: None,
        },
    );
    state.doc.state = Some(existing);

    let tool = batch_design_snapshot(&state);
    let mut args = BTreeMap::new();
    args.insert(
        "operations".into(),
        r##"root=I(null, {"type":"frame","name":"Card","width":320,"height":240,"state":{"count":{"type":"int","default":1}}})"##
            .into(),
    );
    let (json, cmd) = match tool.call(&args) {
        ToolOutcome::OkJsonWithCommand(json, cmd) => (json, cmd),
        other => panic!("expected OkJsonWithCommand, got {other:?}"),
    };
    // The line must not be misreported as errored — the merge is a
    // designed no-op, not a failure.
    assert!(
        !json.contains("\"errors\""),
        "a no-op state merge must not surface as a line error: {json}"
    );

    let before = state.active_children().len();
    assert!(
        state.apply(cmd),
        "the outcome command must apply cleanly despite the pre-existing state key"
    );
    assert_eq!(
        state.active_children().len(),
        before + 1,
        "the insert must land even though its declared state key was a no-op"
    );
    assert_eq!(
        state
            .doc
            .state
            .as_ref()
            .unwrap()
            .get("count")
            .unwrap()
            .default,
        Some(serde_json::json!(1)),
        "the doc-owned state entry is untouched"
    );
}

#[test]
fn batch_design_promotes_radio_group_role() {
    // Task D2: jian's promote table grew a `radio-group` role (D1) — a
    // legacy frame marked `role:"radio-group"` must collapse into a real
    // `radio_group` node through the same operations/I() path, with each
    // visible text child becoming an option.
    let tool = batch_design_snapshot(&sample());
    let mut args = BTreeMap::new();
    args.insert(
        "operations".into(),
        r##"rg=I(null, {"type":"frame","name":"Plan","role":"radio-group","width":200,"height":80,"children":[{"type":"text","content":"Monthly"},{"type":"text","content":"Yearly"}]})"##
            .into(),
    );
    match tool.call(&args) {
        ToolOutcome::OkJsonWithCommand(_, EditorCommand::InsertAuthoredSubtree { nodes, .. }) => {
            let v = serde_json::to_value(&nodes[0]).expect("json");
            assert_eq!(v["type"], "radio_group", "role frame must promote, got {v}");
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
}

#[test]
fn insert_node_data_hoists_node_state() {
    use crate::write_tools::insert_node_snapshot;
    let tool = insert_node_snapshot();
    let mut args = BTreeMap::new();
    args.insert(
        "data".into(),
        r##"{"type":"frame","name":"Widgetful","width":200,"height":100,"state":{"on":{"type":"bool","default":false}},"children":[{"type":"text","content":"hi"}]}"##
            .into(),
    );
    match tool.call(&args) {
        ToolOutcome::OkWithCommand(_, EditorCommand::Batch { commands }) => {
            assert!(
                matches!(&commands[0], EditorCommand::MergeAppState { plan_idx, state }
                if *plan_idx == usize::MAX && state.contains_key("on"))
            );
            assert!(matches!(&commands[1], EditorCommand::InsertSubtree { .. }));
        }
        other => panic!("expected Batch command, got {other:?}"),
    }
}

#[test]
fn design_content_hoists_node_state() {
    use crate::batch_layered::dispatch_design_content;
    let mut args = BTreeMap::new();
    args.insert("sectionId".into(), "sec1".into());
    args.insert(
        "children".into(),
        r##"[{"type":"frame","name":"Counter","width":200,"height":100,"state":{"n":{"type":"int","default":0}}}]"##
            .into(),
    );
    match dispatch_design_content(&args) {
        ToolOutcome::OkWithCommand(_, EditorCommand::Batch { commands }) => {
            assert!(
                matches!(&commands[0], EditorCommand::MergeAppState { plan_idx, state }
                if *plan_idx == usize::MAX && state.contains_key("n"))
            );
            assert!(matches!(&commands[1], EditorCommand::InsertSubtree { .. }));
        }
        other => panic!("expected Batch command, got {other:?}"),
    }
}

#[cfg(feature = "script")]
#[test]
fn batch_design_script_input_builds_nodes() {
    let state = op_editor_core::EditorState::new();
    let tool = batch_design_snapshot(&state);
    let mut args = BTreeMap::new();
    args.insert(
        "script".to_string(),
        r#"const root = I(null, {type: "frame", name: "S"});
for (let i = 0; i < 3; i++) { I(root, {type: "text", content: "t" + i}); }"#
            .to_string(),
    );
    match tool.call(&args) {
        ToolOutcome::OkJsonWithCommand(json, _cmd) => {
            assert!(json.contains("\"nodeCount\""), "envelope: {json}");
        }
        other => panic!("expected OkJsonWithCommand, got {other:?}"),
    }
}

#[cfg(feature = "script")]
#[test]
fn batch_design_rejects_script_plus_operations() {
    let state = op_editor_core::EditorState::new();
    let tool = batch_design_snapshot(&state);
    let mut args = BTreeMap::new();
    args.insert(
        "script".to_string(),
        "I(null, {type: \"frame\"});".to_string(),
    );
    args.insert(
        "operations".to_string(),
        "r=I(null, {\"type\":\"frame\"})".to_string(),
    );
    match tool.call(&args) {
        ToolOutcome::Err(ToolErrorCode::InvalidArgument, msg) => {
            assert!(msg.contains("only one of"), "msg: {msg}");
        }
        other => panic!("expected InvalidArgument, got {other:?}"),
    }
}

#[cfg(not(feature = "script"))]
#[test]
fn batch_design_script_unavailable_without_feature() {
    let state = op_editor_core::EditorState::new();
    let tool = batch_design_snapshot(&state);
    let mut args = BTreeMap::new();
    args.insert("script".to_string(), "I(null, {});".to_string());
    match tool.call(&args) {
        ToolOutcome::Err(ToolErrorCode::InvalidArgument, msg) => {
            assert!(msg.contains("script-enabled"), "msg: {msg}");
        }
        other => panic!("expected InvalidArgument, got {other:?}"),
    }
}

/// Models routinely send the unused input slots along as empty placeholders.
/// Rejecting those killed a whole batch over a field the caller never meant
/// to fill (measured 2026-07-12: a `script` batch died on "provide only one
/// of script, operations, or nodes_json").
#[test]
fn empty_placeholder_slots_do_not_compete_with_the_real_input() {
    let tool = batch_design_snapshot(&sample());
    for placeholder in ["[]", "", "null", "{}"] {
        let mut args = BTreeMap::new();
        args.insert(
            "nodes_json".into(),
            r#"[{"kind":"rect","name":"A","x":0,"y":0,"width":10,"height":10}]"#.into(),
        );
        args.insert("operations".into(), placeholder.into());
        match tool.call(&args) {
            ToolOutcome::OkWithCommand(out, _) => {
                assert_eq!(
                    out.get("wrote").map(String::as_str),
                    Some("true"),
                    "{placeholder}"
                );
            }
            other => {
                panic!("empty `operations: {placeholder}` must not block nodes_json: {other:?}")
            }
        }
    }
}

#[test]
fn a_lone_empty_operations_list_still_reports_its_own_error() {
    // The placeholder tolerance must not turn an empty program into a silent
    // success: with nothing else to fall through to, the batch still reports.
    let tool = batch_design_snapshot(&sample());
    let mut args = BTreeMap::new();
    args.insert("operations".into(), "[]".into());
    match tool.call(&args) {
        ToolOutcome::Err(ToolErrorCode::InvalidArgument, _) => {}
        ToolOutcome::OkJson(json) => assert!(
            json.contains("\"applied\":false"),
            "an empty program must not read as applied: {json}"
        ),
        other => panic!("empty program silently accepted: {other:?}"),
    }
}

#[test]
fn two_real_batch_inputs_are_rejected_symmetrically() {
    let tool = batch_design_snapshot(&sample());
    let mut args = BTreeMap::new();
    args.insert(
        "operations".into(),
        r#"root=I(null,{"type":"frame","width":100,"height":100})"#.into(),
    );
    args.insert(
        "nodes_json".into(),
        r#"[{"kind":"rect","name":"A","x":0,"y":0,"width":10,"height":10}]"#.into(),
    );
    match tool.call(&args) {
        ToolOutcome::Err(ToolErrorCode::InvalidArgument, message) => {
            assert!(message.contains("only one of"));
        }
        other => panic!("two real inputs must not silently prefer one: {other:?}"),
    }
}

#[test]
fn an_empty_script_does_not_block_real_operations() {
    let tool = batch_design_snapshot(&sample());
    let mut args = BTreeMap::new();
    args.insert("script".into(), String::new());
    args.insert(
        "operations".into(),
        r#"root=I(null,{"type":"frame","width":100,"height":100})"#.into(),
    );
    assert!(matches!(
        tool.call(&args),
        ToolOutcome::OkJsonWithCommand(_, EditorCommand::InsertAuthoredSubtree { .. })
    ));
}

/// Layout omission is ambiguous: a title+rail section probably stacks, while
/// a toolbar or comparison group probably forms a row. Shape normalization
/// must preserve the omission; the agent loop reports it and asks the model to
/// choose explicitly instead of silently inventing design intent.
#[test]
fn a_container_without_a_layout_remains_unspecified() {
    let mut section = serde_json::json!({
        "type": "frame", "id": "n6", "name": "Popular Destinations",
        "width": "fill_container", "height": 240,
        "children": [
            { "type": "frame", "id": "n25", "name": "Section Header", "layout": "horizontal" },
            { "type": "frame", "id": "n28", "name": "Rail", "layout": "horizontal" }
        ]
    });
    crate::batch_design::normalize_node_shape(&mut section);
    assert!(
        section.get("layout").is_none(),
        "normalization must not guess whether the children form a row or column"
    );
}

#[test]
fn an_authored_layout_and_an_absolute_stack_are_left_alone() {
    let mut row = serde_json::json!({
        "type": "frame", "id": "row", "layout": "horizontal",
        "children": [
            { "type": "frame", "id": "a" },
            { "type": "frame", "id": "b" }
        ]
    });
    crate::batch_design::normalize_node_shape(&mut row);
    assert_eq!(
        row.get("layout").and_then(|v| v.as_str()),
        Some("horizontal")
    );

    // Absolute children are out of flow — direction is irrelevant, and the
    // caller may mean `layout: none`.
    let mut overlay = serde_json::json!({
        "type": "frame", "id": "overlay",
        "children": [
            { "type": "frame", "id": "badge", "x": 8, "y": 8 },
            { "type": "frame", "id": "heart", "x": 300, "y": 8 }
        ]
    });
    crate::batch_design::normalize_node_shape(&mut overlay);
    assert!(
        overlay.get("layout").is_none(),
        "an absolutely-positioned stack keeps its authored (absent) layout"
    );
}

#[test]
fn a_single_child_container_is_not_given_a_direction() {
    let mut wrapper = serde_json::json!({
        "type": "frame", "id": "w",
        "children": [{ "type": "image", "id": "img", "src": "data:image/png;base64,AA" }]
    });
    crate::batch_design::normalize_node_shape(&mut wrapper);
    assert!(
        wrapper.get("layout").is_none(),
        "one child has no direction to get wrong"
    );
}
