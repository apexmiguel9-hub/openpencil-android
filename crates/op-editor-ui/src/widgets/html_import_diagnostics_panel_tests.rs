//! Geometry, localization and flow guards for the HTML-import diagnostics
//! overlay. Mirrors the shape of the missing-fonts panel tests.

use op_editor_core::editor_ui_state::Locale;
use op_editor_core::html_import_diagnostics::{
    HtmlImportDiagnostic, HtmlImportDiagnosticsHover, MAX_DIAGNOSTIC_ROWS,
};
use op_editor_core::EditorState;

use super::html_import_diagnostics_flow as flow;
use super::html_import_diagnostics_panel::{row_text, HtmlImportDiagnosticsPanel};
use crate::Point2D;

const VIEWPORT: (f32, f32) = (1440.0, 900.0);

fn diagnostic(code: &str, args: &[(&str, &str)], message: &str) -> HtmlImportDiagnostic {
    HtmlImportDiagnostic::new(
        code,
        format!("htmlImport.warn.{code}"),
        args.iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect(),
        message,
    )
}

fn state_with(rows: Vec<HtmlImportDiagnostic>) -> EditorState {
    let mut state = EditorState::new();
    flow::publish(&mut state, rows);
    state
}

fn simple_rows(count: usize) -> Vec<HtmlImportDiagnostic> {
    (0..count)
        .map(|index| {
            diagnostic(
                "layout.float_ignored",
                &[],
                &format!("CSS float ignored during structured HTML import {index}"),
            )
        })
        .collect()
}

fn centre(rect: crate::Rect) -> Point2D {
    Point2D::new(
        rect.origin.x + rect.size.x / 2.0,
        rect.origin.y + rect.size.y / 2.0,
    )
}

#[test]
fn no_overlay_without_diagnostics() {
    let state = EditorState::new();
    assert!(HtmlImportDiagnosticsPanel::for_editor(&state).is_none());
}

#[test]
fn dismissing_stops_painting_and_releases_the_rows() {
    let mut state = state_with(simple_rows(2));
    assert!(HtmlImportDiagnosticsPanel::for_editor(&state).is_some());
    flow::dismiss(&mut state);
    assert!(HtmlImportDiagnosticsPanel::for_editor(&state).is_none());
    // Nothing else lists these rows and every request snapshot clones
    // `EditorUiState`, so dismissal is the end of their life.
    assert!(state.editor_ui.html_import_diagnostics.is_empty());
    assert_eq!(state.editor_ui.html_import_diagnostics_scroll.offset, 0.0);
    // The scalar record of what the import did survives.
    assert_eq!(state.editor_ui.html_import_diagnostics_total, 2);
}

#[test]
fn the_card_sits_in_the_bottom_right_and_grows_when_expanded() {
    let mut state = state_with(simple_rows(8));
    let collapsed = HtmlImportDiagnosticsPanel::for_editor(&state)
        .expect("open")
        .rect(VIEWPORT.0, VIEWPORT.1);
    assert!(collapsed.origin.x + collapsed.size.x <= VIEWPORT.0);
    assert!(collapsed.origin.y + collapsed.size.y <= VIEWPORT.1);
    assert!(collapsed.origin.x > VIEWPORT.0 / 2.0, "anchored right");

    state.editor_ui.html_import_diagnostics_expanded = true;
    let expanded = HtmlImportDiagnosticsPanel::for_editor(&state)
        .expect("open")
        .rect(VIEWPORT.0, VIEWPORT.1);
    assert!(expanded.size.y > collapsed.size.y);
    assert_eq!(expanded.size.x, collapsed.size.x);
    assert!(expanded.origin.y + expanded.size.y <= VIEWPORT.1);
}

#[test]
fn a_tiny_viewport_still_keeps_the_card_below_the_top_bar() {
    let state = state_with(simple_rows(3));
    let rect = HtmlImportDiagnosticsPanel::for_editor(&state)
        .expect("open")
        .rect(320.0, 120.0);
    assert!(rect.origin.y >= crate::widgets::TOP_BAR_HEIGHT);
    assert!(rect.origin.x >= 0.0);
}

