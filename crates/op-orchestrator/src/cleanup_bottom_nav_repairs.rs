//! Bottom-nav repairs: duplicate-section removal and tab distribution.

use super::*;

/// Mobile root-level bottom-nav dedupe. Weak-model Chinese prompts can produce
/// both a localized bottom nav section and an English normalized bottom nav.
/// Keep the bottom-most/last top-level nav and remove earlier duplicates.
pub(super) fn remove_duplicate_bottom_nav_sections(sink: &mut dyn DocSink, root_id: &str) {
    let dupes: Vec<NodeId> = {
        let Some(root) = find_root(sink.state(), root_id) else {
            return;
        };
        if !is_mobile_root(root) {
            return;
        }
        let Some(children) = root.children() else {
            return;
        };
        let nav_indices: Vec<usize> = children
            .iter()
            .enumerate()
            .filter_map(|(index, child)| is_bottom_nav_section(child).then_some(index))
            .collect();
        if nav_indices.len() < 2 {
            return;
        }
        let keep_index = nav_indices
            .iter()
            .copied()
            .max_by(|a, b| compare_bottom_nav_position(children, *a, *b))
            .expect("nav_indices is non-empty");
        nav_indices
            .into_iter()
            .filter(|index| *index != keep_index)
            .map(|index| NodeId::new(children[index].id_str().to_string()))
            .collect()
    };
    for id in dupes {
        sink.apply(EditorCommand::DeleteNode {
            node_id: id,
            page_id: None,
        });
    }
}

pub(super) fn distribute_bottom_nav_tabs(sink: &mut dyn DocSink, root_id: &str) {
    let repairs: Vec<BottomNavTabDistributionRepair> = {
        let Some(root) = find_root(sink.state(), root_id) else {
            return;
        };
        let mut repairs = Vec::new();
        collect_bottom_nav_tab_distribution_repairs(root, &mut repairs);
        repairs
    };

    for repair in repairs {
        for tab_id in repair.tabs_to_fill {
            sink.apply(EditorCommand::SetNodeLayoutProp {
                node_id: tab_id,
                property: "width".to_string(),
                value: LayoutPropValue::Keyword("fill_container".to_string()),
            });
        }
        if repair.set_row_justify {
            sink.apply(EditorCommand::SetNodeLayoutProp {
                node_id: repair.row_id,
                property: "justifyContent".to_string(),
                value: LayoutPropValue::Keyword("space_between".to_string()),
            });
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct BottomNavTabDistributionRepair {
    row_id: NodeId,
    tabs_to_fill: Vec<NodeId>,
    set_row_justify: bool,
}

pub(super) fn collect_bottom_nav_tab_distribution_repairs(
    node: &PenNode,
    repairs: &mut Vec<BottomNavTabDistributionRepair>,
) {
    if let Some(repair) = bottom_nav_tab_distribution_repair(node) {
        repairs.push(repair);
    }
    if let Some(children) = node.children() {
        for child in children {
            collect_bottom_nav_tab_distribution_repairs(child, repairs);
        }
    }
}

pub(super) fn bottom_nav_tab_distribution_repair(
    node: &PenNode,
) -> Option<BottomNavTabDistributionRepair> {
    let props = frame_container_props(node)?;
    if props.layout.as_ref() != Some(&LayoutMode::Horizontal)
        // `clipContent: true` on a horizontal row is an explicit scrolling
        // contract. Product cards often contain both a favorite icon and text,
        // so shape alone must never turn such a rail into distributed nav tabs.
        || props.clip_content == Some(true)
        // This pass used to recurse over every icon+label row. Require the row
        // itself to carry bottom-nav semantics; unnamed structural navs are
        // handled (with bottom-position context) by mobile-chrome repair.
        || !is_bottom_nav_section(node)
    {
        return None;
    }
    let children = node.children()?;
    if children.len() < 3 || !children.iter().all(is_vertical_icon_label_tab_frame) {
        return None;
    }

    let tabs_to_fill = children
        .iter()
        .filter_map(|child| {
            let child_props = frame_container_props(child)?;
            (!width_is_fill_container(child_props)).then(|| NodeId::new(child.id_str().to_string()))
        })
        .collect::<Vec<_>>();
    let set_row_justify = !matches!(
        props.justify_content.as_ref(),
        Some(JustifyContent::SpaceBetween) | Some(JustifyContent::SpaceAround)
    );

    (!tabs_to_fill.is_empty() || set_row_justify).then(|| BottomNavTabDistributionRepair {
        row_id: NodeId::new(node.id_str().to_string()),
        tabs_to_fill,
        set_row_justify,
    })
}

pub(super) fn is_vertical_icon_label_tab_frame(node: &PenNode) -> bool {
    let Some(props) = frame_container_props(node) else {
        return false;
    };
    if props.layout.as_ref() != Some(&LayoutMode::Vertical) {
        return false;
    }
    let mut has_icon = false;
    let mut has_text = false;
    collect_icon_label_descendants(node, &mut has_icon, &mut has_text);
    has_icon && has_text
}

pub(super) fn collect_icon_label_descendants(
    node: &PenNode,
    has_icon: &mut bool,
    has_text: &mut bool,
) {
    if *has_icon && *has_text {
        return;
    }
    if let Some(children) = node.children() {
        for child in children {
            match child {
                PenNode::IconFont(_) => *has_icon = true,
                PenNode::Text(_) => *has_text = true,
                _ => {}
            }
            collect_icon_label_descendants(child, has_icon, has_text);
            if *has_icon && *has_text {
                return;
            }
        }
    }
}
