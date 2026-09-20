//! Hover bookkeeping on the web `WidgetHost` — LayerPanel row hover plus
//! the "clear everything under this overlay" ladder that keeps a floating
//! panel from lighting up the chrome beneath it.
//!
//! Split out of the `widget_host.rs` spine to keep it under the repo's
//! 800-line cap.

use super::*;

impl WidgetHost {
    /// Update `editor_ui.hovered_layer_id` from the cursor.
    /// Returns true if hover state changed (caller should
    /// repaint). Mirrors the native host.
    pub fn update_layer_hover(&mut self, x: f32, y: f32, viewport_h: f32) -> bool {
        use op_editor_ui::widgets::{LayerPanelHit, TOP_BAR_HEIGHT};
        let sidebar_open = self.editor_state.editor_ui.sidebar_open;
        let panel_w = self.editor_state.editor_ui.layer_panel_width;
        let blocked_by_overlay = self
            .chat_model_picker_rect(self.last_viewport_w, viewport_h)
            .is_some()
            || self.over_topmost_panel(x, y, self.last_viewport_w, viewport_h)
            || self.over_dropdown_overlay(x, y, self.last_viewport_w, viewport_h);
        let (new_layer, new_page) = if sidebar_open
            && !blocked_by_overlay
            && y >= TOP_BAR_HEIGHT
            && x >= 0.0
            && x <= panel_w
        {
            self.refresh_layout_scene();
            let layer_rect = self.layer_panel_rect(viewport_h);
            let panel = self.layer_panel();
            match panel.hit_test(layer_rect, Point2D::new(x, y)) {
                Some(LayerPanelHit::Layer(id))
                | Some(LayerPanelHit::ToggleHidden(id))
                | Some(LayerPanelHit::ToggleLocked(id))
                | Some(LayerPanelHit::ToggleCollapsed(id)) => (Some(id), None),
                Some(LayerPanelHit::Page(idx)) | Some(LayerPanelHit::DeletePage(idx)) => {
                    (None, Some(idx))
                }
                _ => (None, None),
            }
        } else {
            (None, None)
        };
        // shell-core hit-test returns shell-core `NodeId`s; translate
        // to op-editor-core ids for storage on `editor_ui`.
        let new_layer_ec = new_layer.clone();
        let changed = new_layer_ec != self.editor_state.editor_ui.hovered_layer_id
            || new_page != self.editor_state.editor_ui.hovered_page_index;
        if changed {
            self.editor_state.editor_ui.hovered_layer_id = new_layer_ec;
            self.editor_state.editor_ui.hovered_page_index = new_page;
            self.mark_dirty();
        }
        changed
    }

    pub(in crate::widget_host) fn clear_layer_panel_hover(&mut self) -> bool {
        let ui = &mut self.editor_state.editor_ui;
        let cleared_layer = ui.hovered_layer_id.take().is_some();
        let cleared_page = ui.hovered_page_index.take().is_some();
        let changed = cleared_layer || cleared_page;
        if changed {
            self.mark_dirty();
        }
        changed
    }

    pub(in crate::widget_host) fn clear_hover_below_chat_model_picker(&mut self) -> bool {
        self.clear_lower_overlay_hover_impl(false)
    }

    pub(in crate::widget_host) fn clear_hover_below_topmost_panel(&mut self) -> bool {
        self.clear_lower_overlay_hover_impl(true)
    }

    /// Clear hover feedback below the collaboration popover while preserving
    /// the popover's own active control.
    pub(in crate::widget_host) fn clear_hover_below_collab_panel(&mut self) -> bool {
        let mut changed = false;
        {
            let ui = &mut self.editor_state.editor_ui;
            changed |= ui.canvas_hover_node.take().is_some();
            changed |= ui.hovered_layer_id.take().is_some();
            changed |= ui.hovered_page_index.take().is_some();
            changed |= ui.file_menu.hover.take().is_some();
            changed |= ui.export_quick_menu_hover.take().is_some();
            changed |= ui.locale_picker.hover.take().is_some();
            changed |= ui.shape_picker.hover.take().is_some();
            changed |= ui.fill_type_picker.hover.take().is_some();
            changed |= ui.toolbar_hover.take().is_some();
            changed |= ui.align_toolbar_hover.take().is_some();
            changed |= ui.statusbar_hover.take().is_some();
            changed |= ui.topbar_button_hover.take().is_some();
            changed |= ui.chat_model_picker.hover.take().is_some();
            changed |= ui.chat_header_hover.take().is_some();
            changed |= ui.chat_tab_hover.take().is_some();
            changed |= ui.chat_design_block_hover.take().is_some();
            changed |= ui.chat_footer_hover.take().is_some();
            changed |= ui.clear_chat_style_chip_hover();
            changed |= ui.chat_example_hover.take().is_some();
            changed |= ui.parallel_agents_picker_hover.take().is_some();
            changed |= ui.export_picker_hover.take().is_some();
            changed |= ui.variables_panel_hover.take().is_some();
            changed |= ui.variables_preset_menu_hover.take().is_some();
            changed |= ui.property_action_hover.take().is_some();
            changed |= ui.property_tab_hover.take().is_some();
        }
        changed |= self.editor_state.codegen.framework_hover.take().is_some();
        changed |= self.editor_state.codegen.action_hover.take().is_some();
        if changed {
            self.mark_dirty();
        }
        changed
    }

