//! Hover tracking across the modal's buttons, switches and menus.
//!
//! Split out of `agent_settings_tests.rs` to keep every file under the
//! repo's 800-line cap.

use super::*;

#[test]
fn mcp_server_button_hover_tracks_cursor() {
    let mut host = WidgetHostNative::new();
    host.last_viewport_w = VIEWPORT_W;
    host.last_viewport_h = VIEWPORT_H;
    host.editor_state_mut().editor_ui.agent_settings_open = true;
    host.editor_state_mut().editor_ui.agent_settings.tab = AgentSettingsTab::Mcp;
    host.editor_state_mut()
        .editor_ui
        .agent_settings
        .mcp_server
        .running = true;

    let rect = AgentSettingsPanel::for_editor(host.editor_state()).rect(VIEWPORT_W, VIEWPORT_H);
    let button = op_editor_ui::widgets::agent_settings_panel::mcp_server_button(rect);

    assert!(host.update_agent_settings_hover(
        button.origin.x + button.size.x / 2.0,
        button.origin.y + button.size.y / 2.0,
    ));
    assert!(
        host.editor_state()
            .editor_ui
            .agent_settings
            .hover_mcp_server_button
    );
}

#[test]
fn image_settings_button_hover_tracks_cursor() {
    let mut host = WidgetHostNative::new();
    host.last_viewport_w = VIEWPORT_W;
    host.last_viewport_h = VIEWPORT_H;
    host.editor_state_mut().editor_ui.agent_settings_open = true;
    host.editor_state_mut().editor_ui.agent_settings.tab = AgentSettingsTab::Images;
    host.editor_state_mut()
        .editor_ui
        .agent_settings
        .images_advanced_open = true;

    let (content_x, content_y, content_w) = agent_settings_images_metrics(&host);
    let test_x = content_x + content_w - 28.0;
    let test_y = content_y + 196.0;
    assert!(host.update_agent_settings_hover(test_x, test_y));
    assert!(
        host.editor_state()
            .editor_ui
            .agent_settings
            .hover_image_search_test_button
    );

    let add_x = content_x + content_w - 36.0;
    let add_y = content_y + 260.0;
    assert!(host.update_agent_settings_hover(add_x, add_y));
    assert!(
        host.editor_state()
            .editor_ui
            .agent_settings
            .hover_image_gen_add_button
    );
    assert!(
        !host
            .editor_state()
            .editor_ui
            .agent_settings
            .hover_image_search_test_button
    );
}

#[test]
fn image_provider_menu_option_hover_tracks_cursor() {
    let mut host = WidgetHostNative::new();
    host.last_viewport_w = VIEWPORT_W;
    host.last_viewport_h = VIEWPORT_H;
    host.editor_state_mut().editor_ui.agent_settings_open = true;
    host.editor_state_mut().editor_ui.agent_settings.tab = AgentSettingsTab::Images;
    host.editor_state_mut()
        .editor_ui
        .agent_settings
        .add_image_gen_profile();
    host.editor_state_mut().editor_ui.agent_settings.focus = Some(SettingsFocus::ImageGenProfile {
        index: 0,
        field: ImageGenField::Name,
    });
    host.editor_state_mut()
        .editor_ui
        .agent_settings
        .image_gen_provider_menu_open = Some(0);

    let (content_x, content_y, _) = agent_settings_images_metrics(&host);
    let provider_x = content_x + 8.0 + 110.0 + 20.0;
    let provider_y = content_y + 36.0 + 24.0 + 28.0 + 36.0 + 8.0 + 32.0 + 8.0 + 36.0;

    assert!(host.update_agent_settings_hover(provider_x, provider_y + 24.0 + 2.0 * 24.0 + 12.0));

    assert_eq!(
        host.editor_state()
            .editor_ui
            .agent_settings
            .hover_image_gen_provider_option,
        Some((0, ImageGenProvider::Replicate))
    );
}
