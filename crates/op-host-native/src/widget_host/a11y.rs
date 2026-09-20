//! Accessibility region tree for the native host (#67).
//!
//! The region set / ordering / focus rules and the action routing live in
//! the platform-free `op_editor_ui::accessibility_regions` (shared with
//! the web host's `a11y_bridge.rs`); the tree shape / node-id mapping
//! lives one layer below in `op_editor_ui::accessibility`. This file is
//! only the native geometry hand-off: it reuses the very same
//! `canvas_region` / `*_rect` helpers paint uses, so the a11y tree and
//! the painted frame never drift.

use super::helpers::{TOOLBAR_INSET_X, TOOLBAR_INSET_Y};
use super::WidgetHostNative;
use op_editor_ui::accessibility_regions::{self as a11y_regions, RegionPlacement};

impl WidgetHostNative {
    /// Assemble the accessibility tree for the current editor frame.
    ///
    /// Hosts call this on the same cadence they paint (initial publish +
    /// every dirty frame); the assembler suppresses no-op events on the
    /// adapter side.
    ///
    /// Takes `&mut self` because the canvas region reads the
    /// layout-resolved scene, which `refresh_layout_scene` lazily rebuilds
    /// when the editor state is dirty — same contract as `paint`.
    pub fn accessibility_tree_update(
        &mut self,
        viewport_width: f32,
        viewport_height: f32,
    ) -> accesskit::TreeUpdate {
        // Keep the canvas scene in sync with editor state (cheap no-op
        // when not dirty) so the CanvasViewport widget is consistent
        // with what paint draws.
        self.refresh_layout_scene();

        let (canvas_left, _canvas_y, canvas_w, canvas_h) =
            self.canvas_region(viewport_width, viewport_height);
        let placement = RegionPlacement {
            viewport_width,
            viewport_height,
            canvas_left,
            canvas_width: canvas_w,
            canvas_height: canvas_h,
            ai_chat_rect: self.ai_chat_rect(viewport_width, viewport_height),
            status_bar_rect: self.status_bar_rect(viewport_width, viewport_height),
            toolbar_inset_x: TOOLBAR_INSET_X,
            toolbar_inset_y: TOOLBAR_INSET_Y,
        };
        let layer_panel = self.layer_panel();
        a11y_regions::editor_tree_update(
            &self.editor_state,
            &self.layout_scene,
            &layer_panel,
            self.now_ms,
            placement,
        )
    }

    /// Route an accesskit action targeting a known editor region back
    /// into host state. Returns `true` when the action changed state (so
    /// the runner repaints + re-publishes the tree).
    ///
    /// `target` is the raw `accesskit::NodeId.0` (== `WidgetId.0`), and
    /// `is_focus` distinguishes a `Focus` request from a `Click` /
    /// `Default` activation.
    pub fn apply_a11y_action(&mut self, target: u64, is_focus: bool) -> bool {
        if a11y_regions::apply_region_action(&mut self.editor_state, target, is_focus, self.now_ms)
        {
            self.mark_editor_state_dirty();
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_editor_ui::accessibility::node_id;
    use op_editor_ui::accessibility_regions::{AI_CHAT_WIDGET_ID, CANVAS_WIDGET_ID};
    use op_editor_ui::widgets::{WidgetId, ROOT_WIDGET_ID};

    fn host() -> WidgetHostNative {
        WidgetHostNative::new()
    }

    #[test]
    fn tree_includes_always_present_regions() {
        let mut h = host();
        let update = h.accessibility_tree_update(1280.0, 800.0);
        // Root + at least: top bar, layer panel, canvas, toolbar,
        // chat, status bar (property panel only with a selection).
        let ids: Vec<_> = update.nodes.iter().map(|(id, _)| *id).collect();
        assert!(ids.contains(&node_id(ROOT_WIDGET_ID)));
        assert!(ids.contains(&node_id(WidgetId::new(5000))), "top bar");
        assert!(ids.contains(&node_id(WidgetId::new(4000))), "canvas");
        assert!(ids.contains(&node_id(WidgetId::new(7000))), "chat");
        // Root advertises every emitted child.
        let (_, root) = &update.nodes[0];
        for child in root.children() {
            assert!(
                update.nodes.iter().any(|(id, _)| id == child),
                "root child {child:?} missing a node"
            );
        }
    }

    #[test]
    fn focus_defaults_to_canvas() {
        let mut h = host();
        let update = h.accessibility_tree_update(1280.0, 800.0);
        assert_eq!(update.focus, node_id(WidgetId::new(CANVAS_WIDGET_ID)));
    }

    #[test]
    fn focused_chat_input_takes_focus() {
        let mut h = host();
        h.editor_state_mut().chat.focused = true;
        let update = h.accessibility_tree_update(1280.0, 800.0);
        assert_eq!(update.focus, node_id(WidgetId::new(AI_CHAT_WIDGET_ID)));
    }

    #[test]
    fn a11y_action_on_chat_focuses_input() {
        let mut h = host();
        h.set_now_ms(1234);
        let changed = h.apply_a11y_action(AI_CHAT_WIDGET_ID, true);
        assert!(changed);
        assert!(h.editor_state().chat.focused);
    }

    #[test]
    fn a11y_focus_on_canvas_blurs_chat() {
        let mut h = host();
        h.editor_state_mut().chat.focused = true;
        let changed = h.apply_a11y_action(CANVAS_WIDGET_ID, true);
        assert!(changed);
        assert!(!h.editor_state().chat.focused);
    }

    #[test]
    fn a11y_action_on_unknown_region_is_noop() {
        let mut h = host();
        assert!(!h.apply_a11y_action(99999, true));
    }

    #[test]
    fn collapsed_sidebar_drops_layer_panel_region() {
        let mut h = host();
        h.editor_state_mut().editor_ui.sidebar_open = false;
        let update = h.accessibility_tree_update(1280.0, 800.0);
        let ids: Vec<_> = update.nodes.iter().map(|(id, _)| *id).collect();
        assert!(
            !ids.contains(&node_id(WidgetId::new(1000))),
            "layer panel hidden"
        );
    }
}
