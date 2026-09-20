//! Image-node property dispatch for the web host.
//!
//! The state machine is shared with the native host in
//! `op_editor_core::host_image_panel_transitions`; what stays here is
//! the platform glue — popover-input selection drag, chrome-input blur,
//! the property-focus commit, and the layout-scene-backed hit-test.
//! Actual network / file IO is drained by the web shell outside this
//! dispatch layer.

use super::WidgetHost;
use op_editor_core::host_image_panel_transitions as image_ops;
use op_editor_ui::widgets::{PropertyPanel, TOP_BAR_HEIGHT};
use op_editor_ui::{Point2D, Rect};

impl WidgetHost {
    pub(in crate::widget_host) fn toggle_image_search_popover(&mut self) {
        let opening = !self.editor_state.editor_ui.image_panel.search_open;
        // Seed from the pre-blur selection.
        let seed = image_ops::selected_image_seed(&self.editor_state, false);
        self.clear_image_input_selection_drag();
        if opening {
            self.blur_text_inputs_on_blank_press();
        }
        image_ops::apply_image_search_toggle(&mut self.editor_state, opening, seed, self.now_ms);
        self.close_other_property_popovers_for_image();
    }

    pub(in crate::widget_host) fn toggle_image_generate_popover(&mut self) {
        let opening = !self.editor_state.editor_ui.image_panel.generate_open;
        let seed = image_ops::selected_image_seed(&self.editor_state, true);
        self.clear_image_input_selection_drag();
        if opening {
            self.blur_text_inputs_on_blank_press();
        }
        image_ops::apply_image_generate_toggle(&mut self.editor_state, opening, seed, self.now_ms);
        self.close_other_property_popovers_for_image();
    }

    /// Close the property pickers that would overlap the popovers.
    fn close_other_property_popovers_for_image(&mut self) {
        self.commit_image_tile_scale_focus_if_any();
        image_ops::close_other_property_popovers_for_image(&mut self.editor_state.editor_ui);
    }

    pub(in crate::widget_host) fn run_image_search(&mut self) {
        image_ops::run_image_search(&mut self.editor_state);
    }

    pub(in crate::widget_host) fn select_image_search_result(&mut self, index: usize) {
        let Some(url) = image_ops::image_search_result_url(&self.editor_state, index) else {
            return;
        };
        self.write_selected_image_src(&url);
        self.clear_image_input_selection_drag();
        self.editor_state.editor_ui.image_panel.close_popovers();
    }

    pub(in crate::widget_host) fn run_image_generate(&mut self) {
        image_ops::run_image_generate(&mut self.editor_state);
    }

    pub(in crate::widget_host) fn apply_generated_image(&mut self) {
        let Some(url) = image_ops::generated_preview_url(&self.editor_state) else {
            return;
        };
        self.write_selected_image_src(&url);
        self.clear_image_input_selection_drag();
        self.editor_state.editor_ui.image_panel.close_popovers();
    }

    pub(in crate::widget_host) fn retry_image_generate(&mut self) {
        image_ops::retry_image_generate(&mut self.editor_state);
    }

    pub(in crate::widget_host) fn open_image_gen_settings(&mut self) {
        self.clear_image_input_selection_drag();
        image_ops::open_image_gen_settings(&mut self.editor_state);
    }

    pub(in crate::widget_host) fn write_selected_image_src(&mut self, src: &str) {
        if image_ops::write_selected_image_src(&mut self.editor_state, src) {
            self.mark_dirty();
        }
    }

    pub(in crate::widget_host) fn apply_image_panel_text(&mut self, c: char) -> bool {
        let effect = image_ops::image_panel_text(&mut self.editor_state, c, self.now_ms);
        self.finish_image_panel_input(effect)
    }

    pub(in crate::widget_host) fn apply_image_panel_backspace(&mut self) -> bool {
        let effect = image_ops::image_panel_backspace(&mut self.editor_state, self.now_ms);
        self.finish_image_panel_input(effect)
    }

