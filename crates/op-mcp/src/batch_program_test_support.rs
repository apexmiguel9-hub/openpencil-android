//! Shared helpers for the `batch_design` DSL program-executor test
//! modules (`batch_program_tests.rs` + `batch_program_repair_tests.rs`),
//! carved off `batch_program_tests.rs` to keep every file under the
//! 800-line cap.

use std::collections::BTreeMap;

use op_editor_core::{EditorCommand, PenNodeExt};
use serde_json::Value;

use super::batch_design_snapshot;
use super::{McpTool, ToolOutcome};

pub(crate) fn call_operations(
    state: &op_editor_core::EditorState,
    operations: &str,
) -> (Value, Option<EditorCommand>) {
    let tool = batch_design_snapshot(state);
    let mut args = BTreeMap::new();
    args.insert("operations".into(), operations.into());
    match tool.call(&args) {
        ToolOutcome::OkJson(json) => (serde_json::from_str(&json).expect("json"), None),
        ToolOutcome::OkJsonWithCommand(json, cmd) => {
            (serde_json::from_str(&json).expect("json"), Some(cmd))
        }
        other => panic!("expected a TS result envelope, got {other:?}"),
    }
}

/// Drive the executor with the internal best-effort policy (the
/// orchestrator's script-gen path) — for tests exercising per-line
/// semantics that need failing lines DROPPED rather than rolling back
/// the batch.
pub(crate) fn call_operations_best_effort(
    state: &op_editor_core::EditorState,
    operations: &str,
) -> (Value, Option<EditorCommand>) {
    let tool = batch_design_snapshot(state);
    let mut args = BTreeMap::new();
    args.insert("operations".into(), operations.into());
    args.insert("_line_policy".into(), "best_effort".into());
    match tool.call(&args) {
        ToolOutcome::OkJson(json) => (serde_json::from_str(&json).expect("json"), None),
        ToolOutcome::OkJsonWithCommand(json, cmd) => {
            (serde_json::from_str(&json).expect("json"), Some(cmd))
        }
        other => panic!("expected a TS result envelope, got {other:?}"),
    }
}

pub(crate) fn binding_id(envelope: &Value, binding: &str) -> String {
    envelope["results"]
        .as_array()
        .expect("results")
        .iter()
        .find(|r| r["binding"] == binding)
        .unwrap_or_else(|| panic!("binding {binding} missing in {envelope}"))["nodeId"]
        .as_str()
        .expect("nodeId")
        .to_string()
}

/// Recursively search a command (unwrapping `Batch`) for a leaked
/// `MergeAppState` — the regression guard for a failed stateful
/// `I()`/`R()` line (see `failed_insert_line_does_not_leak_merge_app_state`
/// / `failed_replace_line_does_not_leak_merge_app_state`).
pub(crate) fn contains_merge_app_state(cmd: &EditorCommand) -> bool {
    match cmd {
        EditorCommand::MergeAppState { .. } => true,
        EditorCommand::Batch { commands } => commands.iter().any(contains_merge_app_state),
        _ => false,
    }
}

pub(crate) fn subtree_contains_text(node: &jian_ops_schema::node::PenNode, expected: &str) -> bool {
    let value = serde_json::to_value(node).expect("node json");
    if value.get("type").and_then(Value::as_str) == Some("text")
        && value.get("content").and_then(Value::as_str) == Some(expected)
    {
        return true;
    }
    node.children()
        .map(|children| {
            children
                .iter()
                .any(|child| subtree_contains_text(child, expected))
        })
        .unwrap_or(false)
}
