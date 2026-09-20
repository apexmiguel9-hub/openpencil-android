//! `apply_cursor_move` tiers 7-8 and the final base tier — TopBar hover,
//! the single-shot chat cursor probe, and the toolbar / property / canvas
//! hover fall-through.
//!
//! `cursor_move_chat_hover` is deliberately NOT a consuming tier: it
//! resolves the chat panel exactly once, aggregates every chat-owned hover
//! write, and reports whether the chat owns the point. The late pointer
//! drags run between it and the base tier, which is why the ownership
//! answer is returned instead of acted on immediately.

use super::cursor_move_ctx::CursorMoveCtx;
use super::WidgetHostNative;
use op_editor_ui::widgets::cursor_hover_flow as hover_flow;
use op_editor_ui::widgets::{AIChatPlaceholder, PropertyPanel};
use op_editor_ui::{Point2D, Rect};

impl WidgetHostNative {
    /// TopBar traffic-light cluster + chrome-button hover wash.
    /// `None` — the TopBar did not consume the move.
    pub(in crate::widget_host) fn cursor_move_topbar_tiers(
        &mut self,
        ctx: &mut CursorMoveCtx,
    ) -> Option<bool> {
        let (x, y) = (ctx.x, ctx.y);
        let chat_surface_owns_point = ctx.chat_surface_owns_point;
        // TopBar window-control cluster — hovering it reveals the
        // close / minimise / maximise glyphs on the 3 dots.
        {
            use op_editor_ui::widgets::{TopBar, TOP_BAR_HEIGHT};
            let tb_rect = Rect {
                origin: Point2D::new(0.0, 0.0),
                size: Point2D::new(self.last_viewport_w, TOP_BAR_HEIGHT),
            };
            let over = !chat_surface_owns_point
                && (TopBar::traffic_cluster_rect(tb_rect)).contains(Point2D::new(x, y));
            if over != self.editor_state.editor_ui.topbar_traffic_hover {
                self.editor_state.editor_ui.topbar_traffic_hover = over;
                self.mark_dirty();
                if chat_surface_owns_point {
                    ctx.upper_hover_changed = true;
                } else {
                    return Some(true);
                }
            }
        }
        // TopBar chrome-button hover wash (sidebar / file-menu / figma /
        // theme / locale / fullscreen / git / agent chip). Reuses the
        // click hit-test so paint + hover can never drift.
        {
            use op_editor_ui::widgets::{TopBar, TOP_BAR_HEIGHT};
            let tb_rect = Rect {
                origin: Point2D::new(0.0, 0.0),
                size: Point2D::new(self.last_viewport_w, TOP_BAR_HEIGHT),
            };
            let mut top_bar = TopBar::for_editor_ui(&self.editor_state.editor_ui);
            top_bar.chip_text_w = Some(self.topbar_chip_text_w(&top_bar));
            let new_hover = (!chat_surface_owns_point)
                .then(|| self.topbar_hit_test(&top_bar, tb_rect, Point2D::new(x, y)))
                .flatten()
                .map(op_editor_ui::widgets::editor_state_ext::topbar_button_hover);
            let ui = &mut self.editor_state.editor_ui;
            if ui.set_topbar_button_hover(new_hover, self.now_ms) {
                self.mark_dirty();
                if chat_surface_owns_point {
                    ctx.upper_hover_changed = true;
                } else {
                    return Some(true);
                }
            }
        }
        None
    }

    /// Resolve the chat panel once and apply every chat-owned hover write.
    /// Returns whether the chat owns the point (authoritative — true for
    /// the painted body and the invisible resize gutter alike).
    pub(in crate::widget_host) fn cursor_move_chat_hover(
        &mut self,
        ctx: &mut CursorMoveCtx,
    ) -> bool {
        let (x, y) = (ctx.x, ctx.y);
        let over_topmost = ctx.over_topmost;
        // Construct the chat panel once for this cursor event and resolve all
        // of its hover results in one immutable scope. Besides keeping the
        // transcript fingerprint to one pass, this avoids cloning translated
        // labels and tab titles again for every chat sub-control.
        let (
            chat_probe,
            chat_tab_hover,
            chat_footer_hover,
            parallel_hover,
            example_hover,
            style_chip_hover,
        ) = if let Some(chat_rect) = self.ai_chat_rect(self.last_viewport_w, self.last_viewport_h) {
            let point = Point2D::new(x, y);
            let panel = AIChatPlaceholder::from_editor_at(&self.editor_state, self.now_ms)
                .owned_by(self.chat_panel_owner);
            (
                Some(panel.cursor_probe(chat_rect, point)),
                panel.tab_hover_at(chat_rect, point),
                panel.footer_hover_at(chat_rect, point),
                panel.parallel_agents_picker_hover_at(chat_rect, point),
                panel.example_hover_at(chat_rect, point),
                panel.style_chip_hover_at(chat_rect, point),
            )
        } else {
            (None, None, None, None, None, false)
        };
        // `cursor_probe.hit` is the authoritative Chat ownership result: it is
        // Some for every painted body point and for the invisible resize
        // gutter. Aggregate all Chat-owned hover writes before returning so a
        // changed control can never leave a stale canvas highlight behind.
        let chat_owns_point = chat_probe.as_ref().is_some_and(|probe| probe.hit.is_some());
        let mut chat_hover_changed = false;
        let new_header_hover = chat_probe
            .as_ref()
            .and_then(|probe| probe.hit.as_ref())
            .and_then(op_editor_ui::widgets::editor_state_ext::chat_header_hover);
        if new_header_hover != self.editor_state.editor_ui.chat_header_hover {
            self.editor_state.editor_ui.chat_header_hover = new_header_hover;
            chat_hover_changed = true;
        }
        // AI chat tab row hover — drives the close-× visibility on each tab.
        if chat_tab_hover != self.editor_state.editor_ui.chat_tab_hover {
            self.editor_state.editor_ui.chat_tab_hover = chat_tab_hover;
            chat_hover_changed = true;
        }
        if chat_footer_hover != self.editor_state.editor_ui.chat_footer_hover {
            self.editor_state.editor_ui.chat_footer_hover = chat_footer_hover;
            chat_hover_changed = true;
        }
        // Pinned-style chip — a dwell clock rather than a wash, so it goes
        // through the setter that owns the clock's start / stop rule.
        chat_hover_changed |= self
            .editor_state
            .editor_ui
            .set_chat_style_chip_hover(style_chip_hover, self.now_ms);
        // Parallel-agents picker row hover — drives the highlight wash inside the overlay.
        if parallel_hover != self.editor_state.editor_ui.parallel_agents_picker_hover {
            self.editor_state.editor_ui.parallel_agents_picker_hover = parallel_hover;
            chat_hover_changed = true;
        }
        if example_hover != self.editor_state.editor_ui.chat_example_hover {
            self.editor_state.editor_ui.chat_example_hover = example_hover;
            chat_hover_changed = true;
        }
        // Design-block hover — reuse the combined probe resolved above (gated on
        // `over_topmost` exactly as the old dedicated pass was).
        let design_hover = if over_topmost {
            None
        } else {
            chat_probe
                .as_ref()
                .and_then(|probe| probe.design_block_hover)
        };
        chat_hover_changed |= self.apply_chat_design_hover(design_hover);
        if chat_hover_changed {
            self.mark_dirty();
            ctx.upper_hover_changed = true;
        }
        chat_owns_point
    }

