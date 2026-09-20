//! Images tab: image-generation profiles and the Openverse OAuth
//! credential form.
//!
//! Split out of `agent_settings_tests.rs` to keep every file under the
//! repo's 800-line cap.

use super::*;

#[test]
fn image_generation_profile_buttons_add_activate_and_remove() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut().editor_ui.agent_settings.tab = AgentSettingsTab::Images;

    let panel = AgentSettingsPanel::for_editor(host.editor_state());
    let rect = panel.rect(VIEWPORT_W, VIEWPORT_H);
    let content_x = op_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .x;
    let content_y = op_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .y;
    let content_w = op_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect)
        .size
        .x;
    let gen_top = content_y + 36.0 + 24.0 + 28.0;

    assert!(host.dispatch_agent_settings_press(
        content_x + content_w - 36.0,
        gen_top + 14.0,
        VIEWPORT_W,
        VIEWPORT_H
    ));
    assert!(host.dispatch_agent_settings_press(
        content_x + content_w - 36.0,
        gen_top + 14.0,
        VIEWPORT_W,
        VIEWPORT_H
    ));

    let first = host
        .editor_state()
        .editor_ui
        .agent_settings
        .image_gen_profiles[0]
        .id
        .clone();
    let second = host
        .editor_state()
        .editor_ui
        .agent_settings
        .image_gen_profiles[1]
        .id
        .clone();
    assert_eq!(
        host.editor_state()
            .editor_ui
            .agent_settings
            .active_image_gen_profile_id
            .as_deref(),
        Some(first.as_str())
    );

    let second_row_y = gen_top + 36.0 + 8.0 + 32.0 + 6.0;
    assert!(host.dispatch_agent_settings_press(
        content_x + 23.0,
        second_row_y + 16.0,
        VIEWPORT_W,
        VIEWPORT_H
    ));
    assert_eq!(
        host.editor_state()
            .editor_ui
            .agent_settings
            .active_image_gen_profile_id
            .as_deref(),
        Some(second.as_str())
    );

    assert!(host.dispatch_agent_settings_press(
        content_x + content_w - 12.0,
        second_row_y + 16.0,
        VIEWPORT_W,
        VIEWPORT_H
    ));
    assert_eq!(
        host.editor_state().editor_ui.pressed_button,
        Some(ButtonPressTarget::AgentSettings(
            AgentSettingsButton::ImageProfileRemove(1)
        ))
    );
    assert!(host.apply_release_with_viewport(VIEWPORT_W, VIEWPORT_H));
    assert_eq!(host.editor_state().editor_ui.pressed_button, None);

    let settings = &host.editor_state().editor_ui.agent_settings;
    assert_eq!(settings.image_gen_profiles.len(), 1);
    assert_eq!(
        settings.active_image_gen_profile_id.as_deref(),
        Some(first.as_str())
    );
}

#[test]
fn image_generation_profile_focus_accepts_text_and_commits() {
    let mut host = WidgetHostNative::new();
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
        .settings_input
        .set_text("");

    for c in "Hero Images".chars() {
        assert!(host.apply_text(c));
    }
    assert!(host.apply_send());

    let settings = &host.editor_state().editor_ui.agent_settings;
    assert_eq!(settings.image_gen_profiles[0].name, "Hero Images");
    assert!(settings.focus.is_none());
    assert!(host
        .editor_state()
        .editor_ui
        .settings_input
        .text()
        .is_empty());
}

#[test]
fn editing_browser_owned_image_profile_transfers_it_to_operator_ownership() {
    let mut host = WidgetHostNative::new();
    let settings = &mut host.editor_state_mut().editor_ui.agent_settings;
    settings.add_image_gen_profile();
    settings.image_gen_profiles[0].id = "web-credential:image:igp-web-1".into();
    settings.active_image_gen_profile_id = Some("web-credential:image:igp-web-1".into());
    settings.next_image_gen_profile_id = 4;
    settings.focus = Some(SettingsFocus::ImageGenProfile {
        index: 0,
        field: ImageGenField::Name,
    });
    host.editor_state_mut()
        .editor_ui
        .settings_input
        .set_text("Operator Images");

    assert!(host.apply_send());

    let settings = &host.editor_state().editor_ui.agent_settings;
    assert_eq!(settings.image_gen_profiles[0].id, "igp-4");
    assert_eq!(settings.image_gen_profiles[0].name, "Operator Images");
    assert_eq!(
        settings.active_image_gen_profile_id.as_deref(),
        Some("igp-4")
    );
    assert_eq!(settings.next_image_gen_profile_id, 5);
}

