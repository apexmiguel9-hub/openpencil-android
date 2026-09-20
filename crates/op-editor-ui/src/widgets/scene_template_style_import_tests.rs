//! Paste-box geometry and text-wrapping tests.

use super::*;
use crate::widgets::asset_center_style_cards::style_test_support::exclusive_user_styles;
use crate::widgets::scene_template_panel::test_rects::MEDIUM as PANEL;
use op_editor_core::{AssetCenterTab, EditorState};

fn open_panel_state() -> EditorState {
    let mut state = EditorState::default();
    state.editor_ui.open_scene_template_center(0);
    state
        .editor_ui
        .scene_template_center
        .select_tab(AssetCenterTab::Styles);
    state
}

fn with_box(text: &str) -> EditorState {
    let mut state = open_panel_state();
    state
        .editor_ui
        .scene_template_center
        .open_style_import_paste(0);
    state
        .editor_ui
        .scene_template_center
        .import
        .text
        .set_text(text);
    state
}

#[test]
fn a_closed_box_routes_nothing() {
    let _guard = exclusive_user_styles();
    let state = open_panel_state();
    let panel = SceneTemplatePanel::for_editor(&state).expect("open");
    assert!(!panel.style_import_open());
    assert_eq!(panel.style_import_hit(PANEL, PANEL.origin), None);
}

/// While the box is up it owns every press inside the panel — including one
/// aimed at gallery chrome it covers, which must dismiss rather than fire the
/// control the user cannot see.
#[test]
fn an_open_box_owns_every_press_in_the_panel() {
    let _guard = exclusive_user_styles();
    let state = with_box("");
    let panel = SceneTemplatePanel::for_editor(&state).expect("open");

    let confirm = panel.style_import_confirm_rect(PANEL);
    assert_eq!(
        panel.style_import_hit(PANEL, center_of(confirm)),
        Some(SceneTemplateHit::ConfirmStyleImport)
    );
    let cancel = panel.style_import_cancel_rect(PANEL);
    assert_eq!(
        panel.style_import_hit(PANEL, center_of(cancel)),
        Some(SceneTemplateHit::CancelStyleImport)
    );
    // The gallery's close button is under the scrim; pressing there dismisses
    // the box instead of closing the whole Asset Center.
    let close = center_of(SceneTemplatePanel::close_rect(PANEL));
    assert_eq!(
        panel.style_import_hit(PANEL, close),
        Some(SceneTemplateHit::CancelStyleImport)
    );
    // Inside the box but on no control: swallowed, not dismissed.
    let box_rect = panel.style_import_rect(PANEL);
    assert_eq!(
        panel.style_import_hit(
            PANEL,
            Point2D::new(box_rect.origin.x + 4.0, box_rect.origin.y + 4.0)
        ),
        Some(SceneTemplateHit::InsideStyleImport)
    );
}

#[test]
fn the_boxs_controls_do_not_overlap_each_other() {
    let _guard = exclusive_user_styles();
    let state = with_box("");
    let panel = SceneTemplatePanel::for_editor(&state).expect("open");
    let box_rect = panel.style_import_rect(PANEL);
    let text = panel.style_import_text_rect(PANEL);
    let cancel = panel.style_import_cancel_rect(PANEL);
    let confirm = panel.style_import_confirm_rect(PANEL);

    for rect in [text, cancel, confirm] {
        assert!(
            rect.origin.x >= box_rect.origin.x
                && rect.origin.x + rect.size.x <= box_rect.origin.x + box_rect.size.x
                && rect.origin.y >= box_rect.origin.y
                && rect.origin.y + rect.size.y <= box_rect.origin.y + box_rect.size.y,
            "control {rect:?} escapes the box {box_rect:?}"
        );
    }
    assert!(text.origin.y + text.size.y <= cancel.origin.y);
    assert!(cancel.origin.x + cancel.size.x <= confirm.origin.x);
    assert!(text.size.y > 0.0, "the text area must have room to paint");
}

#[test]
fn a_press_in_the_text_area_focuses_it() {
    let _guard = exclusive_user_styles();
    let state = with_box("# Guide\nprose");
    let panel = SceneTemplatePanel::for_editor(&state).expect("open");
    let text = panel.style_import_text_rect(PANEL);
    assert!(matches!(
        panel.style_import_hit(
            PANEL,
            Point2D::new(text.origin.x + 2.0, text.origin.y + 2.0)
        ),
        Some(SceneTemplateHit::FocusStyleImport(_))
    ));
}

/// A caret index is an offset into the pasted document, so the wrapper has to
/// report where each display line starts — deriving it by summing line lengths
/// miscounts every line the wrapper broke rather than the author.
#[test]
fn wrapped_lines_report_their_offset_in_the_original_text() {
    let text = "alpha\nbeta\ngamma";
    let lines = wrap_document(text, 10_000.0, 11.5);
    assert_eq!(
        lines,
        vec![
            (0, "alpha".to_string()),
            (6, "beta".to_string()),
            (11, "gamma".to_string()),
        ]
    );
    for (start, line) in &lines {
        assert_eq!(&text[*start..*start + line.len()], line);
    }
}

/// Paragraph breaks are part of how a markdown document reads; collapsing
/// them would show the user a different document from the one they copied.
#[test]
fn blank_lines_survive_wrapping() {
    let lines = wrap_document("a\n\nb", 10_000.0, 11.5);
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[1].1, "");
}

#[test]
fn a_long_line_wraps_and_keeps_its_offsets_consistent() {
    let text = "x".repeat(400);
    let lines = wrap_document(&text, 100.0, 11.5);
    assert!(lines.len() > 1, "a 400-char line must break");
    let mut expected = 0_usize;
    for (start, line) in &lines {
        assert_eq!(*start, expected);
        expected += line.len();
    }
    assert_eq!(expected, text.len(), "wrapping must not drop characters");
}

#[test]
fn multibyte_text_wraps_on_character_boundaries() {
    let text = "温暖厨房的设计说明\n主色 #E07A5F";
    let lines = wrap_document(text, 40.0, 11.5);
    for (start, line) in &lines {
        // Panics on a bad boundary, which is the assertion.
        assert_eq!(&text[*start..*start + line.len()], line);
    }
}

fn center_of(rect: Rect) -> Point2D {
    Point2D::new(
        rect.origin.x + rect.size.x / 2.0,
        rect.origin.y + rect.size.y / 2.0,
    )
}
