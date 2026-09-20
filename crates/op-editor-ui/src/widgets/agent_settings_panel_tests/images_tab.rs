//! Images tab: profile rows, advanced search fields, carets and the test button.
//!
//! Split out of `agent_settings_panel_tests.rs` to keep that file under the
//! 800-line cap.

use super::*;

#[test]
fn images_tab_profile_rows_expose_active_and_remove_targets() {
    let mut state = EditorState::default();
    state.editor_ui.agent_settings.tab = AgentSettingsTab::Images;
    state.editor_ui.agent_settings.add_image_gen_profile();
    let panel = AgentSettingsPanel::for_editor(&state);
    let rect = panel.rect(1200.0, 800.0);
    let content_x = crate::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .x;
    let content_y = crate::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .y;
    let content_w = crate::widgets::agent_settings_panel::secondary_tab_body(rect)
        .size
        .x;

    let gen_top = content_y + 36.0 + 24.0 + 28.0;
    let row_y = gen_top + 36.0 + 8.0;

    assert_eq!(
        panel.hit_test(rect, crate::Point2D::new(content_x + 24.0, row_y + 16.0)),
        AgentSettingsHit::SetActiveGenConfig(0)
    );
    assert_eq!(
        panel.hit_test(
            rect,
            crate::Point2D::new(content_x + content_w - 12.0, row_y + 16.0)
        ),
        AgentSettingsHit::RemoveGenConfig(0)
    );
}

#[test]
fn images_tab_profile_row_paints_expand_chevron_before_delete_like_ts() {
    let mut state = EditorState::default();
    state.editor_ui.agent_settings.tab = AgentSettingsTab::Images;
    state.editor_ui.agent_settings.add_image_gen_profile();
    let panel = AgentSettingsPanel::for_editor(&state);
    let rect = panel.rect(1200.0, 800.0);
    let content_x = crate::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .x;
    let content_y = crate::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .y;
    let content_w = crate::widgets::agent_settings_panel::secondary_tab_body(rect)
        .size
        .x;
    let gen_top = content_y + 36.0 + 24.0 + 28.0;
    let row_y = gen_top + 36.0 + 8.0;
    let row_x = content_x + 8.0;
    let row_w = content_w - 16.0;
    let chevron_origin = crate::Point2D::new(row_x + row_w - 32.0 - 24.0 + 4.0, row_y + 10.0);
    let delete_origin = crate::Point2D::new(content_x + content_w - 30.0, row_y + 10.0);

    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    panel.paint(&mut cx, rect);

    let chevron_idx = backend
        .icon_strokes
        .iter()
        .find_map(|(at, size, idx)| {
            ((*size - 12.0).abs() < 0.01
                && (at.x - chevron_origin.x).abs() < 0.01
                && (at.y - chevron_origin.y).abs() < 0.01)
                .then_some(*idx)
        })
        .expect("profile rows should paint the TS expand/collapse chevron");
    let delete_idx = backend
        .icon_strokes
        .iter()
        .find_map(|(at, size, idx)| {
            ((*size - 12.0).abs() < 0.01
                && (at.x - delete_origin.x).abs() < 0.01
                && (at.y - delete_origin.y).abs() < 0.01)
                .then_some(*idx)
        })
        .expect("profile rows should paint the delete icon");
    assert!(
        chevron_idx < delete_idx,
        "profile rows should paint the TS expand/collapse chevron before the delete icon"
    );
}

#[test]
fn images_tab_advanced_search_fields_are_focusable() {
    let mut state = EditorState::default();
    state.editor_ui.agent_settings.tab = AgentSettingsTab::Images;
    state.editor_ui.agent_settings.images_advanced_open = true;
    let panel = AgentSettingsPanel::for_editor(&state);
    let rect = panel.rect(1200.0, 800.0);
    let content_x = crate::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .x;
    let content_y = crate::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .y;
    let field_x = content_x + 110.0 + 16.0;
    let first_field_y = content_y + 36.0 + 24.0 + 22.0;

    assert_eq!(
        panel.hit_test(rect, crate::Point2D::new(field_x, first_field_y + 18.0)),
        AgentSettingsHit::FocusSearchField(ImageSearchField::ClientId)
    );
    assert_eq!(
        panel.hit_test(
            rect,
            crate::Point2D::new(field_x, first_field_y + 36.0 + 10.0 + 18.0)
        ),
        AgentSettingsHit::FocusSearchField(ImageSearchField::ClientSecret)
    );
}

#[test]
fn focused_image_search_field_paints_visible_caret_at_blink_on_phase() {
    let mut state = EditorState::default();
    state.editor_ui.agent_settings.tab = AgentSettingsTab::Images;
    state.editor_ui.agent_settings.images_advanced_open = true;
    state.editor_ui.agent_settings.focus =
        Some(SettingsFocus::ImageSearch(ImageSearchField::ClientId));
    state.editor_ui.settings_input.set_text("openverse-client");

    let panel = AgentSettingsPanel::for_editor_at(&state, 100);
    let rect = panel.rect(1200.0, 800.0);
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    panel.paint(&mut cx, rect);

    assert_eq!(caret_fills(&backend.fills, panel.theme.foreground).len(), 1);
}

