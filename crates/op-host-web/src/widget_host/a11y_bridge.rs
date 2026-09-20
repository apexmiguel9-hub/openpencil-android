//! Host-side bridge for the hidden accessibility DOM mirror (#57).
//!
//! Two responsibilities, both thin over the shared
//! `op_editor_ui::accessibility_regions` (which the native host's
//! `widget_host/a11y.rs` also drives):
//!
//! 1. **Enumerate** the editor's always-present regions at the rects the
//!    web paint pass (`widget_host/paint.rs`) places them, and let the
//!    shared layer assemble the `accesskit::TreeUpdate`. The CanvasKit
//!    mount renders that update into the hidden DOM mirror
//!    (`crate::a11y_dom`) so screen readers can read the opaque canvas.
//! 2. **Route** incoming DOM accessibility events (focus / click on a
//!    mirror node) back into editor state — focusing the chat input,
//!    blurring it when focus moves to the canvas / a panel, or activating
//!    a tool. These keep the painted frame in lock-step with the screen
//!    reader's focus.

use super::WidgetHost;
use op_editor_ui::accessibility_regions::{self as a11y_regions, RegionPlacement};

use super::{TOOLBAR_INSET_X, TOOLBAR_INSET_Y};

impl WidgetHost {
    /// Assemble the accessibility tree for the current editor frame.
    ///
    /// Pairs each always-present region with the rect the web paint pass
    /// paints it at and hands the geometry to the shared assembler.
    ///
    /// Takes `&mut self` because the canvas region reads the
    /// layout-resolved scene, which `refresh_layout_scene` lazily rebuilds
    /// when editor state is dirty — same contract as `paint`.
    pub(crate) fn accessibility_tree_update(
        &mut self,
        viewport_width: f32,
        viewport_height: f32,
    ) -> accesskit::TreeUpdate {
        // Keep the canvas scene in sync with editor state (cheap no-op when
        // not dirty) so the CanvasViewport widget matches what paint draws.
        self.refresh_layout_scene();
        self.last_viewport_w = viewport_width;
        self.last_viewport_h = viewport_height;

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

    /// Route an accessibility action targeting a known editor region back
    /// into host state. Returns `true` when the action changed state (so
    /// the mount repaints + re-publishes the tree). Mirrors the native
    /// host's `apply_a11y_action`.
    ///
    /// `target` is the raw `accesskit::NodeId.0` (== `WidgetId.0`), and
    /// `is_focus` distinguishes a `focus` event from a `click` activation.
    pub(crate) fn apply_a11y_action(&mut self, target: u64, is_focus: bool) -> bool {
        // The a11y tree only emits the chat node when `ai_chat_rect` is
        // Some, but a stale assistive-tech target could still replay the
        // id, so gate here too — the VS Code embed's chat is MCP-driven
        // and must never accept keyboard focus. Web-only: `EmbedHost` is
        // never `VsCode` on the native host.
        if target == a11y_regions::AI_CHAT_WIDGET_ID
            && self.editor_state.editor_ui.embed == op_editor_core::EmbedHost::VsCode
        {
            return false;
        }
        if a11y_regions::apply_region_action(&mut self.editor_state, target, is_focus, self.now_ms)
        {
            self.mark_dirty();
            true
        } else {
            false
        }
    }

    /// Activate a tool from the hidden a11y toolbar — mirrors the painted
    /// toolbar's `ToolbarHit::Tool` arm (tool write + shape-picker close).
    /// Retained for a future per-tool mirror node set; the v1 mirror only
    /// surfaces the toolbar as a single region (so this has no caller yet —
    /// it is parity surface mirrored from the native host + exercised by the
    /// tests below).
    #[allow(dead_code)]
    pub(crate) fn a11y_set_tool(&mut self, tool: op_editor_core::Tool) {
        a11y_regions::set_tool(&mut self.editor_state, tool);
        self.mark_dirty();
    }

    /// Focus the chat input from the hidden a11y mirror — mirrors
    /// `widget_host/click.rs` `AIChatHit::FocusInput` (focus + clear stale
    /// selections), plus the caret-blink anchor reset so the painted caret
    /// restarts its phase like a real click. Callers should `set_now_ms`
    /// first so the anchor is current.
    ///
    /// The chat action itself routes through the shared
    /// `apply_region_action`, so this is a mirror-facing entry point with
    /// no in-crate caller outside the tests below (same shape as
    /// [`Self::a11y_set_tool`]).
    #[allow(dead_code)]
    pub(crate) fn a11y_focus_chat_input(&mut self) {
        a11y_regions::focus_chat_input(&mut self.editor_state, self.now_ms);
        self.mark_dirty();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_editor_ui::accessibility::node_id;
    use op_editor_ui::accessibility_regions::{AI_CHAT_WIDGET_ID, CANVAS_WIDGET_ID};
    use op_editor_ui::widgets::{WidgetId, ROOT_WIDGET_ID};

    fn host() -> WidgetHost {
        WidgetHost::new()
    }

    #[test]
    fn tree_includes_always_present_regions() {
        let mut h = host();
        let update = h.accessibility_tree_update(1280.0, 800.0);
        let ids: Vec<_> = update.nodes.iter().map(|(id, _)| *id).collect();
        assert!(ids.contains(&node_id(ROOT_WIDGET_ID)));
        assert!(ids.contains(&node_id(WidgetId::new(5000))), "top bar");
        assert!(
            ids.contains(&node_id(WidgetId::new(CANVAS_WIDGET_ID))),
            "canvas"
        );
        assert!(
            ids.contains(&node_id(WidgetId::new(AI_CHAT_WIDGET_ID))),
            "chat"
        );
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
        h.editor_state.chat.focused = true;
        let update = h.accessibility_tree_update(1280.0, 800.0);
        assert_eq!(update.focus, node_id(WidgetId::new(AI_CHAT_WIDGET_ID)));
    }

    #[test]
    fn a11y_action_on_chat_focuses_input() {
        let mut host = host();
        host.set_now_ms(1234);
        let changed = host.apply_a11y_action(AI_CHAT_WIDGET_ID, true);
        assert!(changed);
        assert!(host.editor_state.chat.focused);
    }

    #[test]
    fn a11y_focus_on_canvas_blurs_chat() {
        let mut host = host();
        host.editor_state.chat.focused = true;
        let changed = host.apply_a11y_action(CANVAS_WIDGET_ID, true);
        assert!(changed);
        assert!(!host.editor_state.chat.focused);
    }

    #[test]
    fn a11y_action_on_unknown_region_is_noop() {
        let mut host = host();
        assert!(!host.apply_a11y_action(99999, true));
    }

    #[test]
    fn a11y_set_tool_switches_tool_and_closes_shape_picker() {
        let mut host = host();
        host.editor_state.editor_ui.shape_picker.open = true;
        host.a11y_set_tool(op_editor_core::Tool::Frame);
        assert_eq!(host.editor_state.tool, op_editor_core::Tool::Frame);
        assert!(!host.editor_state.editor_ui.shape_picker.open);
        assert!(host.editor_state_dirty);
    }

    #[test]
    fn a11y_focus_chat_input_focuses_and_clears_selections() {
        let mut host = host();
        host.set_now_ms(1234);
        host.editor_state.chat.set_input_text("hello");
        host.editor_state.chat.select_all_input(0);
        host.a11y_focus_chat_input();
        let chat = &host.editor_state.chat;
        assert!(chat.focused);
        assert!(chat.input.highlight_range().is_none());
        assert!(chat.transcript_selection.is_none());
        assert_eq!(chat.input.next_blink_flip_ms(1234), 1734);
    }

    #[test]
    fn collapsed_sidebar_drops_layer_panel_region() {
        let mut h = host();
        h.editor_state.editor_ui.sidebar_open = false;
        let update = h.accessibility_tree_update(1280.0, 800.0);
        let ids: Vec<_> = update.nodes.iter().map(|(id, _)| *id).collect();
        assert!(
            !ids.contains(&node_id(WidgetId::new(1000))),
            "layer panel hidden"
        );
    }
}
