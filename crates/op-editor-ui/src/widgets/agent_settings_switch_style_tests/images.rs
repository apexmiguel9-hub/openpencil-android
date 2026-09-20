//! Images-tab button styling — hover and pressed feedback on the search
//! test and the generation add actions.
//!
//! Split out of `agent_settings_switch_style_tests.rs` to keep that file
//! under the 800-line cap.

use super::*;

/// The Images tab body sits under a hero the panel paints for it.
fn images_body_metrics(rect: Rect) -> (f32, f32, f32) {
    let body = crate::widgets::agent_settings_panel::secondary_tab_body(rect);
    (body.origin.x, body.origin.y, body.size.x)
}

fn image_search_test_button_rect(rect: Rect) -> Rect {
    let (content_x, content_y, content_w) = images_body_metrics(rect);
    let register_y = content_y + 36.0 + 24.0 + 22.0 + 36.0 + 10.0 + 36.0 + 14.0;
    Rect {
        origin: Point2D::new(content_x + content_w - 56.0, register_y + 4.0),
        size: Point2D::new(56.0, 28.0),
    }
}

fn image_gen_add_button_rect(rect: Rect) -> Rect {
    let (content_x, content_y, content_w) = images_body_metrics(rect);
    let advanced_body_h = 22.0 + 36.0 + 10.0 + 36.0 + 14.0 + 36.0;
    let gen_top = content_y + 36.0 + 24.0 + advanced_body_h + 28.0;
    Rect {
        origin: Point2D::new(content_x + content_w - 72.0, gen_top + 4.0),
        size: Point2D::new(72.0, 28.0),
    }
}

#[test]
fn hovered_image_settings_buttons_paint_hover_wash() {
    let mut state = EditorState::default();
    state.editor_ui.theme_mode = ThemeMode::Light;
    state.editor_ui.agent_settings.tab = AgentSettingsTab::Images;
    state.editor_ui.agent_settings.images_advanced_open = true;
    state
        .editor_ui
        .agent_settings
        .hover_image_search_test_button = true;
    state.editor_ui.agent_settings.hover_image_gen_add_button = true;
    let panel = AgentSettingsPanel::for_editor(&state);
    let rect = panel.rect(1200.0, 800.0);
    let search_test = image_search_test_button_rect(rect);
    let add = image_gen_add_button_rect(rect);
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    panel.paint(&mut cx, rect);

    assert!(
        backend.round_fills.iter().any(
            |(r, color)| rect_eq(*r, search_test) && color_eq(*color, panel.theme.button_hover)
        ),
        "hovering the image search test button should paint the shared hover token"
    );
    assert!(
        backend
            .round_fills
            .iter()
            .any(|(r, color)| rect_eq(*r, add) && color_eq(*color, panel.theme.button_hover)),
        "hovering the image generation add button should paint the shared hover token"
    );
}

#[test]
fn pressed_image_settings_buttons_use_shared_button_feedback() {
    for (button, expected_rect) in [
        (
            AgentSettingsButton::ImageSearchTest,
            image_search_test_button_rect as fn(Rect) -> Rect,
        ),
        (
            AgentSettingsButton::ImageGenAdd,
            image_gen_add_button_rect as fn(Rect) -> Rect,
        ),
    ] {
        let mut state = EditorState::default();
        state.editor_ui.theme_mode = ThemeMode::Light;
        state.editor_ui.agent_settings.tab = AgentSettingsTab::Images;
        state.editor_ui.agent_settings.images_advanced_open = true;
        state.editor_ui.pressed_button = Some(ButtonPressTarget::AgentSettings(button));
        let panel = AgentSettingsPanel::for_editor(&state);
        let rect = panel.rect(1200.0, 800.0);
        let target = expected_rect(rect);
        let expected = panel
            .theme
            .button_hover
            .with_alpha(panel.theme.button_hover.a * 1.8);
        let mut backend = CaptureBackend::default();
        let mut cx = PaintCx {
            backend: &mut backend,
        };

        panel.paint(&mut cx, rect);

        assert!(
            backend
                .round_fills
                .iter()
                .any(|(r, color)| rect_eq(*r, target) && color_eq(*color, expected)),
            "pressed {button:?} should paint the shared pressed feedback token"
        );
    }
}
