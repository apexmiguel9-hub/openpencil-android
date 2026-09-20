//! Section sizing repairs: collapse redundant `fill_container` content
//! sections and equalize horizontal card-row heights.

use super::*;

/// A fixed-height vertical artboard's direct content sections are content-sized
/// by default. A content-bearing `fill_container` child consumes the remaining
/// main-axis space and creates the large blank tail seen in `0713-1-ds.op`.
///
/// Keep the repair deliberately shallow: horizontal cross-axis stretch (for
/// example a desktop sidebar), nested layout contracts, explicit scroll
/// viewports, and empty flexible spacers are authored sizing decisions.
pub(super) fn collapse_fill_container_content_sections(sink: &mut dyn DocSink, root_id: &str) {
    let repairs: Vec<NodeId> = {
        let Some(root) = find_root(sink.state(), root_id) else {
            return;
        };
        if !is_fixed_height_vertical_root(root) {
            return;
        }
        let Some(children) = root.children() else {
            return;
        };
        children
            .iter()
            .filter(|child| is_ordinary_fill_height_content_frame(child))
            .map(|child| NodeId::new(child.id_str().to_string()))
            .collect()
    };

    for node_id in repairs {
        sink.apply(EditorCommand::SetNodeLayoutProp {
            node_id,
            property: "height".to_string(),
            value: LayoutPropValue::Keyword("fit_content".to_string()),
        });
    }
}

pub(super) fn is_fixed_height_vertical_root(root: &PenNode) -> bool {
    let Some(props) = frame_container_props(root) else {
        return false;
    };
    props.layout.as_ref() == Some(&LayoutMode::Vertical)
        && matches!(props.height.as_ref(), Some(SizingBehavior::Number(_)))
}

pub(super) fn is_ordinary_fill_height_content_frame(node: &PenNode) -> bool {
    let Some(props) = frame_container_props(node) else {
        return false;
    };
    matches!(
        props.height.as_ref(),
        Some(SizingBehavior::Keyword(SizingKeyword::FillContainer))
    ) && node.children().is_some_and(|children| !children.is_empty())
        && !is_explicit_remaining_height_consumer(node, props)
}

pub(super) fn is_explicit_remaining_height_consumer(
    node: &PenNode,
    props: &ContainerProps,
) -> bool {
    if props.clip_content == Some(true) {
        return true;
    }

    let role = node
        .base()
        .role
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if matches!(
        role.as_str(),
        "main"
            | "scroll"
            | "viewport"
            | "scroll-area"
            | "scroll-viewport"
            | "workspace"
            | "work-surface"
    ) {
        return true;
    }

    contains_any(
        &node_identity_haystack(node),
        &[
            "main content",
            "scroll",
            "viewport",
            "workspace",
            "work surface",
            "work-surface",
            "滚动",
            "视口",
            "工作区",
        ],
    )
}

pub(super) fn equalize_horizontal_card_heights(sink: &mut dyn DocSink, root_id: &str) {
    let repairs: Vec<CardHeightRepair> = {
        let Some(root) = find_root(sink.state(), root_id) else {
            return;
        };
        let mut repairs = Vec::new();
        collect_horizontal_card_height_repairs(root, &mut repairs);
        repairs
    };

    for repair in repairs {
        sink.apply(EditorCommand::SetNodeLayoutProp {
            node_id: repair.card_id,
            property: "height".to_string(),
            value: LayoutPropValue::Keyword("fill_container".to_string()),
        });
    }
}

#[derive(Debug, Clone)]
pub(super) struct CardHeightRepair {
    card_id: NodeId,
}

pub(super) fn collect_horizontal_card_height_repairs(
    node: &PenNode,
    repairs: &mut Vec<CardHeightRepair>,
) {
    if let Some(card_repairs) = horizontal_card_height_repairs(node) {
        repairs.extend(card_repairs);
    }
    if let Some(children) = node.children() {
        for child in children {
            collect_horizontal_card_height_repairs(child, repairs);
        }
    }
}

pub(super) fn horizontal_card_height_repairs(node: &PenNode) -> Option<Vec<CardHeightRepair>> {
    let props = frame_container_props(node)?;
    if props.layout.as_ref() != Some(&LayoutMode::Horizontal)
        || props.align_items.as_ref() != Some(&AlignItems::Stretch)
    {
        return None;
    }

    let mut cards = Vec::new();
    for child in node.children()? {
        let Some(child_props) = frame_container_props(child) else {
            continue;
        };
        // Equal height is a card-row convention, not a generic horizontal-row
        // convention. An unnamed title/location group beside a rating group has
        // the same fill-width + fit-height shape but must remain content-sized.
        if !has_explicit_card_semantics(child) {
            return None;
        }
        if child
            .children()
            .map(|children| children.is_empty())
            .unwrap_or(true)
        {
            continue;
        }
        if !width_is_fill_container(child_props) {
            return None;
        }
        if matches!(
            child_props.height.as_ref(),
            Some(SizingBehavior::Keyword(SizingKeyword::FitContent))
        ) {
            cards.push(child);
        }
    }

    (cards.len() >= 2).then(|| {
        cards
            .into_iter()
            .map(|card| CardHeightRepair {
                card_id: NodeId::new(card.id_str().to_string()),
            })
            .collect()
    })
}

pub(super) fn has_explicit_card_semantics(node: &PenNode) -> bool {
    matches!(
        node.base().role.as_deref().map(str::trim),
        Some(
            "card"
                | "image-card"
                | "product-card"
                | "restaurant-card"
                | "menu-card"
                | "feature-card"
        )
    ) || contains_any(&node_identity_haystack(node), &["card", "卡片"])
}
