//! Web arm over the shared HTML-import diagnostics overlay flow.
//!
//! The behaviour lives in
//! `op_editor_ui::widgets::html_import_diagnostics_flow` (the native host
//! drives the same flow); this file only supplies the viewport and the
//! `mark_dirty` tails, matching `missing_fonts_press.rs`.

use super::WidgetHost;
use op_editor_core::html_import_diagnostics::HtmlImportDiagnostic;
use op_editor_ui::widgets::html_import_diagnostics_flow as diagnostics_flow;
use op_editor_ui::Point2D;

impl WidgetHost {
    /// Publish the diagnostics of a finished HTML import and raise the
    /// overlay. Called by `file_actions` once an ingest lands.
    pub(crate) fn show_html_import_diagnostics(&mut self, diagnostics: Vec<HtmlImportDiagnostic>) {
        diagnostics_flow::publish(&mut self.editor_state, diagnostics);
        self.mark_dirty();
    }

    /// Route a press to the overlay. `false` means the point missed the card,
    /// so the press falls through — the overlay is a non-modal notice.
    pub(in crate::widget_host) fn dispatch_html_import_diagnostics_press(
        &mut self,
        x: f32,
        y: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> bool {
        if !diagnostics_flow::press(
            &mut self.editor_state,
            viewport_width,
            viewport_height,
            Point2D::new(x, y),
        ) {
            return false;
        }
        self.mark_dirty();
        true
    }

    /// Wheel routed to the expanded rows.
    pub(in crate::widget_host) fn try_scroll_html_import_diagnostics(
        &mut self,
        x: f32,
        y: f32,
        delta_y: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> bool {
        let Some(changed) = diagnostics_flow::scroll(
            &mut self.editor_state,
            x,
            y,
            delta_y,
            viewport_width,
            viewport_height,
        ) else {
            return false;
        };
        if changed {
            self.mark_dirty();
        }
        true
    }

    /// Hover tint for the overlay's two buttons. Returns whether the cursor
    /// is over the card, so the caller can stop the hover ladder there.
    pub(in crate::widget_host) fn update_html_import_diagnostics_hover(
        &mut self,
        x: f32,
        y: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> bool {
        if diagnostics_flow::hover(
            &mut self.editor_state,
            viewport_width,
            viewport_height,
            Point2D::new(x, y),
        ) {
            self.mark_dirty();
        }
        diagnostics_flow::panel_rect(&self.editor_state, viewport_width, viewport_height)
            .is_some_and(|rect| rect.contains(Point2D::new(x, y)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diagnostic(index: usize) -> HtmlImportDiagnostic {
        HtmlImportDiagnostic::new(
            "layout.float_ignored",
            "htmlImport.warn.layout.float_ignored",
            Vec::new(),
            format!("CSS float ignored during structured HTML import {index}"),
        )
    }

    fn host_with_diagnostics(count: usize) -> WidgetHost {
        let mut host = WidgetHost::new();
        host.show_html_import_diagnostics((0..count).map(diagnostic).collect());
        host
    }

    fn control_points(
        host: &WidgetHost,
        viewport_width: f32,
        viewport_height: f32,
    ) -> (Point2D, Point2D) {
        let panel =
            op_editor_ui::widgets::HtmlImportDiagnosticsPanel::for_editor(&host.editor_state)
                .expect("overlay is open");
        let rect = panel.rect(viewport_width, viewport_height);
        let centre = |r: op_editor_ui::Rect| {
            Point2D::new(r.origin.x + r.size.x / 2.0, r.origin.y + r.size.y / 2.0)
        };
        (
            centre(panel.toggle_rect(rect)),
            centre(panel.dismiss_rect(rect)),
        )
    }

    #[test]
    fn publishing_raises_a_collapsed_overlay() {
        let host = host_with_diagnostics(2);
        let ui = &host.editor_state.editor_ui;
        assert!(ui.html_import_diagnostics_open);
        assert!(!ui.html_import_diagnostics_expanded);
        assert_eq!(ui.html_import_diagnostics_total, 2);
    }

    #[test]
    fn a_press_outside_the_card_falls_through() {
        let mut host = host_with_diagnostics(2);
        assert!(!host.dispatch_html_import_diagnostics_press(20.0, 200.0, 1200.0, 800.0));
        assert!(host.editor_state.editor_ui.html_import_diagnostics_open);
    }

    #[test]
    fn dismiss_hides_the_overlay_and_releases_the_rows() {
        let mut host = host_with_diagnostics(3);
        let (_, dismiss) = control_points(&host, 1200.0, 800.0);
        assert!(host.dispatch_html_import_diagnostics_press(dismiss.x, dismiss.y, 1200.0, 800.0));
        assert!(!host.editor_state.editor_ui.html_import_diagnostics_open);
        assert!(host
            .editor_state
            .editor_ui
            .html_import_diagnostics
            .is_empty());
        assert_eq!(host.editor_state.editor_ui.html_import_diagnostics_total, 3);
    }

    #[test]
    fn the_toggle_expands_the_rows() {
        let mut host = host_with_diagnostics(5);
        let (toggle, _) = control_points(&host, 1200.0, 800.0);
        assert!(host.dispatch_html_import_diagnostics_press(toggle.x, toggle.y, 1200.0, 800.0));
        assert!(host.editor_state.editor_ui.html_import_diagnostics_expanded);
    }
}
