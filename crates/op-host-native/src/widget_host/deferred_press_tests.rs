use super::WidgetHostNative;
use op_editor_core::agent_settings::{
    AgentSettingsTab, ImageGenField, ImageGenProvider, SettingsFocus,
};
use op_editor_core::chat::{AgentProvider, ModelEntry};
use op_editor_core::{
    AgentSettingsButton, ButtonPressTarget, GitCandidateFile, GitOverflowView, GitPanelState,
};
use op_editor_ui::widgets::agent_settings_panel::AgentSettingsPanel;
use op_editor_ui::widgets::{
    ai_chat_model_picker, AIChatPlaceholder, GitPanel, LayerPanel, LayerPanelHit, PropertyPanel,
    PropertyPanelAction, TOP_BAR_HEIGHT,
};
use op_editor_ui::{Point2D, Rect};

fn seed_two_chat_models(host: &mut WidgetHostNative) {
    host.editor_state_mut()
        .chat
        .available_models
        .push(ModelEntry::new(AgentProvider::CodexCli, "gpt-5", "GPT-5"));
    host.editor_state_mut()
        .chat
        .available_models
        .push(ModelEntry::new(
            AgentProvider::Antigravity,
            "default",
            "Antigravity Default",
        ));
}

fn seed_many_chat_models(host: &mut WidgetHostNative, count: usize) {
    host.editor_state_mut().chat.available_models = (0..count)
        .map(|idx| {
            ModelEntry::new(
                AgentProvider::CodexCli,
                format!("model-{idx}"),
                format!("Model {idx}"),
            )
        })
        .collect();
}

fn seed_layer_for_context_menu(host: &mut WidgetHostNative) {
    let doc = jian_ops_schema::load_str(
        r#"{"version":"1.0.0","children":[
            {"type":"rectangle","id":"n1","name":"Layer","x":0,"y":0,"width":100,"height":50}
        ]}"#,
    )
    .expect("layer fixture parses")
    .value;
    *host.editor_state_mut() = op_editor_core::EditorState::from_document(doc);
    host.mark_paint_dirty_for_test();
}

fn first_layer_row_point(host: &WidgetHostNative, viewport_h: f32) -> Point2D {
    let rect = Rect::xywh(
        0.0,
        TOP_BAR_HEIGHT,
        host.editor_state().editor_ui.layer_panel_width,
        viewport_h - TOP_BAR_HEIGHT,
    );
    let panel = LayerPanel::from_editor(host.editor_state());
    let x = 48.0;
    let mut y = rect.origin.y + 1.0;
    while y < rect.origin.y + rect.size.y {
        let point = Point2D::new(x, y);
        if matches!(panel.hit_test(rect, point), Some(LayerPanelHit::Layer(_))) {
            return point;
        }
        y += 1.0;
    }
    panic!("layer row not found");
}

#[test]
fn chat_model_row_press_selects_and_closes_immediately() {
    let mut host = WidgetHostNative::new();
    seed_two_chat_models(&mut host);
    host.editor_state_mut().editor_ui.chat_model_picker.open = true;
    let chat_rect = host.ai_chat_rect(1200.0, 800.0).unwrap();
    let panel = AIChatPlaceholder::from_editor(host.editor_state());
    let picker = panel.model_picker_bounds(chat_rect).unwrap();
    let row_y = picker.origin.y
        + ai_chat_model_picker::MODEL_SEARCH_H
        + ai_chat_model_picker::MODEL_PICKER_PAD_Y
        + ai_chat_model_picker::MODEL_GROUP_H
        + ai_chat_model_picker::MODEL_ROW_H
        + ai_chat_model_picker::MODEL_GROUP_H
        + ai_chat_model_picker::MODEL_ROW_H / 2.0;

    assert!(host.apply_press(picker.origin.x + 24.0, row_y, 1200.0, 800.0));

    assert_eq!(host.editor_state().chat.selected_model, 1);
    assert_eq!(host.editor_state().editor_ui.chat_selected_agent, 4);
    assert!(!host.editor_state().editor_ui.chat_model_picker.open);
    assert!(!host.editor_state_dirty);
    assert_eq!(
        host.editor_state().editor_ui.chat_model_picker.pressed,
        None
    );

    assert!(!host.apply_release_with_viewport(1200.0, 800.0));
    assert_eq!(host.editor_state().chat.selected_model, 1);
}