#[test]
fn image_generation_add_press_sets_and_release_clears_agent_settings_button() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut().editor_ui.agent_settings.tab = AgentSettingsTab::Images;

    let panel = AgentSettingsPanel::for_editor(host.editor_state());
    let rect = panel.rect(VIEWPORT_W, VIEWPORT_H);
    let content_x = op_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .x;
    let content_y = op_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .y;
    let content_w = op_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect)
        .size
        .x;
    let gen_top = content_y + 36.0 + 24.0 + 28.0;
    let add_x = content_x + content_w - 36.0;
    let add_y = gen_top + 18.0;

    assert!(host.dispatch_agent_settings_press(add_x, add_y, VIEWPORT_W, VIEWPORT_H));
    assert_eq!(
        host.editor_state().editor_ui.pressed_button,
        Some(ButtonPressTarget::AgentSettings(
            AgentSettingsButton::ImageGenAdd
        ))
    );

    assert!(host.apply_release_with_viewport(VIEWPORT_W, VIEWPORT_H));
    assert_eq!(host.editor_state().editor_ui.pressed_button, None);
}

#[test]
fn image_generation_provider_click_opens_menu_without_changing_profile() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut().editor_ui.agent_settings.tab = AgentSettingsTab::Images;
    host.editor_state_mut()
        .editor_ui
        .agent_settings
        .add_image_gen_profile();
    host.editor_state_mut()
        .editor_ui
        .agent_settings
        .image_gen_profiles[0]
        .model = "dall-e-3".into();
    host.editor_state_mut().editor_ui.agent_settings.focus = Some(SettingsFocus::ImageGenProfile {
        index: 0,
        field: ImageGenField::Name,
    });

    let panel = AgentSettingsPanel::for_editor(host.editor_state());
    let rect = panel.rect(VIEWPORT_W, VIEWPORT_H);
    let content_x = op_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .x;
    let content_y = op_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .y;
    let gen_top = content_y + 36.0 + 24.0 + 28.0;
    let row_y = gen_top + 36.0 + 8.0;
    let provider_y = row_y + 32.0 + 8.0 + 36.0;

    assert!(host.dispatch_agent_settings_press(
        content_x + 110.0 + 20.0,
        provider_y + 12.0,
        VIEWPORT_W,
        VIEWPORT_H
    ));

    let profile = &host
        .editor_state()
        .editor_ui
        .agent_settings
        .image_gen_profiles[0];
    assert_eq!(profile.provider, ImageGenProvider::OpenAi);
    assert_eq!(profile.model, "dall-e-3");
    assert_eq!(
        host.editor_state().editor_ui.agent_settings.focus,
        Some(SettingsFocus::ImageGenProfile {
            index: 0,
            field: ImageGenField::Name,
        })
    );
    assert_eq!(
        host.editor_state()
            .editor_ui
            .agent_settings
            .image_gen_provider_menu_open,
        Some(0)
    );
    assert_eq!(
        host.editor_state().editor_ui.pressed_button,
        Some(ButtonPressTarget::AgentSettings(
            AgentSettingsButton::ImageProfileProvider(0)
        ))
    );
    assert!(host.apply_release_with_viewport(VIEWPORT_W, VIEWPORT_H));
    assert_eq!(host.editor_state().editor_ui.pressed_button, None);

    assert!(host.dispatch_agent_settings_press(
        content_x + 110.0 + 20.0,
        provider_y + 60.0,
        VIEWPORT_W,
        VIEWPORT_H
    ));
    let settings = &host.editor_state().editor_ui.agent_settings;
    let profile = &settings.image_gen_profiles[0];
    assert_eq!(profile.provider, ImageGenProvider::OpenAi);
    assert_eq!(profile.model, "dall-e-3");
    assert_eq!(settings.image_gen_provider_menu_open, Some(0));
    assert_eq!(
        host.editor_state().editor_ui.pressed_button,
        Some(ButtonPressTarget::AgentSettings(
            AgentSettingsButton::ImageProviderOption {
                index: 0,
                provider: ImageGenProvider::Gemini,
            },
        ))
    );

    assert!(host.apply_release_with_viewport(VIEWPORT_W, VIEWPORT_H));

    let settings = &host.editor_state().editor_ui.agent_settings;
    let profile = &settings.image_gen_profiles[0];
    assert_eq!(profile.provider, ImageGenProvider::Gemini);
    assert!(profile.model.is_empty());
    assert!(settings.image_gen_provider_menu_open.is_none());
    assert_eq!(host.editor_state().editor_ui.pressed_button, None);
    assert_eq!(
        settings.focus,
        Some(SettingsFocus::ImageGenProfile {
            index: 0,
            field: ImageGenField::Name,
        })
    );
}