    pub(in crate::widget_host) fn apply_image_panel_delete(&mut self) -> bool {
        let effect = image_ops::image_panel_delete(&mut self.editor_state, self.now_ms);
        self.finish_image_panel_input(effect)
    }

    pub fn apply_image_panel_caret(&mut self, forward: bool, extend: bool) -> bool {
        let effect =
            image_ops::image_panel_caret(&mut self.editor_state, forward, extend, self.now_ms);
        self.finish_image_panel_input(effect)
    }

    pub fn apply_image_panel_edge(&mut self, end: bool, extend: bool) -> bool {
        let effect = image_ops::image_panel_edge(&mut self.editor_state, end, extend, self.now_ms);
        self.finish_image_panel_input(effect)
    }

    pub(in crate::widget_host) fn apply_image_panel_select_all(&mut self) -> bool {
        let effect = image_ops::image_panel_select_all(&mut self.editor_state, self.now_ms);
        self.finish_image_panel_input(effect)
    }

    /// Repaint when the shared routing changed a draft, and report
    /// whether the popover consumed the key.
    fn finish_image_panel_input(&mut self, effect: image_ops::ImageInputEffect) -> bool {
        if effect.changed {
            self.mark_dirty();
        }
        effect.consumed
    }

    pub(in crate::widget_host) fn apply_image_panel_send(&mut self) -> bool {
        if self.editor_state.editor_ui.image_panel.search_open {
            self.run_image_search();
            self.mark_dirty();
            return true;
        }
        self.editor_state.editor_ui.image_panel.generate_open
    }

    pub(in crate::widget_host) fn close_image_popovers_for_higher_overlay(&mut self) -> bool {
        self.clear_image_input_selection_drag();
        // Hoisted ahead of the shared close: the tile-scale commit runs
        // through host-owned variable/effect commits, and it touches
        // state disjoint from the popover flags below.
        if self.editor_state.editor_ui.image_fill_popover_open {
            self.commit_image_tile_scale_focus_if_any();
        }
        let changed = image_ops::close_image_popovers_for_higher_overlay(&mut self.editor_state);
        if changed {
            self.mark_dirty();
        }
        changed
    }

    pub(in crate::widget_host) fn dismiss_image_popovers_on_press(
        &mut self,
        x: f32,
        y: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> bool {
        use op_editor_ui::widgets::PropertyPanelAction as A;
        let panel_state = &self.editor_state.editor_ui.image_panel;
        if !panel_state.search_open && !panel_state.generate_open {
            return false;
        }
        self.refresh_layout_scene();
        if let Some(panel) = PropertyPanel::for_selection(&self.editor_state) {
            let rect = Rect {
                origin: Point2D::new(
                    viewport_width - self.editor_state.editor_ui.property_panel_width,
                    TOP_BAR_HEIGHT,
                ),
                size: Point2D::new(
                    self.editor_state.editor_ui.property_panel_width,
                    (viewport_height - TOP_BAR_HEIGHT).max(0.0),
                ),
            };
            let point = Point2D::new(x, y);
            if let Some(action) = panel.hit_test_action(rect, point) {
                if matches!(
                    action,
                    A::RunImageSearch
                        | A::SelectImageSearchResult(_)
                        | A::RunImageGenerate
                        | A::ApplyGeneratedImage
                        | A::RetryImageGenerate
                        | A::OpenImageGenSettings
                        | A::ToggleImageSearchPopover
                        | A::ToggleImageGeneratePopover
                ) {
                    self.apply_property_action(action);
                    return true;
                }
            }
            if let Some((kind, offset)) = self.image_popover_input_at(&panel, rect, point) {
                self.begin_image_input_selection_drag(kind, offset);
                return true;
            }
            if panel.image_popovers_contain(rect, point) {
                return true;
            }
        }
        self.clear_image_input_selection_drag();
        self.editor_state.editor_ui.image_panel.close_popovers();
        self.mark_dirty();
        true
    }
}
