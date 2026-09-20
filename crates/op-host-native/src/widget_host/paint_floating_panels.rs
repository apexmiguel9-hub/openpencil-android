//! Floating panel paint band for the native host.

use super::WidgetHostNative;
use crate::backend::NativeFrameBackend;
use op_editor_ui::widgets::{
    ComponentBrowserPanel, DesignMdPanel, IconPickerPanel, PaintCx, PromptCenterPanel,
    SceneTemplatePanel, SCENE_TEMPLATE_SCRIM,
};
use op_editor_ui::RenderBackend;

impl WidgetHostNative {
    pub(super) fn paint_floating_panels(
        &mut self,
        frame: &mut NativeFrameBackend<'_>,
        viewport_width: f32,
        viewport_height: f32,
    ) {
        if let (Some(panel), Some(rect)) = (
            ComponentBrowserPanel::for_editor_at(&self.editor_state, self.now_ms),
            self.component_browser_panel_rect(viewport_width, viewport_height),
        ) {
            panel.paint(
                &mut PaintCx {
                    backend: &mut *frame,
                },
                rect,
            );
        }

        if let (Some(panel), Some(rect)) = (
            PromptCenterPanel::for_editor_at(&self.editor_state, self.now_ms),
            self.prompt_center_panel_rect(viewport_width, viewport_height),
        ) {
            panel.paint(
                &mut PaintCx {
                    backend: &mut *frame,
                },
                rect,
            );
        }

        // Asset Center gallery — a dimming scrim across the whole viewport,
        // then the panel over it. The scrim is what turns a floating dialog
        // into an immersive gallery, and it is also the surface that takes a
        // dismiss press (routed in `scene_template_press.rs`), so paint and
        // hit-test must agree that it covers everything.
        if let (Some(panel), Some(rect)) = (
            SceneTemplatePanel::for_editor_at(&self.editor_state, self.now_ms),
            self.scene_template_panel_rect(viewport_width, viewport_height),
        ) {
            if let Some(scrim) = self.scene_template_scrim_rect(viewport_width, viewport_height) {
                frame.fill_rect(scrim, SCENE_TEMPLATE_SCRIM);
            }
            panel.paint(
                &mut PaintCx {
                    backend: &mut *frame,
                },
                rect,
            );
        }

        if let (Some(panel), Some(rect)) = (
            IconPickerPanel::for_editor_at(&self.editor_state, self.now_ms),
            self.icon_picker_panel_rect(viewport_width, viewport_height),
        ) {
            panel.paint(
                &mut PaintCx {
                    backend: &mut *frame,
                },
                rect,
            );
        }

        if let (Some(panel), Some(rect)) = (
            DesignMdPanel::for_editor(&self.editor_state),
            self.design_md_panel_rect(viewport_width, viewport_height),
        ) {
            panel.paint(
                &mut PaintCx {
                    backend: &mut *frame,
                },
                rect,
            );
        }
    }
}
