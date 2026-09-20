//! ACP agent cards: draft form geometry, carets and hover-only actions.
//!
//! Split out of `agent_settings_panel_tests.rs` to keep that file under the
//! 800-line cap.

use super::*;
use crate::widgets::text_metrics;

fn acp_form_remote_half_point(content: Rect, section_y: f32) -> Point2D {
    let card_y = section_y + 28.0 + 28.0;
    Point2D::new(
        content.origin.x + 12.0 + (content.size.x - 24.0) * 0.75,
        card_y + 100.0 + 14.0,
    )
}

#[test]
fn agents_tab_acp_cards_replace_empty_hint_height() {
    let empty = AgentSettingsPanel::for_editor(&EditorState::default()).content_total_height();
    let mut state = EditorState::default();
    state.editor_ui.agent_settings.add_acp_agent();
    state.editor_ui.agent_settings.add_acp_agent();
    let with_acp = AgentSettingsPanel::for_editor(&state).content_total_height();

    assert!(
        with_acp > empty,
        "configured ACP agents should contribute list-card height instead of a fixed empty hint"
    );
}

#[test]
fn full_painted_custom_agent_action_rect_is_clickable_from_its_left_edge() {
    let state = EditorState::default();
    let panel = AgentSettingsPanel::for_editor(&state);
    let panel_rect = panel.rect(1200.0, 800.0);
    let content = crate::widgets::agent_settings_panel_geometry::content_rect(panel_rect);
    let section_y = crate::widgets::agent_settings_panel_geometry::acp_section_y(
        content,
        &state.editor_ui.agent_settings,
    );
    let action =
        crate::widgets::agent_settings_header_action::header_action_rect(content, section_y);

    assert_eq!(
        panel.hit_test(
            panel_rect,
            Point2D::new(action.origin.x + 1.0, action.origin.y + action.size.y / 2.0),
        ),
        AgentSettingsHit::AddAcpAgent
    );
}

#[test]
fn new_local_custom_agent_draft_has_no_visible_or_clickable_remote_choice() {
    let mut state = EditorState::default();
    state.editor_ui.agent_settings.begin_acp_agent_draft();
    let content = Rect {
        origin: Point2D::new(24.0, 32.0),
        size: Point2D::new(520.0, 0.0),
    };
    let section_y = 40.0;

    assert_eq!(
        crate::widgets::agent_settings_acp::hit_test(
            content,
            &state.editor_ui.agent_settings,
            acp_form_remote_half_point(content, section_y),
            section_y,
        ),
        crate::widgets::agent_settings_acp::AcpHit::None,
        "the old Remote half of a new Local draft must be inert"
    );

    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    crate::widgets::agent_settings_acp::paint_acp_section(
        &mut cx,
        &crate::theme::Theme::dark(),
        &state.editor_ui.agent_settings,
        &state.editor_ui,
        content,
        section_y,
        0,
    );
    let local = op_i18n::translate(state.editor_ui.locale, "acp.local");
    let remote = op_i18n::translate(state.editor_ui.locale, "acp.remote");
    assert!(
        backend
            .text_effective_points
            .iter()
            .any(|(text, _)| text == local),
        "new custom-Agent drafts should paint a read-only Local badge"
    );
    assert!(
        !backend
            .text_effective_points
            .iter()
            .any(|(text, _)| text == remote),
        "new custom-Agent drafts must not paint a Remote option"
    );
}

#[test]
fn long_german_and_spanish_acp_subtitles_are_ellipsized_within_content_width() {
    let content = Rect {
        origin: Point2D::new(24.0, 32.0),
        size: Point2D::new(472.0, 0.0),
    };
    let section_y = 40.0;
    let subtitle_baseline =
        section_y + crate::widgets::agent_settings_metrics::SECTION_HEADER_H + 14.0;

    for locale in [op_i18n::Locale::De, op_i18n::Locale::Es] {
        let mut state = EditorState::default();
        state.editor_ui.locale = locale;
        let mut backend = CaptureBackend::default();
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        crate::widgets::agent_settings_acp::paint_acp_section(
            &mut cx,
            &crate::theme::Theme::dark(),
            &state.editor_ui.agent_settings,
            &state.editor_ui,
            content,
            section_y,
            0,
        );

        let (subtitle, point) = backend
            .text_effective_points
            .iter()
            .find(|(_, point)| (point.y - subtitle_baseline).abs() < 0.01)
            .cloned()
            .expect("ACP subtitle should be painted on its fixed-height row");
        let painted_w = text_metrics::measure_chrome(&mut backend, &subtitle, 12.0);
        assert!(
            subtitle.ends_with("..."),
            "{locale:?} ACP subtitle should visibly signal truncation: {subtitle}"
        );
        assert!(
            point.x + painted_w <= content.origin.x + content.size.x + 0.01,
            "{locale:?} ACP subtitle should fit its content row"
        );
    }
}

