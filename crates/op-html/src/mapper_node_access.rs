//! Variant-agnostic accessors over `PenNode`.
//!
//! The canonical schema keeps `PenNodeBase` behind `#[serde(flatten)]` on 21
//! node variants and splits sizing between `ContainerProps` (Frame / Group /
//! Rectangle) and per-leaf `width` / `height` fields. The grid, stacking,
//! wrap and offset post-passes all need the same match arms, so they live
//! here once instead of being re-spelled per module.

use jian_ops_schema::node::base::PenNodeBase;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::sizing::SizingBehavior;

macro_rules! base_ref {
    ($node:expr; $($variant:ident),+ $(,)?) => {
        match $node { $(PenNode::$variant(node) => &node.base,)+ }
    };
}

macro_rules! base_mut {
    ($node:expr; $($variant:ident),+ $(,)?) => {
        match $node { $(PenNode::$variant(node) => &mut node.base,)+ }
    };
}

pub(crate) fn node_base(node: &PenNode) -> &PenNodeBase {
    base_ref!(node; Frame, Group, Rectangle, Ellipse, Line, Polygon, Path, Text,
        TextInput, Image, IconFont, TextArea, Select, Switch, Checkbox, Slider,
        RadioGroup, NumberInput, Progress, Tabs, Ref)
}

pub(crate) fn node_base_mut(node: &mut PenNode) -> &mut PenNodeBase {
    base_mut!(node; Frame, Group, Rectangle, Ellipse, Line, Polygon, Path, Text,
        TextInput, Image, IconFont, TextArea, Select, Switch, Checkbox, Slider,
        RadioGroup, NumberInput, Progress, Tabs, Ref)
}

/// NOTE: the wildcard arm is a silent-drift surface. Unlike `base_ref!` /
/// `base_mut!`, which enumerate every variant and therefore fail to compile
/// when the schema grows one, a new sizing-carrying variant added upstream
/// falls through here as "no width / height" with no diagnostic. The
/// exhaustive `node_width` / `node_height` matches below are the guard: keep
/// this macro's variant list in step with them.
macro_rules! leaf_axis_mut {
    ($node:expr, $field:ident; $($variant:ident),+ $(,)?) => {
        match $node { $(PenNode::$variant(node) => Some(&mut node.$field),)+
            _ => None }
    };
}

/// Mutable handle on the node's authored width, wherever the schema keeps it.
pub(crate) fn node_width_mut(node: &mut PenNode) -> Option<&mut Option<SizingBehavior>> {
    match node {
        PenNode::Frame(node) => Some(&mut node.container.width),
        PenNode::Group(node) => Some(&mut node.container.width),
        PenNode::Rectangle(node) => Some(&mut node.container.width),
        node => leaf_axis_mut!(node, width; Ellipse, Polygon, Path, Text, TextInput,
            Image, IconFont, TextArea, Select, Switch, Checkbox, Slider,
            RadioGroup, NumberInput, Progress, Tabs),
    }
}

pub(crate) fn node_width(node: &PenNode) -> Option<&SizingBehavior> {
    match node {
        PenNode::Frame(node) => node.container.width.as_ref(),
        PenNode::Group(node) => node.container.width.as_ref(),
        PenNode::Rectangle(node) => node.container.width.as_ref(),
        PenNode::Ellipse(node) => node.width.as_ref(),
        PenNode::Polygon(node) => node.width.as_ref(),
        PenNode::Path(node) => node.width.as_ref(),
        PenNode::Text(node) => node.width.as_ref(),
        PenNode::TextInput(node) => node.width.as_ref(),
        PenNode::Image(node) => node.width.as_ref(),
        PenNode::IconFont(node) => node.width.as_ref(),
        PenNode::TextArea(node) => node.width.as_ref(),
        PenNode::Select(node) => node.width.as_ref(),
        PenNode::Switch(node) => node.width.as_ref(),
        PenNode::Checkbox(node) => node.width.as_ref(),
        PenNode::Slider(node) => node.width.as_ref(),
        PenNode::RadioGroup(node) => node.width.as_ref(),
        PenNode::NumberInput(node) => node.width.as_ref(),
        PenNode::Progress(node) => node.width.as_ref(),
        PenNode::Tabs(node) => node.width.as_ref(),
        PenNode::Line(_) | PenNode::Ref(_) => None,
    }
}

pub(crate) fn node_height(node: &PenNode) -> Option<&SizingBehavior> {
    match node {
        PenNode::Frame(node) => node.container.height.as_ref(),
        PenNode::Group(node) => node.container.height.as_ref(),
        PenNode::Rectangle(node) => node.container.height.as_ref(),
        PenNode::Ellipse(node) => node.height.as_ref(),
        PenNode::Polygon(node) => node.height.as_ref(),
        PenNode::Path(node) => node.height.as_ref(),
        PenNode::Text(node) => node.height.as_ref(),
        PenNode::TextInput(node) => node.height.as_ref(),
        PenNode::Image(node) => node.height.as_ref(),
        PenNode::IconFont(node) => node.height.as_ref(),
        PenNode::TextArea(node) => node.height.as_ref(),
        PenNode::Select(node) => node.height.as_ref(),
        PenNode::Switch(node) => node.height.as_ref(),
        PenNode::Checkbox(node) => node.height.as_ref(),
        PenNode::Slider(node) => node.height.as_ref(),
        PenNode::RadioGroup(node) => node.height.as_ref(),
        PenNode::NumberInput(node) => node.height.as_ref(),
        PenNode::Progress(node) => node.height.as_ref(),
        PenNode::Tabs(node) => node.height.as_ref(),
        PenNode::Line(_) | PenNode::Ref(_) => None,
    }
}

/// Pin a concrete width on a node regardless of where its schema keeps it.
pub(crate) fn set_node_width(node: &mut PenNode, width: f64) {
    if let Some(slot) = node_width_mut(node) {
        *slot = Some(SizingBehavior::Number(width));
    }
}

/// The node's main-axis size in CSS px when it is a plain number.
pub(crate) fn numeric_width(node: &PenNode) -> Option<f64> {
    match node_width(node) {
        Some(SizingBehavior::Number(value)) if value.is_finite() && *value >= 0.0 => Some(*value),
        _ => None,
    }
}
