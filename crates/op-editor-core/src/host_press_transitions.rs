//! Press-path chrome transitions shared verbatim by the native and
//! web widget hosts (their `widget_host/press.rs` twins used to carry
//! these as copy-pasted inline blocks). Everything here touches only
//! `EditorState` / `EditorUiState`; hosts keep the platform glue
//! (`mark_dirty`, scene caches, viewport fits) in their thin wrappers.

use crate::editor_ui_state::EditorUiState;
use crate::state::EditorState;

// ─── Toolbar shape-slot dropdown ───────────────────────────────────────

/// Toggle the toolbar's shape-picker dropdown. Opening resets the
/// scroll so the popup always starts at its first row.
pub fn toggle_shape_picker(ui: &mut EditorUiState) {
    let picker = &mut ui.shape_picker;
    picker.open = !picker.open;
    picker.hover = None;
    picker.pressed = None;
    if picker.open {
        picker.scroll.offset = 0.0;
    }
}

/// Close the shape picker and drop its transient hover/press state.
pub fn close_shape_picker(ui: &mut EditorUiState) {
    ui.shape_picker.open = false;
    ui.shape_picker.hover = None;
    ui.shape_picker.pressed = None;
}

// ─── TopBar dropdowns ──────────────────────────────────────────────────

/// Toggle the TopBar locale dropdown (same open/reset shape as the
/// shape picker).
pub fn toggle_locale_picker(ui: &mut EditorUiState) {
    let opened = {
        let picker = &mut ui.locale_picker;
        picker.open = !picker.open;
        picker.hover = None;
        picker.pressed = None;
        if picker.open {
            picker.scroll.offset = 0.0;
        }
        picker.open
    };
    if opened {
        close_export_quick_menu(ui);
    }
}

/// Close the locale dropdown (row pick + outside dismiss share this).
pub fn close_locale_picker(ui: &mut EditorUiState) {
    ui.locale_picker.open = false;
    ui.locale_picker.hover = None;
}

/// Toggle the TopBar import dropdown — a second click on the button
/// closes the menu rather than re-opening it under the cursor.
pub fn toggle_import_menu(ui: &mut EditorUiState) {
    let open = !ui.import_menu_open;
    ui.import_menu_open = open;
    ui.import_menu.open = open;
    ui.import_menu.hover = None;
    if open {
        close_export_quick_menu(ui);
    }
}

/// Toggle the TopBar file menu. The host still clears the layer-panel
/// hover afterwards (that wash lives in host geometry state).
pub fn toggle_file_menu(ui: &mut EditorUiState) {
    ui.file_menu_open ^= true;
    ui.file_menu.hover = None;
    if ui.file_menu_open {
        close_export_quick_menu(ui);
    }
}

/// Toggle the TopBar export quick menu. Opening it closes the other
/// TopBar dropdowns: they share one overlay tier, so two open menus would
/// leave the lower one painted but unreachable.
pub fn toggle_export_quick_menu(ui: &mut EditorUiState) {
    let open = !ui.export_quick_menu_open;
    ui.export_quick_menu_open = open;
    ui.export_quick_menu_hover = None;
    if open {
        ui.file_menu_open = false;
        ui.file_menu.hover = None;
        ui.import_menu_open = false;
        ui.import_menu.open = false;
        ui.import_menu.hover = None;
        ui.account_menu_open = false;
        close_locale_picker(ui);
    }
}

/// Close the export quick menu (row pick + outside dismiss share this).
pub fn close_export_quick_menu(ui: &mut EditorUiState) {
    ui.export_quick_menu_open = false;
    ui.export_quick_menu_hover = None;
}

// ─── Canvas ────────────────────────────────────────────────────────────

/// Blank-canvas press with the Select tool: clear the selection and
/// step out of the entered container (the clearing-exits rule). No-op
/// while `shift_held` — a shift press starts an additive marquee.
///
/// Returns `true` when something visible changed (selection dropped or
/// container scope exited), which the host reports as its press result.
pub fn clear_selection_on_empty_canvas_press(state: &mut EditorState, shift_held: bool) -> bool {
    if shift_held {
        return false;
    }
    let was_set = !state.selection.set.is_empty();
    let had_scope = state.editor_ui.entered_container.is_some();
    if was_set {
        state.clear_selection();
    }
    // Clicking blank canvas steps out of the entered container
    // (clearing-exits rule).
    state.sync_entered_container_with_selection();
    let exited_scope = had_scope && state.editor_ui.entered_container.is_none();
    was_set || exited_scope
}

// ─── History guard ─────────────────────────────────────────────────────

/// Snapshot, run `f`, push the snapshot when `f` reports a change.
/// Mirrors each host's `with_doc_history` method so shared press
/// dispatchers can guard document mutations without a host callback.
pub fn with_doc_history(state: &mut EditorState, f: impl FnOnce(&mut EditorState) -> bool) -> bool {
    let snap = state.snapshot_for_history();
    let changed = f(state);
    if changed {
        state.history_push_past(snap);
    }
    changed
}
