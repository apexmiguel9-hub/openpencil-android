//! TopBar overlay paint arms — the File menu, the export quick menu, and
//! the hover tooltip.
//!
//! Carved out of `paint.rs` at the 800-line ceiling. All three are anchored
//! to a TopBar button and paint in the same overlay band, so they sit
//! together; `paint.rs` keeps only the call sites that fix their Z-order.

use super::WidgetHostNative;
use crate::backend::NativeFrameBackend;
use op_editor_ui::widgets::{ExportQuickMenu, PaintCx, TopBar, Widget};
use op_editor_ui::{Point2D, Rect};

impl WidgetHostNative {
    /// Hover tooltip for whichever TopBar chrome button has been dwelt
    /// on long enough. Paints nothing until the dwell elapses, and
    /// nothing at all for a button whose own menu is already open.
    pub(in crate::widget_host) fn paint_top_bar_tooltip_overlay(
        &self,
        frame: &mut NativeFrameBackend<'_>,
        top_bar: &TopBar,
        top_bar_rect: Rect,
        viewport_width: f32,
    ) {
        let mut cx = PaintCx {
            backend: &mut *frame,
        };
        op_editor_ui::widgets::top_bar_tooltip::paint_top_bar_tooltip(
            &mut cx,
            &self.editor_state.editor_ui,
            top_bar,
            top_bar_rect,
            viewport_width,
            self.now_ms,
        );
    }

    /// File-menu dropdown — anchored under the TopBar folder+chevron button.
    pub(in crate::widget_host) fn paint_file_menu_overlay(
        &self,
        frame: &mut NativeFrameBackend<'_>,
        viewport_width: f32,
    ) {
        use op_editor_ui::widgets::file_menu::FileMenu;
        use op_editor_ui::widgets::top_bar::TopBar;
        let top_bar_rect = Rect {
            origin: Point2D::new(0.0, 0.0),
            size: Point2D::new(viewport_width, op_editor_ui::widgets::TOP_BAR_HEIGHT),
        };
        let anchor =
            TopBar::file_menu_rect(top_bar_rect, self.editor_state.editor_ui.window_fullscreen);
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        // Selected-frame count only names the batch-export row; it leaves
        // row geometry untouched, so the hit-test call sites keep building
        // the menu without it.
        let selected_frames =
            op_editor_core::export_batch::selected_frame_count(&self.editor_state);
        let menu = FileMenu::from_editor_ui(&self.editor_state.editor_ui, now_secs)
            .with_selected_frames(selected_frames);
        let menu_rect = menu.rect_at(anchor);
        let mut cx = PaintCx {
            backend: &mut *frame,
        };
        menu.paint(&mut cx, menu_rect);
    }

    /// Export quick menu — anchored under the TopBar download button.
    pub(in crate::widget_host) fn paint_export_quick_menu_overlay(
        &self,
        frame: &mut NativeFrameBackend<'_>,
        viewport_width: f32,
    ) {
        let menu_rect = self.export_quick_menu_rect(viewport_width);
        let menu = ExportQuickMenu::for_editor_ui(&self.editor_state.editor_ui);
        let mut cx = PaintCx {
            backend: &mut *frame,
        };
        menu.paint(&mut cx, menu_rect);
    }
}
