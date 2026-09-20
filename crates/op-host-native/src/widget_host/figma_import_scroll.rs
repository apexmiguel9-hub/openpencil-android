use super::WidgetHostNative;
use op_editor_ui::Point2D;

impl WidgetHostNative {
    /// The modal scrim owns every wheel so the canvas cannot zoom
    /// underneath it. Only wheels over the prepared page list mutate
    /// the shared Select scroll state.
    pub(super) fn try_scroll_figma_import(
        &mut self,
        x: f32,
        y: f32,
        delta_y: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> bool {
        if !self.editor_state.editor_ui.figma_import_open {
            return false;
        }
        let modal =
            op_editor_ui::widgets::figma_import::FigmaImportModal::for_editor(&self.editor_state);
        let panel = modal.rect(viewport_width, viewport_height);
        if modal
            .page_list_rect(panel)
            .is_some_and(|rect| rect.contains(Point2D::new(x, y)))
        {
            let max = modal.max_page_scroll();
            let select = &mut self.editor_state.editor_ui.figma_import_page_select;
            let next = (select.scroll.offset - delta_y).clamp(0.0, max);
            if next != select.scroll.offset || select.hover.is_some() {
                select.scroll.offset = next;
                select.hover = None;
                self.editor_state.editor_ui.figma_import_hover = None;
                self.mark_dirty();
            }
        }
        true
    }
}
