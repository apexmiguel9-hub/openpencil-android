//! Property-rail hover clearing + the immediate-frame predicate the
//! cursor-move coalescer consults.
//!
//! Split out of `cursor_input.rs` to keep every file under the repo's
//! 800-line cap.

use super::WidgetHost;

impl WidgetHost {
    // Cursor-move coalescing hint — tested + ready to wire; the CanvasKit mount
    // repaints every mousemove rather than scheduling deferred frames.
    #[allow(dead_code)]
    pub(crate) fn cursor_move_requires_immediate_frame(&self) -> bool {
        let color_picker_drag = self
            .editor_state
            .ui
            .color_picker
            .as_ref()
            .and_then(|state| state.drag)
            .is_some();
        self.variables_resize.is_some()
            || color_picker_drag
            || self.design_md_drag.is_some()
            || self.component_browser_drag.is_some()
            || self.icon_picker_drag.is_some()
            || self.code_selection_drag.is_some()
            || self.image_input_selection_drag.is_some()
            || self.chat_input_selection_drag.is_some()
            || self.chat_text_selection_drag.is_some()
            || self.create_drag.is_some()
            || self.path_anchor_drag.is_some()
            || self.handle_drag.is_some()
            || self.image_crop_drag.is_some()
            || self.node_drag.is_some()
            || self.marquee_drag.is_some()
            || self.layer_drag.is_some()
            || self.chat_drag.is_some()
            || self.image_adjustment_drag.is_some()
            || self.effect_radius_drag.is_some()
            || self.drag.is_some()
    }

    pub(in crate::widget_host) fn clear_hover_below_property_panel(&mut self) -> bool {
        let mut changed = false;
        {
            let ui = &mut self.editor_state.editor_ui;
            changed |= ui.canvas_hover_node.take().is_some();
            changed |= ui.hovered_layer_id.take().is_some();
            changed |= ui.hovered_page_index.take().is_some();
            changed |= ui.toolbar_hover.take().is_some();
            changed |= ui.align_toolbar_hover.take().is_some();
            changed |= ui.statusbar_hover.take().is_some();
            changed |= ui.chat_design_block_hover.take().is_some();
            changed |= ui.chat_footer_hover.take().is_some();
            changed |= ui.clear_chat_style_chip_hover();
            changed |= ui.chat_example_hover.take().is_some();
            changed |= ui.chat_tab_hover.take().is_some();
            if let Some(menu) = ui.layer_context_menu.as_mut() {
                changed |= menu.menu.hover.take().is_some();
            }
        }
        if let Some(menu) = self.editor_state.ui.path_anchor_menu.as_mut() {
            changed |= menu.menu.hover.take().is_some();
        }
        if changed {
            self.mark_dirty();
        }
        changed
    }
}

impl WidgetHost {
    /// Fill / stroke colour-variable popup hover tier of
    /// `apply_cursor_move`. Same press order as
    /// `press_property_overlay_tiers`: this popup sits above the padding
    /// and font popovers below it, so it gets first refusal on the point.
    ///
    /// `Some(consumed)` ends the move; `None` falls through to the next
    /// popover.
    pub(in crate::widget_host) fn color_variable_picker_hover_tier(
        &mut self,
        panel: &op_editor_ui::widgets::PropertyPanel,
        property_rect: op_editor_ui::Rect,
        point: op_editor_ui::Point2D,
        chat_or_picker_surface_owns_point: bool,
        upper_hover_changed: &mut bool,
    ) -> Option<bool> {
        let (over_popup, hover_changed) =
            op_editor_ui::widgets::cursor_hover_flow::color_variable_picker_hover(
                &mut self.editor_state,
                Some(panel),
                property_rect,
                point,
            );
        if hover_changed {
            self.mark_dirty();
        }
        if over_popup {
            self.clear_chat_and_lower_hover();
            return Some(true);
        }
        if hover_changed {
            if chat_or_picker_surface_owns_point {
                *upper_hover_changed = true;
            } else {
                return Some(true);
            }
        }
        None
    }
}