    /// Clear hover state for surfaces painted below the regular Chat panel.
    /// Chat's own hover state and the higher Status/Align/overlay tiers are
    /// deliberately preserved.
    pub(in crate::widget_host) fn clear_hover_below_chat_panel(&mut self) -> bool {
        let mut changed = false;
        {
            let ui = &mut self.editor_state.editor_ui;
            changed |= ui.canvas_hover_node.take().is_some();
            changed |= ui.hovered_layer_id.take().is_some();
            changed |= ui.hovered_page_index.take().is_some();
            changed |= ui.toolbar_hover.take().is_some();
            changed |= ui.variables_panel_hover.take().is_some();
            changed |= ui.variables_preset_menu_hover.take().is_some();
            changed |= ui.property_action_hover.take().is_some();
            changed |= ui.property_tab_hover.take().is_some();
            changed |= ui.fill_type_picker.hover.take().is_some();
        }
        changed |= self.editor_state.codegen.framework_hover.take().is_some();
        changed |= self.editor_state.codegen.action_hover.take().is_some();
        if changed {
            self.mark_dirty();
        }
        changed
    }

    /// Clear the regular Chat surface and everything painted below it while
    /// preserving the currently owning higher overlay's own hover state.
    pub(in crate::widget_host) fn clear_chat_and_lower_hover(&mut self) -> bool {
        let mut changed = false;
        {
            let ui = &mut self.editor_state.editor_ui;
            changed |= ui.collab.panel.hover.take().is_some();
            changed |= ui.chat_model_picker.hover.take().is_some();
            changed |= ui.chat_header_hover.take().is_some();
            changed |= ui.chat_tab_hover.take().is_some();
            changed |= ui.chat_design_block_hover.take().is_some();
            changed |= ui.chat_footer_hover.take().is_some();
            changed |= ui.clear_chat_style_chip_hover();
            changed |= ui.chat_example_hover.take().is_some();
            changed |= ui.parallel_agents_picker_hover.take().is_some();
            changed |= ui.canvas_hover_node.take().is_some();
            changed |= ui.hovered_layer_id.take().is_some();
            changed |= ui.hovered_page_index.take().is_some();
            changed |= ui.toolbar_hover.take().is_some();
            changed |= ui.variables_panel_hover.take().is_some();
            changed |= ui.variables_preset_menu_hover.take().is_some();
            changed |= ui.property_action_hover.take().is_some();
            changed |= ui.property_tab_hover.take().is_some();
        }
        changed |= self.editor_state.codegen.framework_hover.take().is_some();
        changed |= self.editor_state.codegen.action_hover.take().is_some();
        if changed {
            self.mark_dirty();
        }
        changed
    }

    fn clear_lower_overlay_hover_impl(&mut self, clear_chat_model_picker: bool) -> bool {
        let mut changed = false;
        {
            let ui = &mut self.editor_state.editor_ui;
            changed |= ui.canvas_hover_node.take().is_some();
            changed |= ui.hovered_layer_id.take().is_some();
            changed |= ui.hovered_page_index.take().is_some();
            changed |= ui.file_menu.hover.take().is_some();
            changed |= ui.export_quick_menu_hover.take().is_some();
            changed |= ui.locale_picker.hover.take().is_some();
            changed |= ui.shape_picker.hover.take().is_some();
            changed |= ui.fill_type_picker.hover.take().is_some();
            changed |= ui.toolbar_hover.take().is_some();
            changed |= ui.align_toolbar_hover.take().is_some();
            changed |= ui.statusbar_hover.take().is_some();
            changed |= ui.topbar_button_hover.take().is_some();
            changed |= ui.collab.panel.hover.take().is_some();
            if clear_chat_model_picker {
                changed |= ui.chat_model_picker.hover.take().is_some();
            }
            changed |= ui.chat_header_hover.take().is_some();
            changed |= ui.chat_tab_hover.take().is_some();
            changed |= ui.chat_design_block_hover.take().is_some();
            changed |= ui.chat_footer_hover.take().is_some();
            changed |= ui.clear_chat_style_chip_hover();
            changed |= ui.chat_example_hover.take().is_some();
            changed |= ui.parallel_agents_picker_hover.take().is_some();
            changed |= ui.export_picker_hover.take().is_some();
            changed |= ui.variables_panel_hover.take().is_some();
            changed |= ui.variables_preset_menu_hover.take().is_some();
            changed |= ui.property_action_hover.take().is_some();
            changed |= ui.property_tab_hover.take().is_some();
            if let Some(menu) = ui.layer_context_menu.as_mut() {
                changed |= menu.menu.hover.take().is_some();
            }
        }
        if let Some(menu) = self.editor_state.ui.path_anchor_menu.as_mut() {
            changed |= menu.menu.hover.take().is_some();
        }
        changed |= self.editor_state.codegen.framework_hover.take().is_some();
        changed |= self.editor_state.codegen.action_hover.take().is_some();
        if changed {
            self.mark_dirty();
        }
        changed
    }

    // Cursor-move dispatch (`apply_cursor_move` /
    // `update_agent_settings_hover`) lives in
    // `widget_host/cursor_input.rs`; mouse-release handling
    // (`apply_release[_with_viewport]` / `commit_marquee_selection` /
    // `commit_layer_drag`) in `widget_host/release_input.rs` — both
    // split out to keep this spine file under the 800-line ceiling.

    // Keyboard / clipboard handlers (`apply_text` / `apply_backspace`
    // / `apply_send` / `apply_delete` / `apply_duplicate` /
    // `apply_nudge` / `apply_select_all` / `apply_copy` /
    // `apply_cut` / `apply_paste` / `apply_reorder` /
    // `apply_escape` / `apply_ime` / `apply_key`) live in
    // `widget_host/keyboard.rs` — split out to keep this spine
    // file under the 800-line ceiling.

    // `paint` lives in `widget_host/paint.rs` — split out to keep
    // this file under the 800-line ceiling.
}
