//! VariablesPanel press-arm transitions shared by the native and web
//! widget hosts.
//!
//! Their `widget_host/variables_panel_press.rs` twins used to carry every
//! arm body twice (theme / variant CRUD, the three dropdown toggles, the
//! add-variable seeding, the axis bookkeeping). Everything here touches
//! only `EditorState`, so the hosts keep just the hit-test dispatch, the
//! panel-resize host field, the row-cell delegates, and `mark_dirty()`.
//!
//! Two drifts were resolved on the way in (both were equivalent-but-
//! costlier readings on the native side):
//!
//! * the axis/variable lookups read `ui.variables.active_theme` and
//!   `doc.variables` directly instead of building a whole
//!   `op_pen_loader` variable table — `editor_state_var_table` clones
//!   `ui.variables.active_theme` verbatim and emits `variables` in
//!   `doc.variables` order, so the results are identical while the
//!   direct reads skip a full-document walk on a press path;
//! * every menu-opening arm now folds the row menu away (the web host
//!   already did), matching `close_variable_menus` and keeping two
//!   menus from painting at once.

use std::collections::BTreeMap;

use jian_ops_schema::variable::{VariableKind, VariableScalar};

use crate::editor_ui_state::{EditorUiState, VariableRowFocus};
use crate::host_keyboard_transitions::{
    mirror_variable_row_input_legacy, mirror_variables_header_input_legacy,
};
use crate::EditorState;

// ─── Menus ─────────────────────────────────────────────────────────────

/// Fold away every VariablesPanel dropdown / row menu.
pub fn close_variable_menus(ui: &mut EditorUiState) {
    ui.axis_dropdown_open = None;
    ui.variables_add_menu_open = false;
    ui.variables_preset_menu_open = false;
    ui.variables_theme_menu_axis = None;
    ui.variables_variant_menu_value = None;
    ui.variables_row_menu = None;
}

/// Toggle the theme-preset dropdown, closing every sibling menu.
pub fn toggle_preset_menu(ui: &mut EditorUiState) {
    ui.variables_preset_menu_open = !ui.variables_preset_menu_open;
    ui.variables_add_menu_open = false;
    ui.axis_dropdown_open = None;
    ui.variables_theme_menu_axis = None;
    ui.variables_variant_menu_value = None;
    ui.variables_row_menu = None;
}

/// Toggle the "+ variable" kind dropdown, closing every sibling menu.
pub fn toggle_add_variable_menu(ui: &mut EditorUiState) {
    ui.variables_add_menu_open = !ui.variables_add_menu_open;
    ui.variables_preset_menu_open = false;
    ui.axis_dropdown_open = None;
    ui.variables_theme_menu_axis = None;
    ui.variables_variant_menu_value = None;
    ui.variables_row_menu = None;
}

/// Toggle the per-axis header menu (rename / delete).
pub fn toggle_theme_menu(ui: &mut EditorUiState, axis: String) {
    ui.variables_theme_menu_axis = if ui.variables_theme_menu_axis.as_deref() == Some(&axis) {
        None
    } else {
        Some(axis)
    };
    ui.variables_variant_menu_value = None;
    ui.variables_add_menu_open = false;
    ui.variables_preset_menu_open = false;
    ui.variables_row_menu = None;
    ui.axis_dropdown_open = None;
}

/// Toggle the per-variant column menu (rename / delete).
pub fn toggle_variant_menu(ui: &mut EditorUiState, value: String) {
    ui.variables_variant_menu_value =
        if ui.variables_variant_menu_value.as_deref() == Some(value.as_str()) {
            None
        } else {
            Some(value)
        };
    ui.variables_theme_menu_axis = None;
    ui.variables_add_menu_open = false;
    ui.variables_preset_menu_open = false;
    ui.variables_row_menu = None;
    ui.axis_dropdown_open = None;
}

/// Toggle the active-value dropdown of the `idx`-th axis chip.
pub fn toggle_variable_axis(state: &mut EditorState, idx: usize) {
    let axis = state.ui.variables.active_theme.keys().nth(idx).cloned();
    if let Some(name) = axis {
        let ui = &mut state.editor_ui;
        ui.axis_dropdown_open = if ui.axis_dropdown_open.as_deref() == Some(name.as_str()) {
            None
        } else {
            Some(name)
        };
        ui.variables_add_menu_open = false;
        ui.variables_preset_menu_open = false;
        ui.variables_row_menu = None;
    }
}

/// Pick an axis value from an open axis dropdown, guarded by history.
pub fn select_axis_value(state: &mut EditorState, axis: &str, value: &str) {
    let snap = state.snapshot_for_history();
    if state.set_active_axis_value(axis, value) {
        state.history_push_past(snap);
    }
    close_variable_menus(&mut state.editor_ui);
}

