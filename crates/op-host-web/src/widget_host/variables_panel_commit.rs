//! VariablesPanel draft commits on the web host — theme/variant
//! header renames and the per-row cell drafts (Number / String /
//! inline Color hex).
//!
//! The whole walk lives in the shared
//! `op_editor_core::host_variables_commit` (the native twin drives the
//! same functions); this file is only the `mark_dirty()` tail plus the
//! keyboard-ownership predicate the web keyboard paths ask for.

use op_editor_core::host_variables_commit as vars_commit;

use super::WidgetHost;

impl WidgetHost {
    /// Commit any pending VariablesPanel theme/variant header rename.
    pub(in crate::widget_host) fn commit_variables_panel_header_focus_if_any(&mut self) {
        if vars_commit::commit_header_focus(&mut self.editor_state) {
            self.mark_dirty();
        }
    }

    pub(in crate::widget_host) fn variable_axis_value_for_variant(
        &self,
        variant: usize,
    ) -> Option<(String, String)> {
        vars_commit::variable_axis_value_for_variant(&self.editor_state, variant)
    }

    /// Commit any pending VariablesPanel row edit (Name / Number /
    /// String / inline Color hex).
    pub(in crate::widget_host) fn commit_variable_row_focus_if_any(&mut self) {
        if vars_commit::commit_row_focus(&mut self.editor_state) {
            self.mark_dirty();
        }
    }

    /// Whether the variables-panel search input owns the keyboard.
    pub(in crate::widget_host) fn variables_search_active(&self) -> bool {
        self.editor_state.editor_ui.variables_search_input_active()
    }
}