#[test]
fn focused_empty_image_search_placeholder_leaves_gap_after_caret() {
    let mut state = EditorState::default();
    state.editor_ui.agent_settings.tab = AgentSettingsTab::Images;
    state.editor_ui.agent_settings.images_advanced_open = true;
    state.editor_ui.agent_settings.focus =
        Some(SettingsFocus::ImageSearch(ImageSearchField::ClientSecret));
    state.editor_ui.settings_input.set_text("");
    state.editor_ui.settings_input.set_caret(0, 0);

    let panel = AgentSettingsPanel::for_editor_at(&state, 100);
    let rect = panel.rect(1200.0, 800.0);
    let content_x = crate::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .x;
    let content_y = crate::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .y;
    let content_w = crate::widgets::agent_settings_panel::secondary_tab_body(rect)
        .size
        .x;
    let secret_y = content_y + 36.0 + 24.0 + 22.0 + 36.0 + 10.0;
    let field = Rect {
        origin: Point2D::new(content_x + 110.0, secret_y),
        size: Point2D::new(content_w - 110.0, 36.0),
    };
    let placeholder_baseline_y = field.origin.y + 36.0 / 2.0 + 5.0;
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    panel.paint(&mut cx, rect);

    let caret = caret_fills(&backend.fills, panel.theme.foreground)
        .into_iter()
        .next()
        .expect("focused empty image search field should paint a caret");
    let placeholder = backend
        .text_points
        .iter()
        .copied()
        .find(|point| {
            (point.y - placeholder_baseline_y).abs() < 0.01
                && point.x > field.origin.x
                && point.x < field.origin.x + field.size.x
        })
        .expect("empty focused image search field should paint its placeholder");

    assert!(
        placeholder.x >= caret.origin.x + 6.0,
        "focused placeholder should leave a visible gap after the caret"
    );
}

#[test]
fn focused_image_search_field_hides_caret_at_blink_off_phase() {
    let mut state = EditorState::default();
    state.editor_ui.agent_settings.tab = AgentSettingsTab::Images;
    state.editor_ui.agent_settings.images_advanced_open = true;
    state.editor_ui.agent_settings.focus =
        Some(SettingsFocus::ImageSearch(ImageSearchField::ClientId));
    state.editor_ui.settings_input.set_text("openverse-client");

    let panel = AgentSettingsPanel::for_editor_at(&state, 500);
    let rect = panel.rect(1200.0, 800.0);
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    panel.paint(&mut cx, rect);

    assert!(caret_fills(&backend.fills, panel.theme.foreground).is_empty());
}

#[test]
fn images_tab_test_search_requires_some_oauth_text() {
    let mut state = EditorState::default();
    state.editor_ui.agent_settings.tab = AgentSettingsTab::Images;
    state.editor_ui.agent_settings.images_advanced_open = true;
    let panel = AgentSettingsPanel::for_editor(&state);
    let rect = panel.rect(1200.0, 800.0);
    let content_y = crate::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .y;
    let content_w = crate::widgets::agent_settings_panel::secondary_tab_body(rect)
        .size
        .x;
    let button_x = crate::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .x
        + content_w
        - 28.0;
    let button_y = content_y + 36.0 + 24.0 + 22.0 + 36.0 + 10.0 + 36.0 + 14.0 + 18.0;

    assert_eq!(
        panel.hit_test(rect, crate::Point2D::new(button_x, button_y)),
        AgentSettingsHit::Inside
    );

    state.editor_ui.agent_settings.openverse_client_id = "client".into();
    let panel = AgentSettingsPanel::for_editor(&state);
    assert_eq!(
        panel.hit_test(rect, crate::Point2D::new(button_x, button_y)),
        AgentSettingsHit::TestImageSearch
    );
}

#[test]
fn images_tab_test_search_is_disabled_while_testing_like_ts() {
    let mut state = EditorState::default();
    state.editor_ui.agent_settings.tab = AgentSettingsTab::Images;
    state.editor_ui.agent_settings.images_advanced_open = true;
    state.editor_ui.agent_settings.openverse_client_id = "client".into();
    state.editor_ui.agent_settings.images_search_test_status = ImageTestStatus::Testing;
    let panel = AgentSettingsPanel::for_editor(&state);
    let rect = panel.rect(1200.0, 800.0);
    let content_y = crate::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .y;
    let content_w = crate::widgets::agent_settings_panel::secondary_tab_body(rect)
        .size
        .x;
    let button_x = crate::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .x
        + content_w
        - 28.0;
    let button_y = content_y + 36.0 + 24.0 + 22.0 + 36.0 + 10.0 + 36.0 + 14.0 + 18.0;

    assert_eq!(
        panel.hit_test(rect, crate::Point2D::new(button_x, button_y)),
        AgentSettingsHit::Inside
    );
}
