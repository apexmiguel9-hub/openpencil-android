//! `batch_design` write tool spine: input selection, the script-arg
//! expansion, phase dispatch, promotion + `$app`-state hoisting, and the
//! `design_skeleton` / `design_content` phase tools.
//!
//! The bulk of the machinery lives in flat siblings carved off for the
//! 800-line cap: `batch_design_dsl.rs` (the `I(parent, node)` program
//! grammar), `batch_design_normalize.rs` (lenient JSON shape repair),
//! `batch_design_wire.rs` (the `nodes_json` descriptor parser), and
//! `batch_design_fill_normalize.rs` (fill coercion).

use std::collections::BTreeMap;

use jian_ops_schema::node::PenNode;
use jian_ops_schema::promote::{promote_frame, widget_kind_for, PromoteNote};
use op_editor_core::{NodeId, PenNodeExt};

use super::batch_direct_ops::{is_direct_image_operation, parse_single_direct_operation};
use super::batch_layered::{dispatch_design_content, dispatch_design_skeleton};
use super::batch_page::{command_with_outer_page_id, optional_page_id};
use super::{EditorCommand, McpTool, ToolErrorCode, ToolOutcome};

use super::batch_design_dsl::parse_insert_operations;
use super::batch_design_dsl_error::OperationsError;
use super::batch_design_wire::parse_batch_items;

// Re-exported so the pre-split `crate::batch_design::<item>` paths that
// the rest of the crate uses keep resolving after the carve-off.
pub(crate) use super::batch_design_dsl::{find_top_level_char, split_operations};
pub(crate) use super::batch_design_normalize::{ensure_node_ids, normalize_node_shape};

// First-party `batch_design` tool — insert N leaf nodes on the
// active page in one atomic shot. Mirrors TS `batch_design` for
// the leaf subset.
//
// Wire shape: one scalar string arg `nodes_json` carrying a JSON
// array of node descriptors. The shell-core parser rejects
// structured args at the top level (so an LLM can't sneak a
// nested object past scalar contracts), but a JSON array
// embedded inside a quoted string round-trips cleanly. Each
// array entry is `{"kind":"...","name":"...","x":N,"y":N,
// "width":N,"height":N,"fill_hex":"#..."}` — the same shape
// `insert_node` accepts, minus the wire wrapping.
//
// The tool parses the inner JSON, validates EVERY entry, and
// emits `McpCommand::BatchInsert { items: ... }`. The apply
// path is all-or-nothing: a single bad entry rejects the whole
// batch so the LLM never sees a partial design tree.
// The `batch_design` tool (BatchDesign + batch_design_snapshot) lives in
// `batch_design_result.rs` so it can hold a document snapshot and emit TS's
// `{results:[{binding,nodeId}], nodeCount}` for the operations path (it
// predicts the host-assigned ids off the snapshot). Non-operations paths fall
// back to `dispatch_batch_design` here.

/// Shared core for `design_skeleton` / `design_content` /
/// `design_refine`. Each phase tool dispatches here with a label
/// stamped into the response so the LLM client can correlate the
/// call back to its layered-workflow phase. Today every phase
/// emits the same `BatchInsert` command — the phasing is purely
/// metadata. A future patch may grow per-phase apply semantics
/// (e.g. `design_refine` patching existing nodes via UpdateNode
/// batches) once a richer command exists.
pub(crate) fn dispatch_phase(args: &BTreeMap<String, String>, phase: &'static str) -> ToolOutcome {
    dispatch_batch_design(args, Some(phase))
}