#[test]
fn localized_acp_header_and_empty_copy_stay_inside_content_width() {
    let content = Rect {
        origin: Point2D::new(24.0, 32.0),
        size: Point2D::new(472.0, 0.0),
    };

    for locale in op_i18n::Locale::ALL {
        let mut state = EditorState::default();
        state.editor_ui.locale = locale;
        let mut backend = CaptureBackend::default();
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        crate::widgets::agent_settings_acp::paint_acp_section(
            &mut cx,
            &crate::theme::Theme::dark(),
            &state.editor_ui.agent_settings,
            &state.editor_ui,
            content,
            40.0,
            0,
        );

        // The header block is the first four runs, in paint order. The
        // quick-add rows paint after them and are measured separately in
        // `acp_presets`.
        assert!(
            backend.text_effective_points.len() >= 4,
            "{locale:?} empty ACP section should paint title, action, subtitle, and hint"
        );
        let rows: Vec<_> = backend
            .text_effective_points
            .iter()
            .take(4)
            .cloned()
            .collect();
        for ((text, point), size) in rows.iter().zip([15.0, 12.0, 12.0, 13.0]) {
            let painted_w = text_metrics::measure_chrome(&mut backend, text, size);
            assert!(
                point.x >= content.origin.x - 0.01
                    && point.x + painted_w <= content.origin.x + content.size.x + 0.01,
                "{locale:?} text overflows ACP content: {text:?}"
            );
        }
        let title_right =
            rows[0].1.x + text_metrics::measure_chrome(&mut backend, &rows[0].0, 15.0);
        assert!(
            title_right + 12.0 <= rows[1].1.x + 0.01,
            "{locale:?} ACP section title overlaps its add action"
        );
        if locale == op_i18n::Locale::De {
            assert!(
                rows[1].0.ends_with("..."),
                "the measured German add action should be safely ellipsized"
            );
        }
    }
}

#[test]
fn saved_local_custom_agent_edit_form_cannot_switch_to_remote() {
    let mut state = EditorState::default();
    state.editor_ui.agent_settings.add_acp_agent_config(
        "Local helper",
        op_editor_core::agent_settings::AcpConnectionType::Local,
        "op-agent",
        Vec::new(),
        Default::default(),
        None,
        true,
    );
    state.editor_ui.agent_settings.focus = Some(SettingsFocus::AcpAgent {
        index: 0,
        field: AcpAgentField::DisplayName,
    });
    let content = Rect {
        origin: Point2D::new(24.0, 32.0),
        size: Point2D::new(520.0, 0.0),
    };
    let section_y = 40.0;

    assert_eq!(
        crate::widgets::agent_settings_acp::hit_test(
            content,
            &state.editor_ui.agent_settings,
            acp_form_remote_half_point(content, section_y),
            section_y,
        ),
        crate::widgets::agent_settings_acp::AcpHit::None,
        "the old Remote half of a saved Local edit form must be inert"
    );
}