#[test]
fn chat_model_row_outside_chat_wins_over_layer_panel() {
    let mut host = WidgetHostNative::new();
    seed_many_chat_models(&mut host, 10);
    {
        let state = host.editor_state_mut();
        state.chat.panel_height = op_editor_ui::widgets::AI_CHAT_MIN_HEIGHT;
        state.chat.panel_position = Some((0.0, 400.0));
        state.editor_ui.chat_model_picker.open = true;
        state.editor_ui.chat_model_picker.scroll.offset = 56.0;
    }
    let chat_rect = host.ai_chat_rect(1200.0, 800.0).unwrap();
    let panel = AIChatPlaceholder::from_editor(host.editor_state());
    let picker = panel.model_picker_bounds(chat_rect).unwrap();
    let point = Point2D::new(
        picker.origin.x + 24.0,
        picker.origin.y
            + ai_chat_model_picker::MODEL_SEARCH_H
            + ai_chat_model_picker::MODEL_PICKER_PAD_Y
            + ai_chat_model_picker::MODEL_GROUP_H
            - 56.0
            + ai_chat_model_picker::MODEL_ROW_H
            + ai_chat_model_picker::MODEL_ROW_H / 2.0,
    );
    assert!(!chat_rect.contains(point));
    assert!(point.x < host.editor_state().editor_ui.layer_panel_width);
    assert_eq!(
        panel.hit_test(chat_rect, point),
        Some(op_editor_ui::widgets::AIChatHit::SelectModel(1))
    );
    let selection_before = host.editor_state().selection.clone();

    assert!(host.apply_press(point.x, point.y, 1200.0, 800.0));

    assert_eq!(host.editor_state().chat.selected_model, 1);
    assert_eq!(host.editor_state().selection, selection_before);
    assert!(!host.editor_state().editor_ui.chat_model_picker.open);
}

#[test]
fn chat_model_row_over_layer_resize_gutter_wins_press() {
    let mut host = WidgetHostNative::new();
    seed_many_chat_models(&mut host, 10);
    let layer_edge = host.editor_state().editor_ui.layer_panel_width;
    {
        let state = host.editor_state_mut();
        state.chat.panel_height = op_editor_ui::widgets::AI_CHAT_MIN_HEIGHT;
        state.chat.panel_position = Some((layer_edge - 96.0, 400.0));
        state.editor_ui.chat_model_picker.open = true;
    }
    let chat = host.ai_chat_rect(1200.0, 800.0).expect("chat rect");
    let panel = AIChatPlaceholder::from_editor(host.editor_state());
    let picker = panel.model_picker_bounds(chat).expect("picker rect");
    let point = Point2D::new(
        layer_edge,
        picker.origin.y
            + ai_chat_model_picker::MODEL_SEARCH_H
            + ai_chat_model_picker::MODEL_PICKER_PAD_Y
            + ai_chat_model_picker::MODEL_GROUP_H
            + ai_chat_model_picker::MODEL_ROW_H
            + ai_chat_model_picker::MODEL_ROW_H / 2.0,
    );
    assert_eq!(
        panel.hit_test(chat, point),
        Some(op_editor_ui::widgets::AIChatHit::SelectModel(1))
    );

    assert!(host.apply_press(point.x, point.y, 1200.0, 800.0));

    assert!(!host.is_resizing_panel());
    assert_eq!(host.editor_state().chat.selected_model, 1);
    assert!(!host.editor_state().editor_ui.chat_model_picker.open);
}

#[test]
fn right_press_on_model_picker_does_not_open_covered_layer_context_menu() {
    let mut host = WidgetHostNative::new();
    seed_layer_for_context_menu(&mut host);
    seed_many_chat_models(&mut host, 10);
    let viewport = (1200.0, 800.0);
    let layer_point = first_layer_row_point(&host, viewport.1);
    {
        let state = host.editor_state_mut();
        state.chat.panel_height = op_editor_ui::widgets::AI_CHAT_MIN_HEIGHT;
        state.chat.panel_position = Some((0.0, layer_point.y + 40.0));
        state.editor_ui.chat_model_picker.open = true;
    }
    let picker = host
        .chat_model_picker_rect(viewport.0, viewport.1)
        .expect("picker rect");
    assert!(picker.contains(layer_point));

    assert!(host.apply_right_press(layer_point.x, layer_point.y, viewport.0, viewport.1));

    assert!(host.editor_state().editor_ui.layer_context_menu.is_none());
    assert!(host.editor_state().editor_ui.chat_model_picker.open);
}

