//! Thin wrappers over the shared chat model-picker filter-input
//! transitions (`op_editor_core::host_ui_transitions`).

use op_editor_core::host_ui_transitions as shared;

use super::WidgetHost;

impl WidgetHost {
    pub(in crate::widget_host) fn apply_chat_model_picker_text(&mut self, c: char) -> bool {
        if shared::chat_model_picker_text(&mut self.editor_state.editor_ui, c, self.now_ms) {
            self.mark_dirty();
            return true;
        }
        false
    }

    pub(in crate::widget_host) fn apply_chat_model_picker_backspace(&mut self) -> bool {
        if shared::chat_model_picker_backspace(&mut self.editor_state.editor_ui, self.now_ms) {
            self.mark_dirty();
            return true;
        }
        false
    }

    pub fn apply_chat_model_picker_caret(&mut self, forward: bool) -> bool {
        if shared::chat_model_picker_caret(&mut self.editor_state.editor_ui, forward, self.now_ms) {
            self.mark_dirty();
            return true;
        }
        false
    }
}