#[test]
fn legacy_remote_custom_agent_keeps_read_only_remote_badge() {
    let mut state = EditorState::default();
    state.editor_ui.agent_settings.add_acp_agent_config(
        "Legacy remote",
        op_editor_core::agent_settings::AcpConnectionType::Remote,
        "",
        Vec::new(),
        Default::default(),
        Some("wss://agent.example.com".into()),
        true,
    );
    state.editor_ui.agent_settings.focus = Some(SettingsFocus::AcpAgent {
        index: 0,
        field: AcpAgentField::DisplayName,
    });
    let content = Rect {
        origin: Point2D::new(24.0, 32.0),
        size: Point2D::new(520.0, 0.0),
    };
    let section_y = 40.0;

    assert_eq!(
        crate::widgets::agent_settings_acp::hit_test(
            content,
            &state.editor_ui.agent_settings,
            acp_form_remote_half_point(content, section_y),
            section_y,
        ),
        crate::widgets::agent_settings_acp::AcpHit::None,
        "legacy Remote transport should be visible but read-only"
    );

    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    crate::widgets::agent_settings_acp::paint_acp_section(
        &mut cx,
        &crate::theme::Theme::dark(),
        &state.editor_ui.agent_settings,
        &state.editor_ui,
        content,
        section_y,
        0,
    );
    let remote = op_i18n::translate(state.editor_ui.locale, "acp.remote");
    assert!(
        backend
            .text_effective_points
            .iter()
            .any(|(text, _)| text == remote),
        "legacy Remote rows should preserve their transport label"
    );
}

#[test]
fn compact_acp_card_paints_first_displayable_name_character_instead_of_transport_icon() {
    let mut state = EditorState::default();
    state.editor_ui.agent_settings.add_acp_agent_config(
        " \n自定义助手",
        op_editor_core::agent_settings::AcpConnectionType::Local,
        "op-agent",
        Vec::new(),
        Default::default(),
        None,
        true,
    );
    let content = Rect {
        origin: Point2D::new(24.0, 32.0),
        size: Point2D::new(520.0, 0.0),
    };
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    crate::widgets::agent_settings_acp::paint_acp_section(
        &mut cx,
        &crate::theme::Theme::dark(),
        &state.editor_ui.agent_settings,
        &state.editor_ui,
        content,
        40.0,
        0,
    );

    assert!(
        backend
            .text_effective_points
            .iter()
            .any(|(text, _)| text == "自"),
        "compact custom-Agent cards should use a CJK-safe display-name monogram"
    );
    assert!(
        backend.svg_strokes.is_empty(),
        "compact custom-Agent cards should not paint Terminal/Globe transport branding"
    );
}

#[test]
fn local_custom_agent_draft_still_saves_through_settings_press_flow() {
    let mut state = EditorState::default();

    crate::widgets::agent_settings_press_flow::apply_agent_settings_hit(
        &mut state,
        AgentSettingsHit::AddAcpAgent,
        op_editor_core::host_settings_commit::SettingsCommitScope::Operator,
        100,
    );
    let draft = state
        .editor_ui
        .agent_settings
        .acp_agent_draft
        .as_ref()
        .expect("add-custom-Agent should open a draft");
    assert_eq!(
        draft.connection_type,
        op_editor_core::agent_settings::AcpConnectionType::Local
    );

    state.editor_ui.settings_input.set_text("op-agent");
    crate::widgets::agent_settings_press_flow::apply_agent_settings_hit(
        &mut state,
        AgentSettingsHit::SaveAcpAgentDraft,
        op_editor_core::host_settings_commit::SettingsCommitScope::Operator,
        101,
    );

    assert!(state.editor_ui.agent_settings.acp_agent_draft.is_none());
    assert_eq!(state.editor_ui.agent_settings.acp_agents.len(), 1);
    assert_eq!(
        state.editor_ui.agent_settings.acp_agents[0].connection_type,
        op_editor_core::agent_settings::AcpConnectionType::Local
    );
    assert_eq!(
        state.editor_ui.agent_settings.acp_agents[0].command,
        "op-agent"
    );
}

#[test]
fn removing_custom_agent_through_settings_flow_invalidates_runtime_connection_state() {
    let mut state = EditorState::default();
    let id = state.editor_ui.agent_settings.add_acp_agent_config(
        "Local helper",
        op_editor_core::agent_settings::AcpConnectionType::Local,
        "op-agent",
        Vec::new(),
        Default::default(),
        None,
        true,
    );
    state
        .editor_ui
        .agent_settings
        .begin_acp_agent_connect(0)
        .expect("configured custom Agent should begin probing");

    crate::widgets::agent_settings_press_flow::apply_agent_settings_hit(
        &mut state,
        AgentSettingsHit::RemoveAcpAgent(0),
        op_editor_core::host_settings_commit::SettingsCommitScope::Operator,
        102,
    );

    assert!(state.editor_ui.agent_settings.acp_agents.is_empty());
    assert_eq!(
        state.editor_ui.agent_settings.pending_acp_agent_connect,
        None
    );
    assert!(!state
        .editor_ui
        .agent_settings
        .acp_agent_connection
        .contains_key(&id));
}