#[test]
fn minimizing_chat_closes_model_picker() {
    let mut host = WidgetHostNative::new();
    seed_two_chat_models(&mut host);
    host.editor_state_mut().editor_ui.chat_model_picker.open = true;
    host.editor_state_mut()
        .editor_ui
        .chat_model_picker_input
        .set_text("gpt");
    let chat = host.ai_chat_rect(1200.0, 800.0).unwrap();

    assert!(host.apply_press(chat.origin.x + 25.0, chat.origin.y + 18.0, 1200.0, 800.0,));

    assert!(host.editor_state().chat.is_minimized());
    assert!(!host.editor_state().editor_ui.chat_model_picker.open);
    assert!(host
        .editor_state()
        .editor_ui
        .chat_model_picker_input
        .text()
        .is_empty());
}

#[test]
fn chat_model_picker_visible_over_topbar_wins_press() {
    let mut host = WidgetHostNative::new();
    seed_many_chat_models(&mut host, 10);
    host.set_now_ms(456);
    {
        let state = host.editor_state_mut();
        state.chat.anchor = op_editor_core::ChatAnchor::TopLeft;
        state.chat.panel_height = op_editor_ui::widgets::AI_CHAT_MIN_HEIGHT;
        state.editor_ui.chat_model_picker.open = true;
    }
    let chat = host.ai_chat_rect(1200.0, 800.0).unwrap();
    let panel = AIChatPlaceholder::from_editor(host.editor_state());
    let picker = panel.model_picker_bounds(chat).unwrap();
    let search_top = picker.origin.y.max(0.0);
    let search_bottom = (picker.origin.y + ai_chat_model_picker::MODEL_SEARCH_H)
        .min(op_editor_ui::widgets::TOP_BAR_HEIGHT);
    assert!(search_top < search_bottom);
    let point = Point2D::new(picker.origin.x + 24.0, (search_top + search_bottom) / 2.0);
    assert_eq!(
        panel.hit_test(chat, point),
        Some(op_editor_ui::widgets::AIChatHit::FocusModelSearch)
    );

    assert!(host.apply_press(point.x, point.y, 1200.0, 800.0));

    assert!(host.editor_state().editor_ui.chat_model_picker.open);
    assert_eq!(
        host.editor_state()
            .editor_ui
            .chat_model_picker_input
            .next_blink_flip_ms(456),
        956
    );
}

#[test]
fn image_provider_option_press_defers_selection_until_release() {
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
        .provider = ImageGenProvider::OpenAi;
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
    let rect = panel.rect(1200.0, 800.0);
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
        1200.0,
        800.0
    ));
    assert!(host.apply_release_with_viewport(1200.0, 800.0));

    assert!(host.dispatch_agent_settings_press(
        content_x + 110.0 + 20.0,
        provider_y + 60.0,
        1200.0,
        800.0
    ));

    let settings = &host.editor_state().editor_ui.agent_settings;
    assert_eq!(
        settings.image_gen_profiles[0].provider,
        ImageGenProvider::OpenAi
    );
    assert_eq!(settings.image_gen_profiles[0].model, "dall-e-3");
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

    assert!(host.apply_release_with_viewport(1200.0, 800.0));
    let settings = &host.editor_state().editor_ui.agent_settings;
    assert_eq!(
        settings.image_gen_profiles[0].provider,
        ImageGenProvider::Gemini
    );
    assert!(settings.image_gen_profiles[0].model.is_empty());
    assert!(settings.image_gen_provider_menu_open.is_none());
    assert_eq!(host.editor_state().editor_ui.pressed_button, None);
}