// ─── Header renames ────────────────────────────────────────────────────

/// Seed the header input with the axis name and enter rename mode.
pub fn start_theme_rename(state: &mut EditorState, axis: String, now_ms: u64) {
    state
        .editor_ui
        .variables_header_input
        .set_text(axis.clone());
    state.editor_ui.variables_header_input.touch(now_ms);
    mirror_variables_header_input_legacy(state, false, now_ms);
    let ui = &mut state.editor_ui;
    ui.variables_theme_rename_axis = Some(axis);
    ui.variables_theme_menu_axis = None;
    ui.variables_variant_menu_value = None;
}

/// Seed the header input with the variant value and enter rename mode.
pub fn start_variant_rename(state: &mut EditorState, value: String, now_ms: u64) {
    state
        .editor_ui
        .variables_header_input
        .set_text(value.clone());
    state.editor_ui.variables_header_input.touch(now_ms);
    mirror_variables_header_input_legacy(state, false, now_ms);
    let ui = &mut state.editor_ui;
    ui.variables_variant_rename_value = Some(value);
    ui.variables_variant_menu_value = None;
    ui.variables_theme_menu_axis = None;
}

// ─── Theme / variant CRUD ──────────────────────────────────────────────

/// Delete a whole theme axis. Refuses to drop the last axis (the panel
/// always needs one) and re-points the current axis when it vanished.
pub fn delete_theme_axis(state: &mut EditorState, axis: String) {
    let Some(themes) = state.doc.themes.as_ref() else {
        return;
    };
    if themes.len() <= 1 || !themes.contains_key(&axis) {
        close_variable_menus(&mut state.editor_ui);
        return;
    }
    let snap = state.snapshot_for_history();
    if let Some(themes) = state.doc.themes.as_mut() {
        themes.remove(&axis);
    }
    state.ui.variables.active_theme.remove(&axis);
    if state.editor_ui.variables_current_axis.as_deref() == Some(axis.as_str()) {
        state.editor_ui.variables_current_axis = state.doc.themes.as_ref().and_then(|themes| {
            themes.iter().next().map(|(next_axis, values)| {
                state.ui.variables.active_theme.insert(
                    next_axis.clone(),
                    values.first().cloned().unwrap_or_else(|| "Default".into()),
                );
                next_axis.clone()
            })
        });
    }
    state.history_push_past(snap);
    close_variable_menus(&mut state.editor_ui);
}

/// Delete one variant value from the current axis. Refuses to drop the
/// last value and re-points the active value when it vanished.
pub fn delete_variant_value(state: &mut EditorState, value: String) {
    let axis = ensure_variable_axis(state);
    let Some(values) = state
        .doc
        .themes
        .as_ref()
        .and_then(|themes| themes.get(&axis))
    else {
        return;
    };
    if values.len() <= 1 || !values.iter().any(|v| v == &value) {
        close_variable_menus(&mut state.editor_ui);
        return;
    }
    let snap = state.snapshot_for_history();
    if let Some(values) = state
        .doc
        .themes
        .as_mut()
        .and_then(|themes| themes.get_mut(&axis))
    {
        values.retain(|v| v != &value);
        if state
            .ui
            .variables
            .active_theme
            .get(&axis)
            .is_some_and(|active| active == &value)
        {
            if let Some(next) = values.first().cloned() {
                state.ui.variables.active_theme.insert(axis, next);
            }
        }
    }
    state.history_push_past(snap);
    close_variable_menus(&mut state.editor_ui);
}

/// Mint a fresh theme axis (with a single `Default` value) and make it
/// current.
pub fn add_variable_theme(state: &mut EditorState) {
    let snap = state.snapshot_for_history();
    let themes = state.doc.themes.get_or_insert_with(BTreeMap::new);
    let name = unique_numbered("Theme", |candidate| themes.contains_key(candidate));
    themes.insert(name.clone(), vec!["Default".into()]);
    let current_axis = name.clone();
    state
        .ui
        .variables
        .active_theme
        .insert(name, "Default".into());
    state.editor_ui.variables_current_axis = Some(current_axis);
    state.history_push_past(snap);
    close_variable_menus(&mut state.editor_ui);
}

/// Make `axis` the current one, seeding its active value if unset.
pub fn select_variable_axis(state: &mut EditorState, axis: String) {
    let Some(values) = state
        .doc
        .themes
        .as_ref()
        .and_then(|themes| themes.get(&axis))
    else {
        return;
    };
    let fallback = values.first().cloned().unwrap_or_else(|| "Default".into());
    state
        .ui
        .variables
        .active_theme
        .entry(axis.clone())
        .or_insert(fallback);
    state.editor_ui.variables_current_axis = Some(axis);
    close_variable_menus(&mut state.editor_ui);
}

