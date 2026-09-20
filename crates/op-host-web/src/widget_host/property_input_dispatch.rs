//! Web property-input commit glue. The commit bodies and the matching
//! draft-seeding (`property_focus_initial`) both live in
//! `op_editor_ui::widgets::property_panel_commit`; what stays here is
//! the web-side ordering (variables-panel drafts first) plus
//! `mark_dirty`.

use super::super::WidgetHost;
use op_editor_core::PropertyFocus;
use op_editor_ui::widgets::property_panel_commit as commit;

impl WidgetHost {
    /// Commit the floating image-fill editor's numeric draft before an action
    /// hides or replaces that editor. Keeping this guard here gives every
    /// close path the same focus/draft cleanup without disturbing unrelated
    /// property inputs.
    pub(in crate::widget_host) fn commit_image_tile_scale_focus_if_any(&mut self) -> bool {
        if self.editor_state.ui.property_focus != Some(PropertyFocus::ImageTileScale) {
            return false;
        }
        self.commit_property_focus_if_any();
        true
    }

    pub(in crate::widget_host) fn commit_effect_param_focus_if_any(&mut self) {
        if commit::commit_effect_param_focus(&mut self.editor_state) {
            self.mark_dirty();
        }
    }

    pub(in crate::widget_host) fn commit_property_focus_if_any(&mut self) {
        self.commit_variables_panel_header_focus_if_any();
        self.commit_variable_row_focus_if_any();
        self.commit_effect_param_focus_if_any();
        if commit::commit_property_focus(&mut self.editor_state) {
            self.mark_dirty();
        }
    }
}
