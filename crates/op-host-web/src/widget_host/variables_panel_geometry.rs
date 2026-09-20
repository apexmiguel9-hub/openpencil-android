//! Floating VariablesPanel geometry on the web host — thin over the
//! shared `op_editor_ui::widgets::variables_panel_geometry_flow` (the
//! native twin drives the same placement + clamping), so the browser
//! build floats the same interactive panel next to the toolbar.

use super::{WidgetHost, TOOLBAR_INSET_X, TOOLBAR_INSET_Y};
use op_editor_ui::widgets::variables_panel_geometry_flow as vars_geometry;
use op_editor_ui::Rect;

impl WidgetHost {
    pub(in crate::widget_host) fn variables_panel_rect(
        &self,
        viewport_w: f32,
        viewport_h: f32,
    ) -> Option<Rect> {
        vars_geometry::variables_panel_rect(
            &self.editor_state,
            self.canvas_region(viewport_w, viewport_h),
            TOOLBAR_INSET_X,
            TOOLBAR_INSET_Y,
        )
    }

    /// Apply an in-flight resize drag: write the new size from the
    /// cursor position (the panel is anchored top-left, so width /
    /// height derive directly from the cursor minus the origin).
    pub(in crate::widget_host) fn apply_variables_panel_resize(
        &mut self,
        x: f32,
        y: f32,
        viewport_w: f32,
        viewport_h: f32,
    ) -> bool {
        let Some(edge) = self.variables_resize else {
            return false;
        };
        let Some(rect) = self.variables_panel_rect(viewport_w, viewport_h) else {
            return false;
        };
        if vars_geometry::resize_from_cursor(&mut self.editor_state, edge, rect, x, y) {
            self.mark_dirty();
        }
        true
    }
}