/// Append a fresh variant column to the current axis.
pub fn add_variable_variant(state: &mut EditorState) {
    let snap = state.snapshot_for_history();
    let axis = ensure_variable_axis(state);
    let Some(values) = state
        .doc
        .themes
        .as_mut()
        .and_then(|themes| themes.get_mut(&axis))
    else {
        return;
    };
    let variant = unique_numbered("Variant", |candidate| values.iter().any(|v| v == candidate));
    values.push(variant);
    state.history_push_past(snap);
    close_variable_menus(&mut state.editor_ui);
}

// ─── Add variable ──────────────────────────────────────────────────────

/// Create a variable of `kind` with `default`, then focus its first
/// editable cell so the user can type over the seeded value (Number /
/// String only — Color opens its own swatch editor).
pub fn add_variable(
    state: &mut EditorState,
    base: &str,
    kind: VariableKind,
    default: VariableScalar,
    now_ms: u64,
) {
    let snap = state.snapshot_for_history();
    ensure_variable_axis(state);
    let name = unique_numbered(base, |candidate| {
        state
            .doc
            .variables
            .as_ref()
            .is_some_and(|vars| vars.contains_key(candidate))
    });
    let default_focus = match (&kind, &default) {
        (VariableKind::Number, VariableScalar::Num(value)) => Some((
            VariableRowFocus::NumberCell { row: 0, variant: 0 },
            format!("{value}"),
            true,
        )),
        (VariableKind::String, VariableScalar::Str(value)) => Some((
            VariableRowFocus::StringCell { row: 0, variant: 0 },
            value.clone(),
            !value.is_empty(),
        )),
        _ => None,
    };
    if state.create_variable(&name, kind, default) {
        state.history_push_past(snap);
        if let Some((focus, draft, select_all)) = default_focus {
            // Row index = position in `doc.variables` (BTreeMap order),
            // which is the order the panel paints its rows in.
            let row = state
                .doc
                .variables
                .as_ref()
                .and_then(|vars| vars.keys().position(|key| key == &name))
                .unwrap_or(0);
            state.editor_ui.variable_row_input.set_text(draft);
            if select_all {
                state.editor_ui.variable_row_input.select_all();
            }
            state.editor_ui.variable_row_input.touch(now_ms);
            mirror_variable_row_input_legacy(state, select_all, now_ms);
            state.editor_ui.variable_row_focus = Some(match focus {
                VariableRowFocus::NumberCell { variant, .. } => {
                    VariableRowFocus::NumberCell { row, variant }
                }
                VariableRowFocus::StringCell { variant, .. } => {
                    VariableRowFocus::StringCell { row, variant }
                }
                other => other,
            });
        }
    }
    close_variable_menus(&mut state.editor_ui);
}

// ─── Axis resolution ───────────────────────────────────────────────────

/// Resolve the axis every write path needs: the current one when it
/// still exists, else the first known axis, else a freshly minted one.
pub fn ensure_variable_axis(state: &mut EditorState) -> String {
    if let Some(axis) = state
        .editor_ui
        .variables_current_axis
        .as_ref()
        .filter(|axis| {
            state
                .doc
                .themes
                .as_ref()
                .is_some_and(|themes| themes.contains_key(*axis))
        })
        .cloned()
    {
        return axis;
    }
    if let Some(axis) = state.ui.variables.active_theme.keys().next().cloned() {
        return axis;
    }
    if let Some((axis, values)) = state
        .doc
        .themes
        .as_ref()
        .and_then(|themes| themes.iter().next())
    {
        let value = values.first().cloned().unwrap_or_else(|| "Default".into());
        state.ui.variables.active_theme.insert(axis.clone(), value);
        state.editor_ui.variables_current_axis = Some(axis.clone());
        return axis.clone();
    }
    let themes = state.doc.themes.get_or_insert_with(BTreeMap::new);
    let axis = unique_numbered("Theme", |candidate| themes.contains_key(candidate));
    themes.insert(axis.clone(), vec!["Default".into()]);
    state
        .ui
        .variables
        .active_theme
        .insert(axis.clone(), "Default".into());
    state.editor_ui.variables_current_axis = Some(axis.clone());
    axis
}

/// `base-N` with the lowest free `N`.
pub fn unique_numbered(base: &str, exists: impl Fn(&str) -> bool) -> String {
    // Bounded scan — 10k collisions on one base never happens in practice,
    // but a pathological document must not spin the UI thread forever.
    for idx in 1u32..=10_000 {
        let candidate = format!("{base}-{idx}");
        if !exists(&candidate) {
            return candidate;
        }
    }
    // Deterministic fallback past the bound; a duplicate name is cosmetic
    // while an unbounded loop would hang the editor.
    format!("{base}-10001")
}
