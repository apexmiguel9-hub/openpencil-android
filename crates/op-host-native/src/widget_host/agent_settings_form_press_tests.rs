use super::WidgetHostNative;
use op_editor_core::{AgentSettingsButton, ButtonPressTarget};
use op_editor_ui::widgets::agent_settings_panel::AgentSettingsPanel;

/// The settings modal is a wide, tall workspace; these fixtures press
/// absolute rects deep in the Agents tab, so they run in a window big
/// enough to keep the whole body above the fold.
const VIEWPORT_W: f32 = 1200.0;
const VIEWPORT_H: f32 = 1000.0;

fn content_metrics(host: &WidgetHostNative) -> (f32, f32, f32) {
    let panel = AgentSettingsPanel::for_editor(host.editor_state());
    let rect = panel.rect(VIEWPORT_W, VIEWPORT_H);
    (
        op_editor_ui::widgets::agent_settings_panel::content_viewport(rect)
            .origin
            .x,
        op_editor_ui::widgets::agent_settings_panel::content_viewport(rect)
            .origin
            .y,
        op_editor_ui::widgets::agent_settings_panel::content_viewport(rect)
            .size
            .x,
    )
}

fn builtin_draft_card_y(content_y: f32) -> f32 {
    content_y + op_editor_ui::widgets::agent_settings_panel::AGENTS_HERO_HEIGHT + 28.0 + 28.0
}

fn acp_draft_card_y(content_y: f32) -> f32 {
    content_y
        + op_editor_ui::widgets::agent_settings_panel::AGENTS_HERO_HEIGHT
        + 28.0
        + 28.0
        + 64.0
        + 28.0
        + 28.0
        + 28.0
}

#[test]
fn builtin_draft_save_press_sets_and_release_clears_agent_settings_button() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut()
        .editor_ui
        .agent_settings
        .begin_builtin_agent_draft();
    host.editor_state_mut()
        .editor_ui
        .agent_settings
        .builtin_agent_draft
        .as_mut()
        .expect("draft should exist")
        .api_key = "sk-test".into();
    let (content_x, content_y, content_w) = content_metrics(&host);

    assert!(host.dispatch_agent_settings_press(
        content_x + content_w - 12.0 - 34.0,
        builtin_draft_card_y(content_y) + 196.0 + 18.0,
        VIEWPORT_W,
        VIEWPORT_H,
    ));
    assert_eq!(
        host.editor_state().editor_ui.pressed_button,
        Some(ButtonPressTarget::AgentSettings(
            AgentSettingsButton::BuiltinSaveDraft
        ))
    );

    assert!(host.apply_release_with_viewport(VIEWPORT_W, VIEWPORT_H));
    assert_eq!(host.editor_state().editor_ui.pressed_button, None);
}

#[test]
fn builtin_draft_cancel_press_sets_and_release_clears_agent_settings_button() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut()
        .editor_ui
        .agent_settings
        .begin_builtin_agent_draft();
    let (content_x, content_y, content_w) = content_metrics(&host);

    assert!(host.dispatch_agent_settings_press(
        content_x + content_w - 12.0 - 68.0 - 8.0 - 34.0,
        builtin_draft_card_y(content_y) + 196.0 + 18.0,
        VIEWPORT_W,
        VIEWPORT_H,
    ));
    assert_eq!(
        host.editor_state().editor_ui.pressed_button,
        Some(ButtonPressTarget::AgentSettings(
            AgentSettingsButton::BuiltinCancelDraft
        ))
    );

    assert!(host.apply_release_with_viewport(VIEWPORT_W, VIEWPORT_H));
    assert_eq!(host.editor_state().editor_ui.pressed_button, None);
}

#[test]
fn acp_draft_save_press_sets_and_release_clears_agent_settings_button() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut()
        .editor_ui
        .agent_settings
        .begin_acp_agent_draft();
    host.editor_state_mut()
        .editor_ui
        .agent_settings
        .acp_agent_draft
        .as_mut()
        .expect("draft should exist")
        .command = "op-agent".into();
    let (content_x, content_y, content_w) = content_metrics(&host);

    assert!(host.dispatch_agent_settings_press(
        content_x + content_w - 12.0 - 34.0,
        acp_draft_card_y(content_y) + 332.0 + 18.0,
        VIEWPORT_W,
        VIEWPORT_H,
    ));
    assert_eq!(
        host.editor_state().editor_ui.pressed_button,
        Some(ButtonPressTarget::AgentSettings(
            AgentSettingsButton::AcpSaveDraft
        ))
    );

    assert!(host.apply_release_with_viewport(VIEWPORT_W, VIEWPORT_H));
    assert_eq!(host.editor_state().editor_ui.pressed_button, None);
}

#[test]
fn acp_draft_cancel_press_sets_and_release_clears_agent_settings_button() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut()
        .editor_ui
        .agent_settings
        .begin_acp_agent_draft();
    let (content_x, content_y, content_w) = content_metrics(&host);

    assert!(host.dispatch_agent_settings_press(
        content_x + content_w - 12.0 - 68.0 - 8.0 - 34.0,
        acp_draft_card_y(content_y) + 332.0 + 18.0,
        VIEWPORT_W,
        VIEWPORT_H,
    ));
    assert_eq!(
        host.editor_state().editor_ui.pressed_button,
        Some(ButtonPressTarget::AgentSettings(
            AgentSettingsButton::AcpCancelDraft
        ))
    );

    assert!(host.apply_release_with_viewport(VIEWPORT_W, VIEWPORT_H));
    assert_eq!(host.editor_state().editor_ui.pressed_button, None);
}