    /// Final tier — chat ownership, toolbar hover, property / code hover,
    /// then canvas-hierarchy hover. Returns the repaint signal for the
    /// whole event.
    pub(in crate::widget_host) fn cursor_move_base_tiers(
        &mut self,
        ctx: &mut CursorMoveCtx,
        chat_owns_point: bool,
    ) -> bool {
        let (x, y) = (ctx.x, ctx.y);
        let point = ctx.point;
        let property_rect = ctx.property_rect;
        let over_topmost = ctx.over_topmost;
        if chat_owns_point {
            let lower_changed = self.clear_hover_below_chat_panel();
            return ctx.upper_hover_changed || lower_changed;
        }
        // Toolbar hover after drag detection.
        if self.update_toolbar_hover(x, y, over_topmost) {
            return true;
        }
        // PropertyPanel tab/action hover wash. Shown with a selection.
        let mut property_hover_changed = false;
        let needs_property_probe = !over_topmost
            && self.editor_state.property_panel_visible()
            && (property_rect.contains(point)
                || self.editor_state.editor_ui.fill_type_picker.open
                || self.editor_state.editor_ui.compositing_picker.open);
        if needs_property_probe && ctx.property_panel_probe.is_none() {
            ctx.property_panel_probe = Some(PropertyPanel::for_selection(&self.editor_state));
        }
        let property_panel = if needs_property_probe {
            ctx.property_panel_probe.as_ref().and_then(Option::as_ref)
        } else {
            None
        };
        property_hover_changed |= hover_flow::property_base_hover(
            &mut self.editor_state,
            property_panel,
            property_rect,
            point,
        );
        // Code-panel hover wash. Reuses Code-panel action geometry so
        // framework chips, scroll chevrons, and body buttons share click and
        // hover hit-testing.
        if hover_flow::code_panel_hover(&mut self.editor_state, property_rect, point, !over_topmost)
        {
            self.mark_dirty();
            return true;
        }
        if property_hover_changed {
            self.mark_dirty();
            return true;
        }
        // Canvas hierarchy hover: resolve the current level's focus
        // from the root-to-deepest scene path. Shared paint outlines
        // the focus solid and all direct children dashed. Reads the
        // CURRENT layout scene without refreshing (same discipline as
        // layer-row hover — hover must not rebuild a stale scene).
        let hover_eligible = !over_topmost
            && matches!(self.editor_state.tool, op_editor_core::Tool::Select)
            && self.over_canvas(x, y, self.last_viewport_w, self.last_viewport_h);
        let new_canvas_hover = if hover_eligible {
            // Skip the (full-tree) hover hit-test for sub-3px jitter —
            // the outline can't visibly change inside that radius and
            // path-heavy documents pay real cost per walk. The skip
            // only ever bypasses the WALK; leaving the canvas (the
            // else branch) always clears, threshold or not.
            if let Some((hx, hy)) = self.last_hover_probe {
                if (x - hx).abs() < 3.0 && (y - hy).abs() < 3.0 {
                    return ctx.cleared || ctx.upper_hover_changed;
                }
            }
            self.last_hover_probe = Some((x, y));
            let (cx0, cy0, cw, ch) = self.canvas_region(self.last_viewport_w, self.last_viewport_h);
            let canvas_rect = Rect {
                origin: Point2D::new(cx0, cy0),
                size: Point2D::new(cw, ch),
            };
            hover_flow::canvas_hover_target(
                &self.editor_state,
                &self.layout_scene,
                canvas_rect,
                Point2D::new(x, y),
            )
        } else {
            self.last_hover_probe = None;
            None
        };
        if new_canvas_hover != self.editor_state.editor_ui.canvas_hover_node {
            self.editor_state.editor_ui.canvas_hover_node = new_canvas_hover;
            self.mark_dirty();
            return true;
        }
        // Fold stale-hover clearing into the repaint signal.
        ctx.cleared || ctx.upper_hover_changed
    }
}
