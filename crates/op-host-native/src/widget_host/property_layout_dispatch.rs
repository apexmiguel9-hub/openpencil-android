//! Layout-scene-backed size resolution for the native property panel.
//!
//! The layout / sizing / typography writers themselves are shared with
//! the web host in `op_editor_ui::widgets::property_panel_layout_ops`;
//! only this axis probe needs the host's resolved `LayoutScene`.

use super::WidgetHostNative;

impl WidgetHostNative {
    pub(in crate::widget_host) fn resolved_selected_sizing_axis(
        &mut self,
        width: bool,
    ) -> Option<f64> {
        let id = self.editor_state.selection.anchor.clone();
        if !id.is_real() {
            return None;
        }
        self.refresh_layout_scene();
        self.layout_scene
            .active_page()
            .and_then(|page| page.find(id.as_str()))
            .map(|node| node.aggregate_bounds())
            .map(|bounds| {
                if width {
                    f64::from(bounds.size.x)
                } else {
                    f64::from(bounds.size.y)
                }
            })
            .filter(|value| value.is_finite() && *value >= 0.0)
    }

    /// Resize the selection to the intrinsic ratio of the image it
    /// paints, keeping its current width. The resolved canvas width is
    /// read first so a Fill / Hug node matches what the user sees.
    pub(in crate::widget_host) fn match_selected_image_aspect_ratio(&mut self) {
        let Some(width) = self.resolved_selected_sizing_axis(true) else {
            return;
        };
        let Some(source) = self
            .editor_state
            .selected_node()
            .and_then(op_editor_ui::widgets::property_panel_image_ratio::node_image_source_size)
        else {
            return;
        };
        let _ = self
            .editor_state
            .match_selected_aspect_ratio(source, width as f32);
    }
}
