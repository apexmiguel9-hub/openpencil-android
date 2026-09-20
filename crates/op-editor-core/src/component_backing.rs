//! Lightweight backing for components whose canonical roots live in a document.

use crate::node_id::NodeId;
use crate::pen_node_ext::PenNodeExt;
use jian_ops_schema::node::{FrameNode, GroupNode, PenNode, RectangleNode};
use jian_ops_schema::PenDocument;
use std::collections::HashMap;
use std::sync::Arc;

/// Stable-enough runtime route to a node in the canonical document.
///
/// Editor mutations can reorder a path, so every lookup verifies the final id
/// and callers fall back to a document-wide id walk on a stale location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DocumentNodeLocation {
    pub(crate) page_index: Option<usize>,
    pub(crate) child_path: Box<[usize]>,
}

pub(crate) type DocumentComponentBacking = Arc<HashMap<NodeId, DocumentNodeLocation>>;

/// Copy only a component root's own canonical properties.
///
/// Constructing the three supported component container variants field by
/// field is intentional. Cloning the enum and clearing `children` afterwards
/// would transiently deep-clone every nested reusable subtree during import,
/// recreating the peak this backing is meant to remove.
pub(crate) fn shallow_root(node: &PenNode) -> PenNode {
    match node {
        PenNode::Frame(frame) => PenNode::Frame(FrameNode {
            base: frame.base.clone(),
            container: frame.container.clone(),
            children: None,
            image_search_query: frame.image_search_query.clone(),
            reusable: frame.reusable,
            slot: frame.slot.clone(),
            state: frame.state.clone(),
            bindings: frame.bindings.clone(),
            events: frame.events.clone(),
            lifecycle: frame.lifecycle.clone(),
            semantics: frame.semantics.clone(),
            gestures: frame.gestures.clone(),
            route: frame.route.clone(),
            screen: frame.screen.clone(),
            breakpoint: frame.breakpoint,
        }),
        PenNode::Group(group) => PenNode::Group(GroupNode {
            base: group.base.clone(),
            container: group.container.clone(),
            children: None,
            state: group.state.clone(),
            bindings: group.bindings.clone(),
            events: group.events.clone(),
            lifecycle: group.lifecycle.clone(),
            semantics: group.semantics.clone(),
            gestures: group.gestures.clone(),
            route: group.route.clone(),
        }),
        PenNode::Rectangle(rectangle) => PenNode::Rectangle(RectangleNode {
            base: rectangle.base.clone(),
            container: rectangle.container.clone(),
            children: None,
            state: rectangle.state.clone(),
            bindings: rectangle.bindings.clone(),
            events: rectangle.events.clone(),
            lifecycle: rectangle.lifecycle.clone(),
            semantics: rectangle.semantics.clone(),
            gestures: rectangle.gestures.clone(),
            route: rectangle.route.clone(),
        }),
        other => other.clone(),
    }
}

pub(crate) fn resolve_document_location<'a>(
    doc: &'a PenDocument,
    location: &DocumentNodeLocation,
    expected_id: &str,
) -> Option<&'a PenNode> {
    let nodes = match location.page_index {
        Some(page_index) => &doc.pages.as_ref()?.get(page_index)?.children,
        None => &doc.children,
    };
    let (first, rest) = location.child_path.split_first()?;
    let mut node = nodes.get(*first)?;
    for index in rest {
        node = node.children()?.get(*index)?;
    }
    (node.id_str() == expected_id).then_some(node)
}

pub(crate) fn find_node_and_location<'a>(
    doc: &'a PenDocument,
    node_id: &str,
) -> Option<(&'a PenNode, DocumentNodeLocation)> {
    fn walk<'a>(
        nodes: &'a [PenNode],
        node_id: &str,
        page_index: Option<usize>,
        path: &mut Vec<usize>,
    ) -> Option<(&'a PenNode, DocumentNodeLocation)> {
        for (index, node) in nodes.iter().enumerate() {
            path.push(index);
            if node.id_str() == node_id {
                return Some((
                    node,
                    DocumentNodeLocation {
                        page_index,
                        child_path: path.clone().into_boxed_slice(),
                    },
                ));
            }
            if let Some(children) = node.children() {
                if let Some(hit) = walk(children, node_id, page_index, path) {
                    return Some(hit);
                }
            }
            path.pop();
        }
        None
    }

    let mut path = Vec::new();
    if let Some(pages) = doc.pages.as_ref() {
        for (page_index, page) in pages.iter().enumerate() {
            if let Some(hit) = walk(&page.children, node_id, Some(page_index), &mut path) {
                return Some(hit);
            }
        }
    }
    walk(&doc.children, node_id, None, &mut path)
}

pub(crate) fn find_node_in_document<'a>(
    doc: &'a PenDocument,
    node_id: &str,
) -> Option<&'a PenNode> {
    find_node_and_location(doc, node_id).map(|(node, _)| node)
}
