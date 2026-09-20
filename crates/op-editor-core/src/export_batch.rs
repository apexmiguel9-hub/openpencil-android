//! Planning layer for the batch "export every frame" flow.
//!
//! This module decides **what** gets exported and **under which file
//! name**; it renders nothing. The render + IO loop lives host-side
//! (`op_host_services::export_batch`) and drives the unmodified
//! single-frame exporter once per planned target, so batch output is
//! pixel-identical to exporting each frame by hand.
//!
//! Scope rules (mirrors the File-menu row's two labels):
//!   - 2 or more top-level frames selected → export exactly those,
//!   - otherwise → export every top-level frame on the active page.
//!
//! Ordering is **document child order** — the order the author put the
//! frames in, not their canvas x position — so a deck exports in
//! reading order even when the frames were dragged out of sequence.

use crate::node_id::NodeId;
use crate::pen_node_ext::PenNodeExt;
use crate::EditorState;
use jian_ops_schema::node::PenNode;
use std::collections::HashSet;

/// One planned output: the node to render and the file name to write
/// it under (relative to the user-chosen directory).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameExportTarget {
    pub node_id: NodeId,
    pub file_name: String,
}

/// Cap on the name portion of an export file name, in characters.
/// Long AI-authored frame names would otherwise blow past per-path
/// limits once the directory prefix is added.
const MAX_NAME_CHARS: usize = 80;

/// Top-level frames on the active page that can actually be rendered,
/// in document child order. Hidden frames are skipped: the exporter
/// resolves them to an empty paint, so including them would only
/// manufacture failures.
fn exportable_frames(state: &EditorState) -> Vec<(NodeId, String)> {
    state
        .active_children()
        .iter()
        .filter(|node| matches!(node, PenNode::Frame(_)))
        .filter(|node| node.base().visible != Some(false))
        .filter_map(|node| {
            let base = node.base();
            let id = NodeId::new_opt(base.id.clone())?;
            let name = base.name.clone().unwrap_or_default();
            Some((id, name))
        })
        .collect()
}

/// How many top-level frames the current selection covers. Non-frame
/// and nested selections do not count — the File-menu row uses this to
/// decide between its "all frames" and "N frames" labels.
pub fn selected_frame_count(state: &EditorState) -> usize {
    exportable_frames(state)
        .iter()
        .filter(|(id, _)| state.selection.contains(id))
        .count()
}

/// The frames a batch export would write, in document order, already
/// named. `extension` is the output extension without the dot.
pub fn plan_frame_exports(state: &EditorState, extension: &str) -> Vec<FrameExportTarget> {
    let all = exportable_frames(state);
    let selected: Vec<(NodeId, String)> = all
        .iter()
        .filter(|(id, _)| state.selection.contains(id))
        .cloned()
        .collect();
    let scope = if selected.len() >= 2 { selected } else { all };
    name_targets(&scope, extension)
}

/// Assign `<NN>-<name>.<ext>` file names to an ordered frame list.
/// The two-digit index runs over the exported list itself, so a
/// multi-select export still numbers from 01.
fn name_targets(frames: &[(NodeId, String)], extension: &str) -> Vec<FrameExportTarget> {
    let mut taken: HashSet<String> = HashSet::new();
    frames
        .iter()
        .enumerate()
        .map(|(i, (id, name))| {
            let index = i + 1;
            let mut stem = strip_matching_ordinal(&sanitize_frame_name(name), index);
            if stem.is_empty() {
                stem = sanitize_frame_name(id.as_str());
            }
            if stem.is_empty() {
                stem = "frame".to_string();
            }
            let file_name = unique_file_name(&format!("{index:02}-{stem}"), extension, &mut taken);
            FrameExportTarget {
                node_id: id.clone(),
                file_name,
            }
        })
        .collect()
}

/// Drop a leading ordinal from `name` when it is the very number the
/// file name is about to be prefixed with — an author who already
/// numbered the frames ("01 封面" as frame 1) gets `01-封面.png`, not
/// `01-01 封面.png`. Strictly an equality check, never a guess: "07 …"
/// in third position keeps its 07, because there the digits carry
/// information the index does not.
fn strip_matching_ordinal(name: &str, index: usize) -> String {
    let digits: String = name.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() || digits.parse::<usize>() != Ok(index) {
        return name.to_string();
    }
    let rest = &name[digits.len()..];
    let stripped = rest.trim_start_matches([' ', '-', '_', '.', '\t']);
    if stripped.len() == rest.len() || stripped.is_empty() {
        // No separator after the digits ("01封面" could be one word), or
        // nothing left to name the file with — keep the original.
        return name.to_string();
    }
    stripped.to_string()
}