#[test]
fn image_generation_profile_header_click_toggles_editor_closed() {
    let mut host = WidgetHostNative::new();
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
        .settings_input
        .set_text("Config 1");

    let panel = AgentSettingsPanel::for_editor(host.editor_state());
    let rect = panel.rect(VIEWPORT_W, VIEWPORT_H);
    let content_x = op_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .x;
    let content_y = op_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .y;
    let gen_top = content_y + 36.0 + 24.0 + 28.0;
    let row_y = gen_top + 36.0 + 8.0;

    assert!(host.dispatch_agent_settings_press(
        content_x + 72.0,
        row_y + 16.0,
        VIEWPORT_W,
        VIEWPORT_H
    ));
    assert_eq!(
        host.editor_state().editor_ui.pressed_button,
        Some(ButtonPressTarget::AgentSettings(
            AgentSettingsButton::ImageProfileHeader(0)
        ))
    );

    assert_eq!(host.editor_state().editor_ui.agent_settings.focus, None);
    assert!(host
        .editor_state()
        .editor_ui
        .settings_input
        .text()
        .is_empty());
    assert!(host.apply_release_with_viewport(VIEWPORT_W, VIEWPORT_H));
    assert_eq!(host.editor_state().editor_ui.pressed_button, None);
}

#[test]
fn image_search_oauth_focus_accepts_text_and_commits() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut().editor_ui.agent_settings.tab = AgentSettingsTab::Images;
    host.editor_state_mut().editor_ui.agent_settings.focus =
        Some(SettingsFocus::ImageSearch(ImageSearchField::ClientId));
    host.editor_state_mut()
        .editor_ui
        .settings_input
        .set_text("");

    for c in "openverse-client".chars() {
        assert!(host.apply_text(c));
    }
    assert!(host.apply_send());

    host.editor_state_mut().editor_ui.agent_settings.focus =
        Some(SettingsFocus::ImageSearch(ImageSearchField::ClientSecret));
    host.editor_state_mut()
        .editor_ui
        .settings_input
        .set_text("");
    for c in "openverse-secret".chars() {
        assert!(host.apply_text(c));
    }
    assert!(host.apply_send());

    let settings = &host.editor_state().editor_ui.agent_settings;
    assert_eq!(settings.openverse_client_id, "openverse-client");
    assert_eq!(settings.openverse_client_secret, "openverse-secret");
    assert!(settings.focus.is_none());
    assert!(host
        .editor_state()
        .editor_ui
        .settings_input
        .text()
        .is_empty());
}

#[test]
fn image_search_test_tracks_invalid_and_testing_status_like_ts() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut().editor_ui.agent_settings.tab = AgentSettingsTab::Images;
    host.editor_state_mut()
        .editor_ui
        .agent_settings
        .images_advanced_open = true;
    host.editor_state_mut()
        .editor_ui
        .agent_settings
        .openverse_client_id = "client".into();

    let panel = AgentSettingsPanel::for_editor(host.editor_state());
    let rect = panel.rect(VIEWPORT_W, VIEWPORT_H);
    let content_y = op_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .y;
    let content_w = op_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect)
        .size
        .x;
    let x = op_editor_ui::widgets::agent_settings_panel::secondary_tab_body(rect)
        .origin
        .x
        + content_w
        - 28.0;
    let y = content_y + 36.0 + 24.0 + 22.0 + 36.0 + 10.0 + 36.0 + 14.0 + 18.0;

    assert!(host.dispatch_agent_settings_press(x, y, VIEWPORT_W, VIEWPORT_H));
    assert_eq!(
        host.editor_state()
            .editor_ui
            .agent_settings
            .images_search_test_status,
        ImageTestStatus::Invalid
    );
    assert!(
        host.editor_state()
            .editor_ui
            .agent_settings
            .images_search_ready
    );

    host.editor_state_mut()
        .editor_ui
        .agent_settings
        .openverse_client_secret = "secret".into();
    assert!(host.dispatch_agent_settings_press(x, y, VIEWPORT_W, VIEWPORT_H));
    assert_eq!(
        host.editor_state()
            .editor_ui
            .agent_settings
            .images_search_test_status,
        ImageTestStatus::Testing
    );
    assert!(
        host.editor_state()
            .editor_ui
            .agent_settings
            .images_search_ready
    );
    assert_eq!(
        host.editor_state().editor_ui.pressed_button,
        Some(ButtonPressTarget::AgentSettings(
            AgentSettingsButton::ImageSearchTest
        ))
    );

    assert!(host.apply_release_with_viewport(VIEWPORT_W, VIEWPORT_H));
    assert_eq!(host.editor_state().editor_ui.pressed_button, None);
}