#[test]
fn font_weight_row_press_defers_selection_until_release() {
    let mut host = WidgetHostNative::new();
    *host.editor_state_mut() = op_editor_core::EditorState::sample();
    host.editor_state_mut().editor_ui.font_weight_picker_open = true;
    let property_rect = Rect {
        origin: Point2D::new(
            1200.0 - host.editor_state().editor_ui.property_panel_width,
            TOP_BAR_HEIGHT,
        ),
        size: Point2D::new(
            host.editor_state().editor_ui.property_panel_width,
            800.0 - TOP_BAR_HEIGHT,
        ),
    };
    let panel = PropertyPanel::for_selection(host.editor_state()).unwrap();
    let before_weight = selected_font_weight(host.editor_state());
    let (point, choice) = find_font_weight_action_point(&panel, property_rect, before_weight);

    assert!(host.dismiss_font_weight_picker_on_press(point.x, point.y, 1200.0, 800.0));

    assert_eq!(selected_font_weight(host.editor_state()), before_weight);
    assert!(host.editor_state().editor_ui.font_weight_picker_open);
    assert!(matches!(
        host.editor_state().editor_ui.pressed_button,
        Some(ButtonPressTarget::FontWeightPicker(_))
    ));

    assert!(host.apply_release_with_viewport(1200.0, 800.0));
    assert!(!host.editor_state().editor_ui.font_weight_picker_open);
    assert_eq!(host.editor_state().editor_ui.pressed_button, None);
    assert_eq!(selected_font_weight(host.editor_state()), choice.value());
}

#[test]
fn tracked_picker_row_press_defers_selection_until_release() {
    let mut host = WidgetHostNative::new();
    {
        let panel = &mut host.editor_state_mut().editor_ui.git_panel;
        *panel = GitPanelState {
            open: true,
            in_repo: true,
            branch: Some("main".into()),
            overflow_open: true,
            overflow_view: GitOverflowView::TrackedPicker,
            candidate_files: vec![candidate("a.op"), candidate("b.op")],
            ..Default::default()
        };
    }
    let (vw, vh) = (1400.0, 900.0);
    let body = host.git_panel_rect(vw, vh).expect("panel open");
    let panel = GitPanel::for_editor(host.editor_state()).unwrap();
    let point = find_tracked_picker_row_point(&panel, body, 1);

    assert!(host.dispatch_git_panel_press(point.x, point.y, vw, vh));

    let git = &host.editor_state().editor_ui.git_panel;
    assert_eq!(git.tracked_picker_selected, None);
    assert_eq!(git.tracked_picker.pressed, Some(1));

    assert!(host.apply_release_with_viewport(vw, vh));
    let git = &host.editor_state().editor_ui.git_panel;
    assert_eq!(git.tracked_picker_selected, Some(1));
    assert_eq!(git.tracked_picker.pressed, None);
}

fn find_font_weight_action_point(
    panel: &PropertyPanel,
    rect: Rect,
    current_weight: u16,
) -> (Point2D, op_editor_ui::widgets::FontWeightChoice) {
    let mut y = rect.origin.y;
    while y <= rect.origin.y + rect.size.y {
        let mut x = rect.origin.x;
        while x <= rect.origin.x + rect.size.x {
            let point = Point2D::new(x, y);
            if let Some(PropertyPanelAction::SetFontWeight(choice)) =
                panel.hit_test_action(rect, point)
            {
                if choice.value() != current_weight {
                    return (point, choice);
                }
            }
            x += 4.0;
        }
        y += 4.0;
    }
    panic!("expected font weight action point");
}

fn selected_font_weight(state: &op_editor_core::EditorState) -> u16 {
    PropertyPanel::for_selection(state)
        .and_then(|panel| panel.snapshot.text.map(|text| text.font_weight))
        .expect("selected text node")
}

fn candidate(name: &str) -> GitCandidateFile {
    GitCandidateFile {
        path: format!("/repo/{name}"),
        relative_path: name.to_string(),
        milestone_count: 0,
        last_commit_time: "now".into(),
        last_commit_message: None,
    }
}

fn find_tracked_picker_row_point(panel: &GitPanel<'_>, body: Rect, row: usize) -> Point2D {
    let mut y = body.origin.y;
    while y <= body.origin.y + body.size.y + 240.0 {
        let mut x = body.origin.x;
        while x <= body.origin.x + body.size.x {
            let point = Point2D::new(x, y);
            if panel.tracked_picker_select_hit(body, point)
                == op_editor_ui::widgets::git_panel::SelectHit::Row(row)
            {
                return point;
            }
            x += 4.0;
        }
        y += 4.0;
    }
    panic!("expected tracked picker row point");
}