#[test]
fn only_the_expanded_card_scrolls_and_the_offset_clamps() {
    let mut state = state_with(simple_rows(60));
    let point = Point2D::new(VIEWPORT.0 - 100.0, VIEWPORT.1 - 100.0);
    // Collapsed: no rows viewport, so the wheel belongs to a lower tier.
    assert!(flow::scroll(&mut state, point.x, point.y, -50.0, VIEWPORT.0, VIEWPORT.1).is_none());

    state.editor_ui.html_import_diagnostics_expanded = true;
    let rows = HtmlImportDiagnosticsPanel::for_editor(&state)
        .expect("open")
        .rows_rect(
            HtmlImportDiagnosticsPanel::for_editor(&state)
                .expect("open")
                .rect(VIEWPORT.0, VIEWPORT.1),
        );
    let inside = centre(rows);
    assert_eq!(
        flow::scroll(&mut state, inside.x, inside.y, -50.0, VIEWPORT.0, VIEWPORT.1),
        Some(true)
    );
    let after_one = state.editor_ui.html_import_diagnostics_scroll.offset;
    assert!(after_one > 0.0);
    // Scrolling far past the end clamps instead of running away.
    for _ in 0..200 {
        flow::scroll(
            &mut state, inside.x, inside.y, -50.0, VIEWPORT.0, VIEWPORT.1,
        );
    }
    let panel = HtmlImportDiagnosticsPanel::for_editor(&state).expect("open");
    let rect = panel.rect(VIEWPORT.0, VIEWPORT.1);
    assert_eq!(
        state.editor_ui.html_import_diagnostics_scroll.offset,
        panel.max_rows_scroll(rect)
    );
}

#[test]
fn hover_tracks_the_two_controls_and_clears_outside() {
    let mut state = state_with(simple_rows(2));
    let panel = HtmlImportDiagnosticsPanel::for_editor(&state).expect("open");
    let rect = panel.rect(VIEWPORT.0, VIEWPORT.1);
    let toggle = centre(panel.toggle_rect(rect));
    let dismiss = centre(panel.dismiss_rect(rect));

    assert!(flow::hover(&mut state, VIEWPORT.0, VIEWPORT.1, toggle));
    assert_eq!(
        state.editor_ui.html_import_diagnostics_hover,
        Some(HtmlImportDiagnosticsHover::Toggle)
    );
    assert!(flow::hover(&mut state, VIEWPORT.0, VIEWPORT.1, dismiss));
    assert_eq!(
        state.editor_ui.html_import_diagnostics_hover,
        Some(HtmlImportDiagnosticsHover::Dismiss)
    );
    assert!(flow::hover(
        &mut state,
        VIEWPORT.0,
        VIEWPORT.1,
        Point2D::new(10.0, 200.0)
    ));
    assert!(state.editor_ui.html_import_diagnostics_hover.is_none());
}

#[test]
fn expanding_resets_the_scroll_so_rows_start_at_the_top() {
    let mut state = state_with(simple_rows(40));
    state.editor_ui.html_import_diagnostics_scroll.offset = 120.0;
    let panel = HtmlImportDiagnosticsPanel::for_editor(&state).expect("open");
    let rect = panel.rect(VIEWPORT.0, VIEWPORT.1);
    let toggle = centre(panel.toggle_rect(rect));
    assert!(flow::press(&mut state, VIEWPORT.0, VIEWPORT.1, toggle));
    assert!(state.editor_ui.html_import_diagnostics_expanded);
    assert_eq!(state.editor_ui.html_import_diagnostics_scroll.offset, 0.0);
}

#[test]
fn publishing_more_than_the_cap_keeps_the_true_total() {
    let state = state_with(simple_rows(MAX_DIAGNOSTIC_ROWS + 7));
    assert_eq!(
        state.editor_ui.html_import_diagnostics.len(),
        MAX_DIAGNOSTIC_ROWS
    );
    assert_eq!(
        state.editor_ui.html_import_diagnostics_total,
        MAX_DIAGNOSTIC_ROWS + 7
    );
    let panel = HtmlImportDiagnosticsPanel::for_editor(&state).expect("open");
    assert!(panel
        .summary_text()
        .contains(&(MAX_DIAGNOSTIC_ROWS + 7).to_string()));
}

#[test]
fn row_copy_is_localized_and_interpolated() {
    let mut state = state_with(vec![diagnostic(
        "list.style_type_unsupported",
        &[("value", "hebrew")],
        "unsupported CSS list-style-type `hebrew` was approximated as a disc",
    )]);
    let row = state.editor_ui.html_import_diagnostics[0].clone();

    // The default locale follows the host environment, so pin it explicitly.
    state.editor_ui.locale = Locale::EnUs;
    let english = row_text(&state.editor_ui, &row);
    assert!(english.contains("hebrew"), "{english}");
    assert!(
        !english.contains("{{"),
        "placeholder left unfilled: {english}"
    );

    state.editor_ui.locale = Locale::ZhCn;
    let chinese = row_text(&state.editor_ui, &row);
    assert!(chinese.contains("hebrew"), "{chinese}");
    assert_ne!(chinese, english, "zh-CN copy must differ from English");
}

