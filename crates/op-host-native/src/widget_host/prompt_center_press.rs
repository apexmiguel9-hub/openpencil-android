//! Thin native dispatch arm for the shared Prompt Center press flow.

use op_editor_ui::widgets::press_flow;
use op_editor_ui::Point2D;

use super::WidgetHostNative;

impl WidgetHostNative {
    pub(in crate::widget_host) fn dispatch_prompt_center_press(
        &mut self,
        x: f32,
        y: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> bool {
        let Some(panel_rect) = self.prompt_center_panel_rect(viewport_width, viewport_height)
        else {
            return false;
        };
        let Some(changed) = press_flow::press_prompt_center(
            &mut self.editor_state,
            panel_rect,
            Point2D::new(x, y),
            self.now_ms,
            epoch_millis(),
        ) else {
            return false;
        };
        if changed {
            self.mark_dirty();
        }
        true
    }
}

fn epoch_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}