/// Whether `args[key]` carries an actual input rather than an empty
/// placeholder (`""`, `[]`, `{}`, `null`).
pub(crate) fn carries_input(args: &BTreeMap<String, String>, key: &str) -> bool {
    args.get(key).is_some_and(|value| {
        let trimmed = value.trim();
        !matches!(trimmed, "" | "[]" | "{}" | "null")
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BatchInputKind {
    Script,
    Operations,
    NodesJson,
}

pub(crate) type BatchInputError = (ToolErrorCode, String);

/// Select the one non-empty write payload. Empty placeholders do not compete,
/// but two real payloads are always an error regardless of argument order.
pub(crate) fn select_batch_input(
    args: &BTreeMap<String, String>,
) -> Result<BatchInputKind, BatchInputError> {
    let active: Vec<BatchInputKind> = [
        ("script", BatchInputKind::Script),
        ("operations", BatchInputKind::Operations),
        ("nodes_json", BatchInputKind::NodesJson),
    ]
    .into_iter()
    .filter_map(|(key, kind)| carries_input(args, key).then_some(kind))
    .collect();
    match active.as_slice() {
        [kind] => Ok(*kind),
        [] => {
            // Preserve a lone slot's own parser error (`nodes_json:{}` is
            // malformed, `script:""` is empty) while treating placeholders
            // as absent whenever another slot carries the real input.
            let present: Vec<BatchInputKind> = [
                ("script", BatchInputKind::Script),
                ("operations", BatchInputKind::Operations),
                ("nodes_json", BatchInputKind::NodesJson),
            ]
            .into_iter()
            .filter_map(|(key, kind)| args.contains_key(key).then_some(kind))
            .collect();
            match present.as_slice() {
                [kind] => Ok(*kind),
                _ => Err((
                    ToolErrorCode::MissingArgument,
                    "one non-empty input is required: script, operations, or nodes_json".into(),
                )),
            }
        }
        _ => Err((
            ToolErrorCode::InvalidArgument,
            "provide only one of script, operations, or nodes_json".into(),
        )),
    }
}

/// Expand a `script` arg into the `operations` DSL program the rest of
/// `batch_design` already understands. Returns:
/// - `None` — the one active input is `operations` or `nodes_json`; an empty
///   `script` placeholder does not steal the route.
/// - `Some(Ok(rewritten))` — `script` removed, `operations` set to the
///   program the sandboxed runner recorded. Caller re-dispatches with the
///   rewritten args so BOTH the flat `dispatch_batch_design` path (used by
///   the `design_skeleton`/`design_content`/`design_refine` phase tools)
///   and the primary `batch_design` tool's richer `operations` handling
///   (`BatchDesign::call`, which intercepts `operations` before ever
///   calling `dispatch_batch_design` — see `batch_design_result.rs`) see
///   the exact same expansion and report through their own native shape.
/// - `Some(Err(outcome))` — zero/multiple real inputs, or (feature off) a real
///   `script` input.
pub(crate) fn expand_script_arg(
    args: &BTreeMap<String, String>,
) -> Option<Result<BTreeMap<String, String>, ToolOutcome>> {
    match select_batch_input(args) {
        Ok(BatchInputKind::Script) => {}
        Ok(BatchInputKind::Operations | BatchInputKind::NodesJson) => return None,
        Err((code, message)) => return Some(Err(ToolOutcome::Err(code, message))),
    }
    let Some(script) = args.get("script") else {
        return Some(Err(ToolOutcome::Err(
            ToolErrorCode::MissingArgument,
            "script argument is missing".into(),
        )));
    };
    #[cfg(feature = "script")]
    {
        let program = match crate::script_runner::run_script_to_program(script) {
            Ok(p) => p,
            Err(e) => {
                return Some(Err(ToolOutcome::Err(
                    ToolErrorCode::InvalidArgument,
                    e.to_string(),
                )))
            }
        };
        let mut forwarded = args.clone();
        forwarded.remove("script");
        forwarded.insert("operations".to_string(), program);
        Some(Ok(forwarded))
    }
    #[cfg(not(feature = "script"))]
    {
        let _ = script;
        Some(Err(ToolOutcome::Err(
            ToolErrorCode::InvalidArgument,
            "script input requires a script-enabled host build".into(),
        )))
    }
}

pub(crate) fn dispatch_batch_design(
    args: &BTreeMap<String, String>,
    phase: Option<&'static str>,
) -> ToolOutcome {
    if let Some(result) = expand_script_arg(args) {
        return match result {
            Ok(forwarded) => dispatch_batch_design(&forwarded, phase),
            Err(outcome) => outcome,
        };
    }
    let input = match select_batch_input(args) {
        Ok(input) => input,
        Err((code, message)) => return ToolOutcome::Err(code, message),
    };
    let page_id = optional_page_id(args);
    if input == BatchInputKind::Operations {
        let Some(operations) = args.get("operations") else {
            return ToolOutcome::Err(
                ToolErrorCode::MissingArgument,
                "operations argument is missing".into(),
            );
        };
        if let Some(phase) = phase.filter(|_| is_direct_image_operation(operations)) {
            return ToolOutcome::Err(
                ToolErrorCode::InvalidArgument,
                format!(
                    "design_{phase} legacy operations cannot execute G() safely: this compatibility path has no document snapshot, so it cannot enforce G() placement or target geometry. Use batch_design with the same operations payload."
                ),
            );
        }
        return match parse_operations(operations) {
            Ok(ParsedOperations::Insert {
                parent_id,
                mut nodes,
                count,
                promoted,
                ..
            }) => {
                let mut out = BTreeMap::new();
                out.insert("wrote".into(), "true".into());
                out.insert("count".into(), count.to_string());
                if let Some(phase) = phase {
                    out.insert("phase".into(), phase.into());
                }
                // Surface Phase E3 promotions so the client sees the
                // legacy role frames that were normalized into widget nodes.
                surface_promotions(&mut out, &promoted);
                let hoist = hoist_generation_state(&mut nodes);
                ToolOutcome::OkWithCommand(
                    out,
                    with_hoisted_state(
                        hoist,
                        EditorCommand::InsertSubtree {
                            nodes,
                            parent_id,
                            page_id,
                        },
                    ),
                )
            }
            Ok(ParsedOperations::Direct(command)) => {
                let mut out = BTreeMap::new();
                out.insert("wrote".into(), "true".into());
                out.insert("count".into(), "1".into());
                if let Some(phase) = phase {
                    out.insert("phase".into(), phase.into());
                }
                ToolOutcome::OkWithCommand(out, command_with_outer_page_id(command, page_id))
            }
            Err(e) => ToolOutcome::Err(ToolErrorCode::InvalidArgument, e.to_string()),
        };
    }
    let Some(raw) = args.get("nodes_json") else {
        return ToolOutcome::Err(
            ToolErrorCode::MissingArgument,
            "nodes_json argument is missing".into(),
        );
    };
    match parse_batch_items(raw) {
        Ok(items) if items.is_empty() => ToolOutcome::Err(
            ToolErrorCode::InvalidArgument,
            "nodes_json must contain at least one descriptor".into(),
        ),
        Ok(items) => {
            let mut out = BTreeMap::new();
            out.insert("wrote".into(), "true".into());
            out.insert("count".into(), items.len().to_string());
            if let Some(phase) = phase {
                out.insert("phase".into(), phase.into());
            }
            ToolOutcome::OkWithCommand(out, EditorCommand::BatchInsert { items, page_id })
        }
        Err(e) => ToolOutcome::Err(ToolErrorCode::InvalidArgument, e.to_string()),
    }
}

pub(crate) enum ParsedOperations {
    Insert {
        parent_id: NodeId,
        nodes: Vec<PenNode>,
        count: usize,
        /// One binding name per top-level `I()` op, used to trace post-remap
        /// ids back to bindings for TS's `results:[{binding,nodeId}]`.
        bindings: Vec<String>,
        /// Per-node legacy-frame promotions applied to `nodes` (Phase E3).
        /// Empty when the AI emitted no explicitly-marked role frames.
        promoted: Vec<PromoteNote>,
    },
    Direct(EditorCommand),
}

pub(crate) fn parse_operations(input: &str) -> Result<ParsedOperations, OperationsError> {
    let lines = split_operations(input);
    if lines.len() == 1 {
        if let Some(command) = parse_single_direct_operation(&lines[0])? {
            return Ok(ParsedOperations::Direct(command));
        }
    }
    let (parent_id, mut nodes, _count, bindings) = parse_insert_operations(input)?;
    // Phase E3 — normalize explicitly-marked legacy frames (`role:"input"`
    // etc., or `semantics.role == input`) into first-class widget nodes
    // BEFORE they become the inserted command, so an old-style
    // `frame role="input"` the AI emits lands a real `text_input` node. Both
    // consumers (flat `InsertSubtree` + the `BatchDesign` result path's
    // `InsertAuthoredSubtree`) see the promoted forest. Recount afterwards:
    // promotion drops the marked frame's children (widget nodes are leaves).
    let mut promoted = Vec::new();
    promote_in_slice(&mut nodes, &mut promoted);
    let count = count_forest(&nodes);
    Ok(ParsedOperations::Insert {
        parent_id,
        nodes,
        count,
        bindings,
        promoted,
    })
}

/// Recursive promotion pass mirroring `jian_ops_schema::promote::
/// promote_document`'s internal slice walker (which isn't `pub`): for every
/// node, if `widget_kind_for` flags it as an explicitly-marked frame, replace
/// it in place with the built widget node; otherwise recurse into container
/// children (Frame / Group / Rectangle / Tabs / Ref). Widget nodes are leaves,
/// so a promoted frame is never recursed into. `notes` collects one
/// `PromoteNote` per promotion for the result surface.
pub(crate) fn promote_in_slice(nodes: &mut [PenNode], notes: &mut Vec<PromoteNote>) {
    for node in nodes.iter_mut() {
        if let Some(kind) = widget_kind_for(node) {
            let PenNode::Frame(frame) = node.clone() else {
                // `widget_kind_for` only returns Some for a Frame.
                continue;
            };
            let from_role = frame
                .base
                .role
                .clone()
                .unwrap_or_else(|| "semantics.role=input".into());
            let id = frame.base.id.clone();
            *node = promote_frame(&frame, kind);
            notes.push(PromoteNote {
                node_id: id,
                from_role,
                to: kind.tag(),
            });
        } else if let Some(children) = node.children_mut() {
            promote_in_slice(children, notes);
        }
    }
}

/// Drain node-level `state` from a generated insert forest into one
/// doc-root [`EditorCommand::MergeAppState`], tagged with the weakest
/// (unplanned) priority — MCP inserts have no orchestrator plan index.
/// Returns `None` when no node declared state, so plain inserts keep
/// their existing single-command shape.
pub(crate) fn hoist_generation_state(nodes: &mut [PenNode]) -> Option<EditorCommand> {
    let cmd = op_editor_core::hoist_app_state(nodes, op_editor_core::UNPLANNED_APP_STATE_IDX);
    match &cmd {
        EditorCommand::MergeAppState { state, .. } if !state.is_empty() => Some(cmd),
        _ => None,
    }
}

/// Wrap `insert` in a [`EditorCommand::Batch`] carrying the hoisted
/// `MergeAppState` FIRST (so `$app` keys land before the nodes that
/// reference them), or return `insert` unchanged when nothing was
/// hoisted. `MergeAppState` allocates no node ids, so prepending it
/// never disturbs id prediction.
pub(crate) fn with_hoisted_state(
    hoist: Option<EditorCommand>,
    insert: EditorCommand,
) -> EditorCommand {
    match hoist {
        Some(merge) => EditorCommand::Batch {
            commands: vec![merge, insert],
        },
        None => insert,
    }
}

/// Count every node in a forest (subtree-inclusive). Used to keep the flat
/// `count` accurate after promotion drops a marked frame's children.
fn count_forest(nodes: &[PenNode]) -> usize {
    fn count_subtree(node: &PenNode) -> usize {
        1 + node
            .children()
            .map(|c| c.iter().map(count_subtree).sum::<usize>())
            .unwrap_or(0)
    }
    nodes.iter().map(count_subtree).sum()
}

/// Stamp Phase E3 promotion info into a flat string-map result. No-op when
/// nothing was promoted, so existing batch_design results are byte-identical
/// for the common (no legacy frames) case. The `promoted` line mirrors TS's
/// pipeline-warning convention ("promoted N legacy role frames"); a per-node
/// `<id>` → `<widget>` summary rides alongside for traceability.
pub(crate) fn surface_promotions(out: &mut BTreeMap<String, String>, promoted: &[PromoteNote]) {
    if promoted.is_empty() {
        return;
    }
    out.insert("promoted".into(), promoted.len().to_string());
    let detail = promoted
        .iter()
        .map(|n| format!("{}({} -> {})", n.node_id, n.from_role, n.to))
        .collect::<Vec<_>>()
        .join(", ");
    out.insert("promotedNodes".into(), detail);
}

/// `design_skeleton` — phase 1 of TS's layered design workflow.
/// Same wire shape as `batch_design`; the result payload carries
/// `phase=skeleton` so clients can phase their prompting.
pub struct DesignSkeleton;
impl McpTool for DesignSkeleton {
    fn name(&self) -> &str {
        "design_skeleton"
    }
    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        if args.contains_key("rootFrame") || args.contains_key("sections") {
            return dispatch_design_skeleton(args);
        }
        dispatch_phase(args, "skeleton")
    }
}
pub fn design_skeleton_snapshot() -> DesignSkeleton {
    DesignSkeleton
}

/// `design_content` — phase 2 of the layered design workflow.
/// Mirrors `batch_design` apply semantics; tagged `phase=content`.
pub struct DesignContent;
impl McpTool for DesignContent {
    fn name(&self) -> &str {
        "design_content"
    }
    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        if args.contains_key("children") || args.contains_key("sectionId") {
            return dispatch_design_content(args);
        }
        dispatch_phase(args, "content")
    }
}
pub fn design_content_snapshot() -> DesignContent {
    DesignContent
}

// `design_refine` (the DesignRefine tool + design_refine_snapshot) lives in
// `design_refine_result.rs` so it can build TS's rich `{rootId, totalNodeCount,
// fixes[], layoutSnapshot}` result (it needs a document snapshot + the layout
// helper, which would push this file over the 800-line cap).