#[test]
fn agents_content_height_contains_every_provider_card() {
    let state = EditorState::default();
    let panel = AgentSettingsPanel::for_editor(&state);
    let rect = panel.rect(1200.0, 800.0);
    let content = crate::widgets::agent_settings_panel_geometry::content_rect(rect);
    let last = crate::widgets::agent_settings_panel_geometry::agent_card_rect_in(
        rect,
        AgentProvider::ALL.len() - 1,
        &state.editor_ui.agent_settings,
    );

    assert!(
        last.origin.y + last.size.y <= content.origin.y + panel.content_total_height(),
        "scrollable content must include the final CLI provider card"
    );
}

#[test]
fn acp_draft_form_reserves_room_for_args_and_env_fields() {
    let mut state = EditorState::default();
    state.editor_ui.agent_settings.begin_acp_agent_draft();

    let height =
        crate::widgets::agent_settings_acp::content_height(&state.editor_ui.agent_settings);

    assert!(
        height >= 320.0,
        "ACP draft form should include display name, connection type, command, args, env, and actions"
    );
}

#[test]
fn focused_acp_agent_field_paints_visible_caret_at_blink_on_phase() {
    let mut state = EditorState::default();
    state.editor_ui.agent_settings.add_acp_agent();
    state.editor_ui.agent_settings.focus = Some(SettingsFocus::AcpAgent {
        index: 0,
        field: AcpAgentField::Command,
    });
    state.editor_ui.settings_input.set_text("node server.js");

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
fn focused_empty_acp_command_field_paints_visible_caret_at_blink_on_phase() {
    let mut state = EditorState::default();
    state.editor_ui.agent_settings.add_acp_agent();
    state.editor_ui.agent_settings.focus = Some(SettingsFocus::AcpAgent {
        index: 0,
        field: AcpAgentField::Command,
    });
    state.editor_ui.settings_input.set_text("");

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
fn focused_acp_agent_field_hides_caret_at_blink_off_phase() {
    let mut state = EditorState::default();
    state.editor_ui.agent_settings.add_acp_agent();
    state.editor_ui.agent_settings.focus = Some(SettingsFocus::AcpAgent {
        index: 0,
        field: AcpAgentField::Command,
    });
    state.editor_ui.settings_input.set_text("node server.js");

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
fn acp_agent_compact_actions_are_hover_only_click_targets() {
    let mut state = EditorState::default();
    state.editor_ui.agent_settings.add_acp_agent();
    let panel = AgentSettingsPanel::for_editor(&state);
    let rect = panel.rect(1200.0, 800.0);
    let content_x = crate::widgets::agent_settings_panel::content_viewport(rect)
        .origin
        .x;
    let content_y = crate::widgets::agent_settings_panel::content_viewport(rect)
        .origin
        .y;
    let content_w = crate::widgets::agent_settings_panel::content_viewport(rect)
        .size
        .x;
    let card_y = content_y
        + crate::widgets::agent_settings_panel::AGENTS_HERO_HEIGHT
        + 120.0
        + 28.0
        + 28.0
        + 28.0;

    assert_eq!(
        panel.hit_test(
            rect,
            crate::Point2D::new(content_x + content_w - 156.0, card_y + 30.0)
        ),
        AgentSettingsHit::Inside
    );
    assert_eq!(
        panel.hit_test(
            rect,
            crate::Point2D::new(content_x + content_w - 128.0, card_y + 30.0)
        ),
        AgentSettingsHit::Inside
    );

    state.editor_ui.agent_settings.hover_acp_agent = 0;
    let panel = AgentSettingsPanel::for_editor(&state);
    assert_eq!(
        panel.hit_test(
            rect,
            crate::Point2D::new(content_x + content_w - 156.0, card_y + 30.0)
        ),
        AgentSettingsHit::EditAcpAgent(0)
    );
    assert_eq!(
        panel.hit_test(
            rect,
            crate::Point2D::new(content_x + content_w - 128.0, card_y + 30.0)
        ),
        AgentSettingsHit::RemoveAcpAgent(0)
    );
}
