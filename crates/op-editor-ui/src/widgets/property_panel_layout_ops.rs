//! Layout / sizing / typography writers behind the PropertyPanel's
//! button groups.
//!
//! These used to live as byte-identical `impl WidgetHost*` blocks in
//! `op-host-native/src/widget_host/property_layout_dispatch.rs` and its
//! `op-host-web` twin. They are pure `EditorState` mutations keyed off
//! the widget-facade value enums, so they belong here — the hosts keep
//! only the layout-scene-dependent size resolution.

use jian_ops_schema::node::PenNode;
use jian_ops_schema::sizing::{SizingBehavior, SizingKeyword};
use op_editor_core::{EditorState, PropertyFocus};

use crate::widgets::property_panel::{
    LayoutAlignValue, LayoutJustifyValue, TextAlignValue, TextGrowthValue, TextVerticalAlignValue,
};

/// Write the flex-layout mode onto the selected node.
pub fn set_selected_layout_mode(state: &mut EditorState, mode: op_editor_core::FlexLayout) {
    let id = state.selection.anchor.clone();
    if !id.is_real() {
        return;
    }
    let value = match mode {
        op_editor_core::FlexLayout::Free => "none",
        op_editor_core::FlexLayout::Vertical => "vertical",
        op_editor_core::FlexLayout::Horizontal => "horizontal",
    };
    state.commit_history();
    let _ = state.apply(op_editor_core::EditorCommand::SetNodeLayoutProp {
        node_id: id,
        property: "layout".to_string(),
        value: op_editor_core::LayoutPropValue::Keyword(value.to_string()),
    });
}

/// Toggle a Fill / Hug sizing keyword on one axis.
///
/// Leaving Fill / Hug must freeze the layout-resolved canvas size: the
/// canonical keyword has no literal pixels, so `aggregate_bounds` can
/// collapse to the much smaller descendant-content union. The caller
/// passes `resolved_fallback` — the size read off the real layout scene
/// BEFORE any instance-write redirect — and it wins over the aggregate.
pub fn toggle_selected_sizing(
    state: &mut EditorState,
    width: bool,
    keyword: SizingKeyword,
    resolved_fallback: Option<f64>,
) {
    let id = state.selection.anchor.clone();
    if !id.is_real() {
        return;
    }
    let (is_current, aggregate_fallback) = {
        let Some(node) = state.selected_node() else {
            return;
        };
        let sizing = selected_sizing(node, width);
        let is_current = matches!(sizing, Some(SizingBehavior::Keyword(k)) if *k == keyword);
        let bounds = op_editor_core::aggregate_bounds(node);
        (is_current, if width { bounds.w } else { bounds.h })
    };
    let fallback = resolved_fallback.unwrap_or(aggregate_fallback);
    state.commit_history();
    if is_current {
        let focus = if width {
            PropertyFocus::SizeW
        } else {
            PropertyFocus::SizeH
        };
        let _ = state.commit_property_edit(focus, fallback.max(0.0) as f32);
    } else {
        let prop = if width { "width" } else { "height" };
        let value = match keyword {
            SizingKeyword::FitContent => "fit_content",
            SizingKeyword::FillContainer => "fill_container",
        };
        let _ = state.apply(op_editor_core::EditorCommand::SetNodeLayoutProp {
            node_id: id,
            property: prop.to_string(),
            value: op_editor_core::LayoutPropValue::Keyword(value.to_string()),
        });
    }
}

/// Flip `clipContent` on the selected container.
pub fn toggle_selected_clip_content(state: &mut EditorState) {
    let id = state.selection.anchor.clone();
    if !id.is_real() {
        return;
    }
    let current = state
        .selected_node()
        .map(selected_clip_content)
        .unwrap_or(false);
    state.commit_history();
    let _ = state.apply(op_editor_core::EditorCommand::SetNodeLayoutProp {
        node_id: id,
        property: "clipContent".to_string(),
        value: op_editor_core::LayoutPropValue::Bool(!current),
    });
}

pub fn set_selected_text_align(state: &mut EditorState, value: TextAlignValue) {
    let keyword = match value {
        TextAlignValue::Left => "left",
        TextAlignValue::Center => "center",
        TextAlignValue::Right => "right",
        TextAlignValue::Justify => "justify",
    };
    set_selected_layout_keyword(state, "textAlign", keyword);
}

pub fn set_selected_layout_align(state: &mut EditorState, value: LayoutAlignValue) {
    let keyword = match value {
        LayoutAlignValue::Start => "start",
        LayoutAlignValue::Center => "center",
        LayoutAlignValue::End => "end",
    };
    set_selected_layout_keyword(state, "alignItems", keyword);
}

pub fn set_selected_layout_justify(state: &mut EditorState, value: LayoutJustifyValue) {
    let keyword = match value {
        LayoutJustifyValue::Start => "start",
        LayoutJustifyValue::Center => "center",
        LayoutJustifyValue::End => "end",
        LayoutJustifyValue::SpaceBetween => "space_between",
        LayoutJustifyValue::SpaceAround => "space_around",
    };
    set_selected_layout_keyword(state, "justifyContent", keyword);
}

