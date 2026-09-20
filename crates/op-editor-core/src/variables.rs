//! Variables / themes mutators — ported from shell-core's
//! `document/variables.rs` (`VariableTable`), retargeted onto the
//! canonical document model.
//!
//! ## Model split (spec §5.2)
//!
//! shell-core's `VariableTable` mixed *persisted* data with
//! *transient* editor state. Here that split is explicit:
//!
//!   - **Persisted** — `EditorState.doc.variables`
//!     (`Option<BTreeMap<String, VariableDefinition>>`) +
//!     `EditorState.doc.themes` (`Option<BTreeMap<String,
//!     Vec<String>>>`, axis-name → ordered value list). They
//!     serialize with the `.op` file.
//!   - **Transient** — the active-theme selection lives on
//!     `EditorState.ui.variables.active_theme`. Rebuilt on load,
//!     never serialized.
//!
//! Theme-routing discipline for themed values is ported verbatim:
//! a write targets the subset-matching entry, else the `theme: None`
//! default when no theme axis is active, else a fresh entry keyed to
//! the active theme appended at the END of the vec (front insertion
//! would shadow pre-existing entries on other axes).
//!
//! ### Module layout
//!
//! This file is the spine: the module docs plus the free helpers the
//! mutators share. The `impl EditorState` surface lives in sibling
//! submodules (per the 800-line-per-file ceiling); nothing about the
//! public API changes because inherent methods resolve regardless of
//! which file declares them:
//!
//! - `scalars` — lookup, colour bindings and the typed scalar setters
//! - `crud` — create / delete / rename / bulk + theme-axis selection

mod crud;
mod scalars;
#[cfg(test)]
mod tests;

use crate::fills::{
    first_solid_fill_hex, first_solid_stroke_hex, set_primary_fill_hex, set_primary_stroke_hex,
};
use crate::state::EditorState;
use crate::ui_draft::ColorTarget;
use crate::walkers::find_node_mut;
use jian_ops_schema::variable::{
    ThemedValue, VariableDefinition, VariableKind, VariableScalar, VariableValue,
};
use std::collections::BTreeMap;

// --- Free helpers ----------------------------------------------------

/// Resolve a `VariableValue` under the active theme — the canonical
/// equivalent of shell-core's `Variable::resolve`.
fn resolve_value<'a>(
    value: &'a VariableValue,
    active: &BTreeMap<String, String>,
) -> Option<&'a VariableScalar> {
    match value {
        VariableValue::Scalar(s) => Some(s),
        VariableValue::Themed(entries) => {
            for e in entries {
                if let Some(t) = &e.theme {
                    if t.iter().all(|(k, v)| active.get(k) == Some(v)) {
                        return Some(&e.value);
                    }
                }
            }
            // No themed match → the un-themed default entry → the
            // FIRST entry (TS `resolveThemedValue` falls back to
            // `values[0]`). Without the last step a fully-themed
            // value list — the shape the panel writes once a second
            // variant exists — resolves to nothing until the user
            // manually picks an axis value.
            entries
                .iter()
                .find(|e| e.theme.is_none())
                .or_else(|| entries.first())
                .map(|e| &e.value)
        }
    }
}

/// Write `scalar` into a `VariableValue` with the theme-routing
/// discipline ported from shell-core's `set_color_hex` /
/// `set_scalar`.
fn write_scalar(
    value: &mut VariableValue,
    scalar: VariableScalar,
    active: &BTreeMap<String, String>,
) {
    match value {
        VariableValue::Scalar(s) => *s = scalar,
        VariableValue::Themed(entries) => {
            let subset_idx = entries.iter().position(|e| match &e.theme {
                Some(t) => t.iter().all(|(k, v)| active.get(k) == Some(v)),
                None => false,
            });
            if let Some(i) = subset_idx {
                entries[i].value = scalar;
                return;
            }
            if active.is_empty() {
                if let Some(i) = entries.iter().position(|e| e.theme.is_none()) {
                    entries[i].value = scalar;
                } else {
                    entries.push(ThemedValue {
                        value: scalar,
                        theme: None,
                    });
                }
                return;
            }
            // Active theme set + no subset match — end-push a fresh
            // entry keyed to the active theme.
            entries.push(ThemedValue {
                value: scalar,
                theme: Some(active.clone()),
            });
        }
    }
}

fn write_scalar_for_theme(
    value: &mut VariableValue,
    scalar: VariableScalar,
    axis: &str,
    theme_value: &str,
    theme_values: &[String],
) {
    match value {
        VariableValue::Scalar(current) => {
            let fallback = current.clone();
            let themed = theme_values
                .iter()
                .map(|value_name| {
                    let mut theme = BTreeMap::new();
                    theme.insert(axis.to_string(), value_name.clone());
                    ThemedValue {
                        value: if value_name == theme_value {
                            scalar.clone()
                        } else {
                            fallback.clone()
                        },
                        theme: Some(theme),
                    }
                })
                .collect();
            *value = VariableValue::Themed(themed);
        }
        VariableValue::Themed(entries) => {
            if let Some(entry) = entries.iter_mut().find(|entry| {
                entry
                    .theme
                    .as_ref()
                    .and_then(|theme| theme.get(axis))
                    .is_some_and(|value| value == theme_value)
            }) {
                entry.value = scalar;
                return;
            }
            let mut theme = BTreeMap::new();
            theme.insert(axis.to_string(), theme_value.to_string());
            entries.push(ThemedValue {
                value: scalar,
                theme: Some(theme),
            });
        }
    }
}

fn normalize_variable_ref_name(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    let name = trimmed.strip_prefix('$').unwrap_or(trimmed);
    (!name.is_empty()).then_some(name)
}
