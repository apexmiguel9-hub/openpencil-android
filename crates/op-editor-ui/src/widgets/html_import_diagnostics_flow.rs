//! HTML-import diagnostics overlay flow shared by both widget hosts.
//!
//! Mirrors `missing_fonts_flow`: every host-visible transition of the overlay
//! (publish, press, wheel, hover, dismiss) is `EditorState` mutation plus
//! widget-layer hit-tests, so `op-host-native` and `op-host-web` only supply
//! the viewport rect and the `mark_dirty()` tail.
//!
//! The overlay is deliberately non-modal: [`press`] returns `false` for a
//! point outside the card, so the canvas underneath keeps receiving presses
//! while the report is up.

use op_editor_core::html_import_diagnostics::{cap_rows, HtmlImportDiagnostic};
use op_editor_core::EditorState;

use crate::widgets::html_import_diagnostics_panel::{
    HtmlImportDiagnosticsHit, HtmlImportDiagnosticsPanel,
};
use crate::{Point2D, Rect};

/// Publish the diagnostics of a finished HTML import and raise the overlay.
///
/// An import that degraded nothing clears any previous report instead of
/// leaving a stale card on screen.
pub fn publish(state: &mut EditorState, mut diagnostics: Vec<HtmlImportDiagnostic>) {
    let summary = cap_rows(&mut diagnostics);
    let ui = &mut state.editor_ui;
    ui.html_import_diagnostics_open = !diagnostics.is_empty();
    ui.html_import_diagnostics_expanded = false;
    ui.html_import_diagnostics_scroll.offset = 0.0;
    ui.html_import_diagnostics_hover = None;
    ui.html_import_diagnostics_total = summary.total;
    ui.html_import_diagnostics = diagnostics;
}

/// Hide the overlay and drop its rows.
///
/// Dismissal is the end of the report's life: no other surface lists these
/// rows, and `EditorUiState` is cloned on every off-thread request snapshot,
/// so keeping up to `MAX_DIAGNOSTIC_ROWS` localizable sentences alive for the
/// rest of the session buys nothing. The scalar `_total` survives as the
/// record of what the last import did.
///
/// Deliberately NOT on the Escape ladder. Escape is a one-layer-per-press
/// dismissal of whatever modal-ish thing has focus, and this overlay takes no
/// focus at all — the canvas keeps every key while it is up. The
/// missing-fonts prompt, the other non-focus-trapping notice, is likewise
/// Escape-immune; both are dismissed by their own button. Adding Escape here
/// would make it eat a keypress the user aimed at the canvas.
pub fn dismiss(state: &mut EditorState) {
    let ui = &mut state.editor_ui;
    ui.html_import_diagnostics_open = false;
    ui.html_import_diagnostics_expanded = false;
    ui.html_import_diagnostics_hover = None;
    ui.html_import_diagnostics.clear();
    ui.html_import_diagnostics_scroll.offset = 0.0;
}

/// Route a press to the overlay. Returns `true` only when the point landed on
/// the card, so an outside press falls through to the tiers below.
pub fn press(
    state: &mut EditorState,
    viewport_width: f32,
    viewport_height: f32,
    point: Point2D,
) -> bool {
    let Some(hit) = HtmlImportDiagnosticsPanel::for_editor(state).map(|panel| {
        let rect = panel.rect(viewport_width, viewport_height);
        panel.hit_test(rect, point)
    }) else {
        return false;
    };
    match hit {
        HtmlImportDiagnosticsHit::Dismiss => {
            dismiss(state);
            true
        }
        HtmlImportDiagnosticsHit::Toggle => {
            let ui = &mut state.editor_ui;
            ui.html_import_diagnostics_expanded = !ui.html_import_diagnostics_expanded;
            ui.html_import_diagnostics_scroll.offset = 0.0;
            true
        }
        HtmlImportDiagnosticsHit::Inside => true,
        HtmlImportDiagnosticsHit::Outside => false,
    }
}

/// Wheel scroll routed to the expanded rows.
///
/// `None` means "not mine" (overlay closed, collapsed, or the pointer is
/// elsewhere) and the host keeps walking its wheel tiers; `Some(changed)`
/// reports whether an offset actually moved.
pub fn scroll(
    state: &mut EditorState,
    x: f32,
    y: f32,
    delta_y: f32,
    viewport_width: f32,
    viewport_height: f32,
) -> Option<bool> {
    let point = Point2D::new(x, y);
    let max_scroll = HtmlImportDiagnosticsPanel::for_editor(state).and_then(|panel| {
        let rect = panel.rect(viewport_width, viewport_height);
        panel
            .rows_rect(rect)
            .contains(point)
            .then(|| panel.max_rows_scroll(rect))
    })?;
    let ui = &mut state.editor_ui;
    let next = (ui.html_import_diagnostics_scroll.offset - delta_y).clamp(0.0, max_scroll);
    if next == ui.html_import_diagnostics_scroll.offset {
        return Some(false);
    }
    ui.html_import_diagnostics_scroll.offset = next;
    Some(true)
}

/// Update the hovered control. Returns `true` when the tint changed.
pub fn hover(
    state: &mut EditorState,
    viewport_width: f32,
    viewport_height: f32,
    point: Point2D,
) -> bool {
    let next = HtmlImportDiagnosticsPanel::for_editor(state).and_then(|panel| {
        let rect = panel.rect(viewport_width, viewport_height);
        panel.hover_at(rect, point)
    });
    if state.editor_ui.html_import_diagnostics_hover == next {
        return false;
    }
    state.editor_ui.html_import_diagnostics_hover = next;
    true
}

/// The overlay's card in viewport coordinates, when it is on screen.
pub fn panel_rect(state: &EditorState, viewport_width: f32, viewport_height: f32) -> Option<Rect> {
    HtmlImportDiagnosticsPanel::for_editor(state)
        .map(|panel| panel.rect(viewport_width, viewport_height))
}
