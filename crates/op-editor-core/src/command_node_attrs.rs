//! Per-node attribute command application — `SetNodeRotation` /
//! `SetNodeText` / `SetNodeCornerRadius` / `SetNodeFontSize` /
//! `SetNodeFontWeight` / `SetNodeStrokeHex` / `SetNodeStrokeWidth` /
//! `SetNodeFillHex` / `SetNodeName` / `SetNodeFlag`.
//!
//! Ported from shell-core's `mcp_apply_node_attrs.rs`, retargeted onto
//! the canonical `jian_ops_schema::PenNode`. shell-core's flat `Node`
//! carried a single `fill` / `stroke` / `corner_radius`; `PenNode`
//! spreads those across per-variant fields, so these helpers route
//! through [`crate::fills`] + per-variant matches.
//!
//! Each helper keeps the validate-then-mutate discipline: kind / range
//! / hex checks happen BEFORE the mutable borrow + write.
//!
//! This file is the slim spine; the implementation lives in sibling
//! modules under `command_node_attrs/` so every
//! `crate::command_node_attrs::…` import path keeps working:
//!
//! | File                              | Purpose                              |
//! | --------------------------------- | ------------------------------------ |
//! | `command_node_attrs/slots.rs`       | corner-radius / effects / stroke slots |
//! | `command_node_attrs/widget_props.rs`| widget text / number / bindings writers |
//! | `command_node_attrs/state_ops.rs`   | `impl EditorState` command methods   |

mod slots;
mod state_ops;
mod widget_props;

/// Which string-typed widget prop a `SetNodeWidgetText` edit targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetTextField {
    /// `placeholder` (TextInput / TextArea / NumberInput / Select).
    Placeholder,
    /// String-typed `value` (Select / RadioGroup) or text-widget
    /// `value` (TextInput / TextArea).
    Value,
    /// `label` (Checkbox).
    Label,
    /// `leadingIcon` lucide glyph name (TextInput / TextArea /
    /// NumberInput). Empty draft clears it.
    LeadingIcon,
    /// `trailingIcon` lucide glyph name (TextInput / TextArea /
    /// NumberInput). Empty draft clears it.
    TrailingIcon,
}

/// Which numeric widget prop a `SetNodeWidgetNumber` edit targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetNumberField {
    /// `min` (Slider / NumberInput).
    Min,
    /// `max` (Slider / NumberInput / Progress).
    Max,
    /// `step` (Slider / NumberInput).
    Step,
    /// Numeric `value` (Slider / NumberInput / Progress).
    Value,
}
