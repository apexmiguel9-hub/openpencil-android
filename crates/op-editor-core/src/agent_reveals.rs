//! Reveal bookkeeping for freshly inserted subtrees.
//!
//! Single-sources the before/after node-id diff plus the staggered
//! reveal-registration walk that op-orchestrator (`subagent.rs`) and
//! op-host-services (`design_agent_tools.rs` / `mcp_live.rs`) previously
//! kept as byte-identical private copies. Timing constants and the reveal
//! store live next door in [`crate::agent_indicators`].

use std::collections::HashSet;

use jian_ops_schema::node::{ContainerProps, PenNode, TextContent};

use crate::pen_node_ext::PenNodeExt;
use crate::EditorState;

/// Every node id in the active page's tree (recursive).
pub fn collect_active_node_ids(state: &EditorState) -> HashSet<String> {
    let mut out = HashSet::new();
    for node in state.active_children() {
        collect_node_ids(node, &mut out);
    }
    out
}

/// Insert `node`'s id and all descendant ids into `out`.
pub fn collect_node_ids(node: &PenNode, out: &mut HashSet<String>) {
    out.insert(node.id_str().to_string());
    if let Some(children) = node.children() {
        for child in children {
            collect_node_ids(child, out);
        }
    }
}

/// Register staggered reveals for every node that is present now but was
/// absent from `ids_before` (see [`crate::agent_indicators::add_reveal`]).
/// No-op when `indicator_epoch` is `None`.
pub fn register_new_node_reveals(
    ids_before: &HashSet<String>,
    state: &EditorState,
    indicator_epoch: Option<u64>,
    reveal_started_ms: u64,
) {
    let Some(epoch) = indicator_epoch else {
        return;
    };
    let mut stream = RevealStream {
        index: 0,
        next_start_ms: reveal_started_ms,
    };
    for node in state.active_children() {
        register_node_reveals(
            node,
            ids_before,
            epoch,
            reveal_started_ms,
            0,
            None,
            &mut stream,
        );
    }
}

struct RevealStream {
    index: u64,
    next_start_ms: u64,
}

fn register_node_reveals(
    node: &PenNode,
    ids_before: &HashSet<String>,
    epoch: u64,
    reveal_started_ms: u64,
    depth: u64,
    parent_reveal_start_ms: Option<u64>,
    stream: &mut RevealStream,
) {
    let id = node.id_str();
    let mut own_reveal_start_ms = parent_reveal_start_ms;
    if !ids_before.contains(id) && should_reveal_node(node, depth) {
        let own_stream_index = stream.index;
        stream.index += 1;
        let base_start =
            reveal_started_ms + crate::agent_indicators::reveal_offset_ms(depth, own_stream_index);
        let child_runway_start = parent_reveal_start_ms
            .map(|started_at| {
                started_at.saturating_add(crate::agent_indicators::REVEAL_CHILD_RUNWAY_MS)
            })
            .unwrap_or(reveal_started_ms);
        let started_at = base_start.max(child_runway_start).max(stream.next_start_ms);
        crate::agent_indicators::add_reveal(epoch, id, started_at);
        stream.next_start_ms =
            started_at.saturating_add(crate::agent_indicators::REVEAL_STAGGER_MS);
        own_reveal_start_ms = Some(started_at);
    }
    if let Some(children) = node.children() {
        for child in children {
            register_node_reveals(
                child,
                ids_before,
                epoch,
                reveal_started_ms,
                depth + 1,
                own_reveal_start_ms,
                stream,
            );
        }
    }
}

fn should_reveal_node(node: &PenNode, depth: u64) -> bool {
    depth == 0 || node_has_own_visual(node) || node_is_named_structure(node)
}

fn node_has_own_visual(node: &PenNode) -> bool {
    match node {
        PenNode::Frame(n) => {
            container_has_own_visual(&n.container) || n.image_search_query.is_some()
        }
        PenNode::Group(n) => container_has_own_visual(&n.container),
        PenNode::Rectangle(n) => container_has_own_visual(&n.container),
        PenNode::Ref(_) => false,
        PenNode::Text(n) => match &n.content {
            TextContent::Plain(s) => !s.is_empty(),
            TextContent::Styled(segments) => !segments.is_empty(),
        },
        _ => true,
    }
}

fn container_has_own_visual(container: &ContainerProps) -> bool {
    container
        .fill
        .as_ref()
        .is_some_and(|fills| !fills.is_empty())
        || container.stroke.is_some()
        || container
            .effects
            .as_ref()
            .is_some_and(|effects| !effects.is_empty())
}

fn node_is_named_structure(node: &PenNode) -> bool {
    if !node.is_container() {
        return false;
    }
    let base = node.base();
    base.role.as_deref().is_some_and(|role| !role.is_empty())
        || base.name.as_deref().is_some_and(|name| !name.is_empty())
}
