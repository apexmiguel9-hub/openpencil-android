//! Variable + theme-axis write tools (`set_variable_color` /
//! `set_active_axis_value`) — carved off `write_tools.rs` to keep both
//! files under the 800-line cap. Re-exported from `write_tools` so the
//! public paths stay unchanged.

use std::collections::BTreeMap;

use jian_ops_schema::variable::VariableKind;
use op_editor_core::EditorState;

use super::write_tools::validate_hex;
use super::{EditorCommand, McpTool, ToolErrorCode, ToolOutcome};

/// First-party `set_variable_color` tool — validates that the variable
/// exists + is Color-kind + the hex parses, then returns
/// `OkWithCommand(SetVariableColor)`.
pub struct SetVariableColor {
    /// Snapshot of which Color variables exist. Validation only.
    pub known_colors: BTreeMap<String, ()>,
}

impl McpTool for SetVariableColor {
    fn name(&self) -> &str {
        "set_variable_color"
    }
    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let Some(name) = args.get("name") else {
            return ToolOutcome::Err(ToolErrorCode::MissingArgument, "name is required".into());
        };
        let Some(hex) = args.get("hex") else {
            return ToolOutcome::Err(ToolErrorCode::MissingArgument, "hex is required".into());
        };
        if !self.known_colors.contains_key(name) {
            return ToolOutcome::Err(
                ToolErrorCode::ToolFailed,
                format!("variable {name:?} not found or not Color-kind"),
            );
        }
        if !validate_hex(hex) {
            return ToolOutcome::Err(
                ToolErrorCode::InvalidArgument,
                format!("hex must be #rgb/#rrggbb/#rrggbbaa, got {hex:?}"),
            );
        }
        let mut out = BTreeMap::new();
        out.insert("wrote".into(), "true".into());
        ToolOutcome::OkWithCommand(
            out,
            EditorCommand::SetVariableColor {
                name: name.clone(),
                hex: hex.clone(),
            },
        )
    }
}

/// First-party `set_active_axis_value` tool — pins an axis to a value.
pub struct SetActiveAxisValue {
    /// Snapshot of axis → allowed-values. Validation only.
    pub axes: BTreeMap<String, Vec<String>>,
}

impl McpTool for SetActiveAxisValue {
    fn name(&self) -> &str {
        "set_active_axis_value"
    }
    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let Some(axis) = args.get("axis") else {
            return ToolOutcome::Err(ToolErrorCode::MissingArgument, "axis is required".into());
        };
        let Some(value) = args.get("value") else {
            return ToolOutcome::Err(ToolErrorCode::MissingArgument, "value is required".into());
        };
        let Some(allowed) = self.axes.get(axis) else {
            return ToolOutcome::Err(
                ToolErrorCode::ToolFailed,
                format!("axis {axis:?} not defined in themes"),
            );
        };
        if !allowed.iter().any(|v| v == value) {
            return ToolOutcome::Err(
                ToolErrorCode::InvalidArgument,
                format!(
                    "value {value:?} not in axis {axis:?}; allowed: {}",
                    allowed.join(", ")
                ),
            );
        }
        let mut out = BTreeMap::new();
        out.insert("wrote".into(), "true".into());
        ToolOutcome::OkWithCommand(
            out,
            EditorCommand::SetActiveAxisValue {
                axis: axis.clone(),
                value: value.clone(),
            },
        )
    }
}
pub fn set_active_axis_value_snapshot(state: &EditorState) -> SetActiveAxisValue {
    let axes = state
        .doc
        .themes
        .as_ref()
        .map(|themes| {
            themes
                .iter()
                .map(|(name, values)| (name.clone(), values.clone()))
                .collect()
        })
        .unwrap_or_default();
    SetActiveAxisValue { axes }
}

pub fn set_variable_color_snapshot(state: &EditorState) -> SetVariableColor {
    let known_colors = state
        .doc
        .variables
        .as_ref()
        .map(|vars| {
            vars.iter()
                .filter(|(_, def)| matches!(def.kind, VariableKind::Color))
                .map(|(name, _)| (name.clone(), ()))
                .collect()
        })
        .unwrap_or_default();
    SetVariableColor { known_colors }
}
