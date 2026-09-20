//! Native arm over the shared HTML-import diagnostics overlay flow.
//!
//! All of the overlay's behaviour lives in
//! `op_editor_ui::widgets::html_import_diagnostics_flow` (the web host drives
//! the same flow); this file only supplies the viewport and the `mark_dirty`
//! tails, matching `missing_fonts_dispatch.rs`.

use super::WidgetHostNative;
use op_editor_core::html_import_diagnostics::HtmlImportDiagnostic;
use op_editor_ui::widgets::html_import_diagnostics_flow as diagnostics_flow;
use op_editor_ui::Point2D;

impl WidgetHostNative {
    /// Publish the diagnostics of a finished HTML import and raise the
    /// overlay. Called by the desktop import session once its worker lands.
    pub fn show_html_import_diagnostics(&mut self, diagnostics: Vec<HtmlImportDiagnostic>) {
        diagnostics_flow::publish(&mut self.editor_state, diagnostics);
        self.mark_dirty();
    }

    /// Route a press to the overlay. `false` means the point missed the card,
    /// so the press falls through to the tiers below — the overlay is a
    /// non-modal notice and must not swallow canvas clicks.
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

    /// Hover tint for the overlay's two buttons.
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

    fn host_with_diagnostics(count: usize) -> WidgetHostNative {
        let mut host = WidgetHostNative::new();
        host.show_html_import_diagnostics((0..count).map(diagnostic).collect());
        host
    }

    #[test]
    fn publishing_raises_a_collapsed_overlay() {
        let host = host_with_diagnostics(3);
        let ui = &host.editor_state().editor_ui;
        assert!(ui.html_import_diagnostics_open);
        assert!(!ui.html_import_diagnostics_expanded);
        assert_eq!(ui.html_import_diagnostics.len(), 3);
        assert_eq!(ui.html_import_diagnostics_total, 3);
    }

    #[test]
    fn an_import_without_degradations_leaves_no_overlay() {
        let mut host = host_with_diagnostics(2);
        host.show_html_import_diagnostics(Vec::new());
        assert!(!host.editor_state().editor_ui.html_import_diagnostics_open);
        assert!(host
            .editor_state()
            .editor_ui
            .html_import_diagnostics
            .is_empty());
    }

    #[test]
    fn a_press_outside_the_card_is_not_consumed() {
        let mut host = host_with_diagnostics(3);
        assert!(!host.dispatch_html_import_diagnostics_press(10.0, 200.0, 1200.0, 800.0));
        assert!(host.editor_state().editor_ui.html_import_diagnostics_open);
    }

    #[test]
    fn the_toggle_expands_and_collapses_the_rows() {
        let mut host = host_with_diagnostics(6);
        let (toggle, _) = control_points(&host, 1200.0, 800.0);
        assert!(host.dispatch_html_import_diagnostics_press(toggle.x, toggle.y, 1200.0, 800.0));
        assert!(
            host.editor_state()
                .editor_ui
                .html_import_diagnostics_expanded
        );
        let (toggle, _) = control_points(&host, 1200.0, 800.0);
        assert!(host.dispatch_html_import_diagnostics_press(toggle.x, toggle.y, 1200.0, 800.0));
        assert!(
            !host
                .editor_state()
                .editor_ui
                .html_import_diagnostics_expanded
        );
    }

    #[test]
    fn dismiss_hides_the_overlay_and_releases_the_rows() {
        let mut host = host_with_diagnostics(4);
        let (_, dismiss) = control_points(&host, 1200.0, 800.0);
        assert!(host.dispatch_html_import_diagnostics_press(dismiss.x, dismiss.y, 1200.0, 800.0));
        let ui = &host.editor_state().editor_ui;
        assert!(!ui.html_import_diagnostics_open);
        assert!(ui.html_import_diagnostics.is_empty());
        assert_eq!(ui.html_import_diagnostics_total, 4);
    }

    #[test]
    fn wheel_scrolls_only_the_expanded_rows() {
        let mut host = host_with_diagnostics(40);
        // Collapsed: the rows viewport is empty, so the wheel is not ours.
        assert!(!host.try_scroll_html_import_diagnostics(1000.0, 700.0, -40.0, 1200.0, 800.0));
        let (toggle, _) = control_points(&host, 1200.0, 800.0);
        host.dispatch_html_import_diagnostics_press(toggle.x, toggle.y, 1200.0, 800.0);
        let rows_point = rows_point(&host, 1200.0, 800.0);
        assert!(host.try_scroll_html_import_diagnostics(
            rows_point.x,
            rows_point.y,
            -40.0,
            1200.0,
            800.0
        ));
        assert!(
            host.editor_state()
                .editor_ui
                .html_import_diagnostics_scroll
                .offset
                > 0.0
        );
    }

