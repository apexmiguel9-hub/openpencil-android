//! PropertyPanel compositing-select glue (web mirror of the native host). The picker transitions live
//! in `op_editor_core::host_ui_transitions`; the widget-facade target
//! mapping + the action dispatch that opens the picker live in
//! `op_editor_ui::widgets::property_panel_dispatch`. What remains here
//! is the close path the keyboard / press families call by name.

use super::super::WidgetHost;

impl WidgetHost {
    pub(in crate::widget_host) fn close_compositing_picker(&mut self) {
        self.editor_state.editor_ui.close_compositing_picker();
    }
}