/// Strip the characters a path cannot carry, under this planner's
/// per-name cap. The character rules themselves live in
/// [`crate::export_name::sanitize_name_component`] so batch and
/// single-shot exports cannot disagree on what a legal name is.
/// Returns an empty string when nothing printable survives — the
/// caller substitutes a fallback.
pub fn sanitize_frame_name(name: &str) -> String {
    crate::export_name::sanitize_name_component(name, MAX_NAME_CHARS)
}

/// Return `<stem>.<ext>`, appending `-2`, `-3`, … to the stem until the
/// name is unused. The numeric prefix already separates same-named
/// frames, so this is a guard rather than the common path — but it is
/// what makes "one target, one file" an invariant the export loop can
/// rely on instead of silently overwriting its own output.
fn unique_file_name(stem: &str, extension: &str, taken: &mut HashSet<String>) -> String {
    let mut candidate = format!("{stem}.{extension}");
    let mut suffix = 2u32;
    while taken.contains(&candidate) {
        candidate = format!("{stem}-{suffix}.{extension}");
        suffix += 1;
    }
    taken.insert(candidate.clone());
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_from(json: &str) -> EditorState {
        let doc = jian_ops_schema::load_str(json)
            .expect("fixture JSON parses")
            .value;
        EditorState::from_document(doc)
    }

    /// Five frames deliberately authored out of x order — the plan must
    /// follow the children array, not the canvas.
    fn deck_state() -> EditorState {
        state_from(
            r#"{"version":"1.0.0","children":[
                {"type":"frame","id":"f1","name":"封面","x":4800,"y":0,"width":100,"height":100},
                {"type":"frame","id":"f2","name":"步骤 1","x":0,"y":0,"width":100,"height":100},
                {"type":"text","id":"t1","name":"stray","content":"hi"},
                {"type":"frame","id":"f3","name":"结尾 CTA","x":2400,"y":0,"width":100,"height":100}
            ]}"#,
        )
    }

    #[test]
    fn plan_follows_document_order_and_skips_non_frames() {
        let plan = plan_frame_exports(&deck_state(), "png");
        assert_eq!(
            plan.iter().map(|t| t.node_id.as_str()).collect::<Vec<_>>(),
            vec!["f1", "f2", "f3"]
        );
        assert_eq!(
            plan.iter()
                .map(|t| t.file_name.as_str())
                .collect::<Vec<_>>(),
            vec!["01-封面.png", "02-步骤 1.png", "03-结尾 CTA.png"]
        );
    }

    #[test]
    fn hidden_frames_are_left_out_of_the_plan() {
        let state = state_from(
            r#"{"version":"1.0.0","children":[
                {"type":"frame","id":"f1","name":"A","width":10,"height":10},
                {"type":"frame","id":"f2","name":"B","width":10,"height":10,"visible":false},
                {"type":"frame","id":"f3","name":"C","width":10,"height":10}
            ]}"#,
        );
        let plan = plan_frame_exports(&state, "png");
        assert_eq!(
            plan.iter()
                .map(|t| t.file_name.as_str())
                .collect::<Vec<_>>(),
            vec!["01-A.png", "02-C.png"]
        );
    }

    #[test]
    fn two_or_more_selected_frames_narrow_the_scope_and_renumber() {
        let mut state = deck_state();
        state.selection.set = vec![NodeId::new("f3"), NodeId::new("f1")];
        state.selection.anchor = NodeId::new("f1");

        let plan = plan_frame_exports(&state, "png");

        // Document order, not selection order.
        assert_eq!(
            plan.iter().map(|t| t.node_id.as_str()).collect::<Vec<_>>(),
            vec!["f1", "f3"]
        );
        assert_eq!(plan[0].file_name, "01-封面.png");
        assert_eq!(plan[1].file_name, "02-结尾 CTA.png");
        assert_eq!(selected_frame_count(&state), 2);
    }

    #[test]
    fn a_single_selected_frame_still_exports_the_whole_page() {
        let mut state = deck_state();
        state.selection.set = vec![NodeId::new("f2")];
        state.selection.anchor = NodeId::new("f2");

        assert_eq!(selected_frame_count(&state), 1);
        assert_eq!(plan_frame_exports(&state, "png").len(), 3);
    }

    #[test]
    fn selected_non_frame_nodes_are_ignored_by_the_scope_rule() {
        let mut state = deck_state();
        // A text node plus one frame is NOT a 2-frame selection.
        state.selection.set = vec![NodeId::new("t1"), NodeId::new("f2")];
        state.selection.anchor = NodeId::new("f2");

        assert_eq!(selected_frame_count(&state), 1);
        assert_eq!(plan_frame_exports(&state, "png").len(), 3);
    }

    #[test]
    fn illegal_path_characters_are_replaced_and_collapsed() {
        assert_eq!(sanitize_frame_name("a/b:c*d"), "a-b-c-d");
        assert_eq!(sanitize_frame_name("a//b"), "a-b");
        assert_eq!(sanitize_frame_name("  spaced  "), "spaced");
        assert_eq!(sanitize_frame_name("trailing."), "trailing");
        assert_eq!(sanitize_frame_name("with\nnewline"), "with-newline");
        assert_eq!(sanitize_frame_name("///"), "");
        assert_eq!(sanitize_frame_name(&"x".repeat(200)).chars().count(), 80);
    }

    #[test]
    fn unnamed_frames_fall_back_to_their_node_id() {
        let state = state_from(
            r#"{"version":"1.0.0","children":[
                {"type":"frame","id":"f1","width":10,"height":10},
                {"type":"frame","id":"f2","name":"  ","width":10,"height":10}
            ]}"#,
        );
        let plan = plan_frame_exports(&state, "png");
        assert_eq!(
            plan.iter()
                .map(|t| t.file_name.as_str())
                .collect::<Vec<_>>(),
            vec!["01-f1.png", "02-f2.png"]
        );
    }

    #[test]
    fn an_authored_ordinal_is_not_doubled_when_it_matches_the_index() {
        let state = state_from(
            r#"{"version":"1.0.0","children":[
                {"type":"frame","id":"f1","name":"01 封面","width":10,"height":10},
                {"type":"frame","id":"f2","name":"02 步骤 1","width":10,"height":10},
                {"type":"frame","id":"f3","name":"07 结尾","width":10,"height":10},
                {"type":"frame","id":"f4","name":"04封面","width":10,"height":10}
            ]}"#,
        );
        let plan = plan_frame_exports(&state, "png");
        assert_eq!(
            plan.iter()
                .map(|t| t.file_name.as_str())
                .collect::<Vec<_>>(),
            vec![
                // Leading ordinal equals the index — dropped.
                "01-封面.png",
                "02-步骤 1.png",
                // 07 in third position carries real information — kept.
                "03-07 结尾.png",
                // No separator after the digits — left alone.
                "04-04封面.png",
            ]
        );
    }

    #[test]
    fn an_ordinal_only_name_keeps_its_digits() {
        let state = state_from(
            r#"{"version":"1.0.0","children":[
                {"type":"frame","id":"f1","name":"1","width":10,"height":10}
            ]}"#,
        );
        assert_eq!(plan_frame_exports(&state, "png")[0].file_name, "01-1.png");
    }

    #[test]
    fn duplicate_names_get_a_numeric_suffix() {
        let mut taken = HashSet::new();
        assert_eq!(
            unique_file_name("01-card", "png", &mut taken),
            "01-card.png"
        );
        assert_eq!(
            unique_file_name("01-card", "png", &mut taken),
            "01-card-2.png"
        );
        assert_eq!(
            unique_file_name("01-card", "png", &mut taken),
            "01-card-3.png"
        );
    }

    #[test]
    fn a_page_without_frames_plans_nothing() {
        let state = state_from(
            r#"{"version":"1.0.0","children":[{"type":"text","id":"t","content":"hi"}]}"#,
        );
        assert!(plan_frame_exports(&state, "png").is_empty());
        assert_eq!(selected_frame_count(&state), 0);
    }
}