    #[test]
    fn hovering_a_button_tints_it_and_leaving_clears_it() {
        let mut host = host_with_diagnostics(2);
        let (toggle, _) = control_points(&host, 1200.0, 800.0);
        assert!(host.update_html_import_diagnostics_hover(toggle.x, toggle.y, 1200.0, 800.0));
        assert_eq!(
            host.editor_state().editor_ui.html_import_diagnostics_hover,
            Some(op_editor_core::html_import_diagnostics::HtmlImportDiagnosticsHover::Toggle)
        );
        assert!(!host.update_html_import_diagnostics_hover(10.0, 200.0, 1200.0, 800.0));
        assert!(host
            .editor_state()
            .editor_ui
            .html_import_diagnostics_hover
            .is_none());
    }

    /// Centres of the toggle and dismiss buttons in viewport coordinates.
    fn control_points(
        host: &WidgetHostNative,
        viewport_width: f32,
        viewport_height: f32,
    ) -> (Point2D, Point2D) {
        let panel =
            op_editor_ui::widgets::HtmlImportDiagnosticsPanel::for_editor(host.editor_state())
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

    fn rows_point(host: &WidgetHostNative, viewport_width: f32, viewport_height: f32) -> Point2D {
        let panel =
            op_editor_ui::widgets::HtmlImportDiagnosticsPanel::for_editor(host.editor_state())
                .expect("overlay is open");
        let rect = panel.rect(viewport_width, viewport_height);
        let rows = panel.rows_rect(rect);
        Point2D::new(
            rows.origin.x + rows.size.x / 2.0,
            rows.origin.y + rows.size.y / 2.0,
        )
    }
}

/// The card's ownership of its own rect — the half of the overlay that is
/// NOT about its two buttons.
#[cfg(test)]
mod overlay_ownership_tests {
    use super::*;
    use op_editor_ui::widgets::html_import_diagnostics_flow as diagnostics_flow;

    const VIEWPORT_W: f32 = 1200.0;
    const VIEWPORT_H: f32 = 800.0;

    fn host_with_card() -> WidgetHostNative {
        let mut host = WidgetHostNative::new();
        host.show_html_import_diagnostics(vec![HtmlImportDiagnostic::new(
            "layout.float_ignored",
            "htmlImport.warn.layout.float_ignored",
            Vec::new(),
            "CSS float ignored during structured HTML import",
        )]);
        host
    }

    fn card_centre(host: &WidgetHostNative) -> Point2D {
        let rect = host
            .html_import_diagnostics_rect(VIEWPORT_W, VIEWPORT_H)
            .expect("card is up");
        Point2D::new(
            rect.origin.x + rect.size.x / 2.0,
            rect.origin.y + rect.size.y / 2.0,
        )
    }

    #[test]
    fn the_whole_card_joins_the_topmost_panel_predicate() {
        let host = host_with_card();
        let point = card_centre(&host);
        // The header band, not just the collapsed rows viewport: a wheel or a
        // right-press there must not fall through to the canvas.
        assert!(host.over_topmost_panel(point.x, point.y, VIEWPORT_W, VIEWPORT_H));
        assert!(host.over_floating_overlay(point.x, point.y, VIEWPORT_W, VIEWPORT_H));
    }

    #[test]
    fn a_dismissed_card_releases_its_rect() {
        let mut host = host_with_card();
        let point = card_centre(&host);
        diagnostics_flow::dismiss(host.editor_state_mut());
        assert!(host
            .html_import_diagnostics_rect(VIEWPORT_W, VIEWPORT_H)
            .is_none());
        assert!(!host.over_topmost_panel(point.x, point.y, VIEWPORT_W, VIEWPORT_H));
    }

    #[test]
    fn a_press_on_the_card_never_places_a_text_edit_caret() {
        let mut host = host_with_card();
        let point = card_centre(&host);
        // `text_edit_press_offset` probes overlay ownership through
        // `over_floating_overlay`, so the card is covered by the same entry
        // that covers the missing-fonts modal and the floating panels.
        assert!(host
            .text_edit_press_offset(point.x, point.y, VIEWPORT_W, VIEWPORT_H)
            .is_none());
    }
}