pub fn set_selected_text_vertical_align(state: &mut EditorState, value: TextVerticalAlignValue) {
    let keyword = match value {
        TextVerticalAlignValue::Top => "top",
        TextVerticalAlignValue::Middle => "middle",
        TextVerticalAlignValue::Bottom => "bottom",
    };
    set_selected_layout_keyword(state, "textAlignVertical", keyword);
}

pub fn set_selected_text_growth(state: &mut EditorState, value: TextGrowthValue) {
    let keyword = match value {
        TextGrowthValue::Auto => "auto",
        TextGrowthValue::FixedWidth => "fixed-width",
        TextGrowthValue::FixedWidthHeight => "fixed-width-height",
    };
    set_selected_layout_keyword(state, "textGrowth", keyword);
}

/// Commit a family picked in the font dropdown. A blank family is
/// dropped so the picker can't clear the authored `fontFamily`.
pub fn set_selected_text_font_family(state: &mut EditorState, family: &str) {
    if family.trim().is_empty() {
        return;
    }
    set_selected_layout_keyword(state, "fontFamily", family);
}

/// Commit a named-weight choice from the typography weight dropdown.
pub fn set_selected_font_weight(state: &mut EditorState, weight: u16) {
    let id = state.selection.anchor.clone();
    if !id.is_real() {
        return;
    }
    state.commit_history();
    let _ = state.commit_property_edit(PropertyFocus::FontWeight, weight as f32);
}

fn set_selected_layout_keyword(state: &mut EditorState, property: &str, keyword: &str) {
    let id = state.selection.anchor.clone();
    if !id.is_real() {
        return;
    }
    state.commit_history();
    let _ = state.apply(op_editor_core::EditorCommand::SetNodeLayoutProp {
        node_id: id,
        property: property.to_string(),
        value: op_editor_core::LayoutPropValue::Keyword(keyword.to_string()),
    });
}

fn selected_sizing(node: &PenNode, width: bool) -> Option<&SizingBehavior> {
    match (node, width) {
        (PenNode::Frame(n), true) => n.container.width.as_ref(),
        (PenNode::Frame(n), false) => n.container.height.as_ref(),
        (PenNode::Group(n), true) => n.container.width.as_ref(),
        (PenNode::Group(n), false) => n.container.height.as_ref(),
        (PenNode::Rectangle(n), true) => n.container.width.as_ref(),
        (PenNode::Rectangle(n), false) => n.container.height.as_ref(),
        (PenNode::Ellipse(n), true) => n.width.as_ref(),
        (PenNode::Ellipse(n), false) => n.height.as_ref(),
        (PenNode::Polygon(n), true) => n.width.as_ref(),
        (PenNode::Polygon(n), false) => n.height.as_ref(),
        (PenNode::Path(n), true) => n.width.as_ref(),
        (PenNode::Path(n), false) => n.height.as_ref(),
        (PenNode::Text(n), true) => n.width.as_ref(),
        (PenNode::Text(n), false) => n.height.as_ref(),
        (PenNode::TextInput(n), true) => n.width.as_ref(),
        (PenNode::TextInput(n), false) => n.height.as_ref(),
        (PenNode::Image(n), true) => n.width.as_ref(),
        (PenNode::Image(n), false) => n.height.as_ref(),
        (PenNode::IconFont(n), true) => n.width.as_ref(),
        (PenNode::IconFont(n), false) => n.height.as_ref(),
        (PenNode::TextArea(n), true) => n.width.as_ref(),
        (PenNode::TextArea(n), false) => n.height.as_ref(),
        (PenNode::Select(n), true) => n.width.as_ref(),
        (PenNode::Select(n), false) => n.height.as_ref(),
        (PenNode::Switch(n), true) => n.width.as_ref(),
        (PenNode::Switch(n), false) => n.height.as_ref(),
        (PenNode::Checkbox(n), true) => n.width.as_ref(),
        (PenNode::Checkbox(n), false) => n.height.as_ref(),
        (PenNode::Slider(n), true) => n.width.as_ref(),
        (PenNode::Slider(n), false) => n.height.as_ref(),
        (PenNode::RadioGroup(n), true) => n.width.as_ref(),
        (PenNode::RadioGroup(n), false) => n.height.as_ref(),
        (PenNode::NumberInput(n), true) => n.width.as_ref(),
        (PenNode::NumberInput(n), false) => n.height.as_ref(),
        (PenNode::Progress(n), true) => n.width.as_ref(),
        (PenNode::Progress(n), false) => n.height.as_ref(),
        (PenNode::Tabs(n), true) => n.width.as_ref(),
        (PenNode::Tabs(n), false) => n.height.as_ref(),
        (PenNode::Line(_) | PenNode::Ref(_), _) => None,
    }
}

fn selected_clip_content(node: &PenNode) -> bool {
    match node {
        PenNode::Frame(n) => n.container.clip_content.unwrap_or(false),
        PenNode::Group(n) => n.container.clip_content.unwrap_or(false),
        PenNode::Rectangle(n) => n.container.clip_content.unwrap_or(false),
        _ => false,
    }
}