#[test]
fn an_unknown_code_falls_back_to_the_importers_own_sentence() {
    let state = state_with(vec![HtmlImportDiagnostic::new(
        "future.not_yet_translated",
        "htmlImport.warn.future.not_yet_translated",
        Vec::new(),
        "a brand new degradation",
    )]);
    let row = &state.editor_ui.html_import_diagnostics[0];
    assert_eq!(row_text(&state.editor_ui, row), "a brand new degradation");
}

#[test]
fn the_summary_is_localized_in_every_shipped_locale() {
    let mut state = state_with(simple_rows(3));
    for locale in Locale::ALL {
        state.editor_ui.locale = locale;
        let summary = HtmlImportDiagnosticsPanel::for_editor(&state)
            .expect("open")
            .summary_text();
        assert!(summary.contains('3'), "{locale:?}: {summary}");
        assert!(!summary.contains("{{"), "{locale:?}: {summary}");
    }
}

/// Paint the expanded card into a recording backend and return every string
/// it drew, in paint order.
fn painted_texts(state: &EditorState) -> Vec<String> {
    use crate::widgets::test_capture_backend::CaptureBackend;
    use crate::widgets::{PaintCx, Widget};
    let panel = HtmlImportDiagnosticsPanel::for_editor(state).expect("open");
    let rect = panel.rect(VIEWPORT.0, VIEWPORT.1);
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    panel.paint(&mut cx, rect);
    backend.texts.into_iter().map(|(text, _)| text).collect()
}

#[test]
fn a_long_row_wraps_to_two_lines_instead_of_being_clipped() {
    let long = "An unsupported CSS background-image layer with a very long \
                description that could never fit on a single 420 pixel row was \
                ignored during the structured import.";
    let mut state = state_with(vec![diagnostic("visual.unknown_long_code", &[], long)]);
    state.editor_ui.locale = Locale::EnUs;
    state.editor_ui.html_import_diagnostics_expanded = true;

    let texts = painted_texts(&state);
    // Header title + summary + two buttons, then the row's message lines and
    // its code line.
    let message_lines: Vec<&String> = texts
        .iter()
        .filter(|text| long.starts_with(text.trim_end_matches('…').trim_end()))
        .collect();
    assert_eq!(
        message_lines.len(),
        1,
        "the first line should be a prefix of the sentence: {texts:?}"
    );
    assert!(
        texts.iter().any(|text| text.ends_with('…')),
        "the second line must ellipsize rather than clip: {texts:?}"
    );
    assert!(
        texts.iter().any(|text| text == "visual.unknown_long_code"),
        "the code line still paints: {texts:?}"
    );
}

#[test]
fn a_short_row_paints_one_line_with_no_ellipsis() {
    let mut state = state_with(vec![diagnostic(
        "layout.float_ignored",
        &[],
        "CSS float was ignored.",
    )]);
    state.editor_ui.locale = Locale::EnUs;
    state.editor_ui.html_import_diagnostics_expanded = true;

    let texts = painted_texts(&state);
    assert!(
        texts.iter().any(|text| text == "CSS float was ignored."),
        "{texts:?}"
    );
    assert!(!texts.iter().any(|text| text.ends_with('…')), "{texts:?}");
}

#[test]
fn a_capped_report_surfaces_the_dropped_rows_as_a_trailing_more_line() {
    let mut state = state_with(simple_rows(MAX_DIAGNOSTIC_ROWS + 4));
    state.editor_ui.locale = Locale::EnUs;
    state.editor_ui.html_import_diagnostics_expanded = true;
    // The footer row is scrollable content of its own, so scrolling to the
    // very end is what brings it into the viewport.
    let max_scroll = {
        let panel = HtmlImportDiagnosticsPanel::for_editor(&state).expect("open");
        assert_eq!(panel.hidden_rows(), 4);
        let rect = panel.rect(VIEWPORT.0, VIEWPORT.1);
        panel.max_rows_scroll(rect)
    };

    state.editor_ui.html_import_diagnostics_scroll.offset = max_scroll;
    let texts = painted_texts(&state);
    assert!(
        texts.iter().any(|text| text == "+4 more"),
        "expected the +N more footer: {texts:?}"
    );
}

#[test]
fn an_uncapped_report_paints_no_more_line() {
    let mut state = state_with(simple_rows(3));
    state.editor_ui.locale = Locale::EnUs;
    state.editor_ui.html_import_diagnostics_expanded = true;
    assert_eq!(
        HtmlImportDiagnosticsPanel::for_editor(&state)
            .expect("open")
            .hidden_rows(),
        0
    );
    let texts = painted_texts(&state);
    assert!(!texts.iter().any(|text| text.contains("more")), "{texts:?}");
}
