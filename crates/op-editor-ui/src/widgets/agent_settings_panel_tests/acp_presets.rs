//! Quick-add preset rows in the ACP section: geometry, press flow, and
//! the "already configured" and "not installed" states.

use super::*;
use crate::widgets::text_metrics;
use op_editor_core::acp_agent_presets::ACP_AGENT_PRESETS;
use op_editor_core::agent_settings::AcpAgentConnectPhase;

/// Centre of visible quick-add row `row`, in panel coordinates.
fn preset_row_point(state: &EditorState, panel_rect: Rect, row: usize) -> Point2D {
    let content = crate::widgets::agent_settings_panel_geometry::content_rect(panel_rect);
    let section_y = crate::widgets::agent_settings_panel_geometry::acp_section_y(
        content,
        &state.editor_ui.agent_settings,
    );
    // Mirrors `agent_settings_acp::presets_y` + the row walk: header +
    // subtitle + the empty hint, then the block label, then the rows.
    let block_y = section_y + 28.0 + 28.0 + 64.0;
    let row_y = block_y + 22.0 + row as f32 * (44.0 + 6.0) + 22.0;
    Point2D::new(
        content.origin.x + content.size.x / 2.0,
        row_y - state.editor_ui.agent_settings.scroll_y.offset,
    )
}

#[test]
fn empty_acp_section_offers_one_quick_add_row_per_preset() {
    let state = EditorState::default();
    let panel = AgentSettingsPanel::for_editor(&state);
    let panel_rect = panel.rect(1200.0, 900.0);

    for row in 0..ACP_AGENT_PRESETS.len() {
        assert_eq!(
            panel.hit_test(panel_rect, preset_row_point(&state, panel_rect, row)),
            AgentSettingsHit::AddAcpPreset(row),
            "quick-add row {row} should be hit-testable"
        );
    }
}

#[test]
fn pressing_a_quick_add_row_saves_the_preset_and_starts_the_handshake() {
    let mut state = EditorState::default();
    let panel = AgentSettingsPanel::for_editor(&state);
    let panel_rect = panel.rect(1200.0, 900.0);
    let point = preset_row_point(&state, panel_rect, 0);
    let hit = panel.hit_test(panel_rect, point);
    assert_eq!(hit, AgentSettingsHit::AddAcpPreset(0));

    crate::widgets::agent_settings_press_flow::apply_agent_settings_hit(
        &mut state,
        hit,
        op_editor_core::host_settings_commit::SettingsCommitScope::Operator,
        0,
    );

    let settings = &state.editor_ui.agent_settings;
    let expected = &ACP_AGENT_PRESETS[0];
    assert_eq!(settings.acp_agents.len(), 1);
    let saved = &settings.acp_agents[0];
    assert_eq!(saved.id, expected.id);
    assert_eq!(saved.command, expected.command);
    assert_eq!(saved.args, expected.args);
    assert!(saved.enabled);
    assert!(
        !saved.connected,
        "a preset must earn `connected` from the probe, never from being added"
    );
    assert_eq!(
        settings.acp_agent_connection_for(expected.id).phase,
        AcpAgentConnectPhase::Probing,
        "adding a preset should enter the ordinary ACP handshake"
    );
}

#[test]
fn an_added_preset_stops_offering_its_quick_add_row() {
    let mut state = EditorState::default();
    let added = state
        .editor_ui
        .agent_settings
        .add_acp_agent_preset(ACP_AGENT_PRESETS[1].id);
    assert!(added.is_some());

    let visible = state.editor_ui.agent_settings.visible_acp_presets();
    assert_eq!(visible.len(), ACP_AGENT_PRESETS.len() - 1);
    assert!(
        !visible
            .iter()
            .any(|preset| preset.id == ACP_AGENT_PRESETS[1].id),
        "a configured preset should not be offered again"
    );
}

#[test]
fn a_second_press_on_the_same_preset_cannot_duplicate_the_card() {
    let mut state = EditorState::default();
    let preset_id = ACP_AGENT_PRESETS[0].id;
    assert!(state
        .editor_ui
        .agent_settings
        .add_acp_agent_preset(preset_id)
        .is_some());
    assert_eq!(
        state
            .editor_ui
            .agent_settings
            .add_acp_agent_preset(preset_id),
        None,
        "re-adding a configured preset must be refused, not appended"
    );
    assert_eq!(state.editor_ui.agent_settings.acp_agents.len(), 1);
}

#[test]
fn a_hand_typed_duplicate_also_hides_the_quick_add_row() {
    let mut state = EditorState::default();
    let preset = &ACP_AGENT_PRESETS[2];
    state.editor_ui.agent_settings.add_acp_agent_config(
        "My Qwen",
        op_editor_core::AcpConnectionType::Local,
        preset.command,
        preset.args.iter().map(|arg| arg.to_string()).collect(),
        Default::default(),
        None,
        true,
    );

    assert!(
        !state
            .editor_ui
            .agent_settings
            .visible_acp_presets()
            .iter()
            .any(|visible| visible.id == preset.id),
        "an agent with the preset's exact transport already covers it"
    );
}

#[test]
fn quick_add_rows_renumber_after_one_is_added() {
    let mut state = EditorState::default();
    // Take the first preset, so what was row 1 becomes row 0.
    state
        .editor_ui
        .agent_settings
        .add_acp_agent_preset(ACP_AGENT_PRESETS[0].id);
    let panel = AgentSettingsPanel::for_editor(&state);
    let panel_rect = panel.rect(1200.0, 900.0);

    let hit = panel.hit_test(panel_rect, preset_row_point(&state, panel_rect, 0));
    assert_eq!(hit, AgentSettingsHit::AddAcpPreset(0));

    crate::widgets::agent_settings_press_flow::apply_agent_settings_hit(
        &mut state,
        hit,
        op_editor_core::host_settings_commit::SettingsCommitScope::Operator,
        0,
    );

    let ids: Vec<_> = state
        .editor_ui
        .agent_settings
        .acp_agents
        .iter()
        .map(|agent| agent.id.as_str())
        .collect();
    assert_eq!(
        ids,
        vec![ACP_AGENT_PRESETS[0].id, ACP_AGENT_PRESETS[1].id],
        "row 0 after the first add is the SECOND preset — the index is \
         positional over visible rows, not over the preset table"
    );
}

#[test]
fn every_quick_add_row_paints_inside_the_content_width_in_every_locale() {
    let content = Rect {
        origin: Point2D::new(24.0, 32.0),
        size: Point2D::new(472.0, 0.0),
    };
    for locale in op_i18n::Locale::ALL {
        let mut state = EditorState::default();
        state.editor_ui.locale = locale;
        // Mark every preset missing so the longest detail line (the
        // install hint) is the one measured.
        for preset in &ACP_AGENT_PRESETS {
            state
                .editor_ui
                .agent_settings
                .acp_preset_installed
                .insert(preset.id.to_string(), false);
        }
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

        // Skip the four header-block runs; everything after is quick-add,
        // in paint order: the block label, then per row the monogram, the
        // display name, the detail line, and the Add label.
        let runs: Vec<_> = backend
            .text_effective_points
            .iter()
            .skip(4)
            .cloned()
            .collect();
        let mut sizes = vec![11.0_f32];
        for _ in 0..ACP_AGENT_PRESETS.len() {
            sizes.extend([13.0, 13.0, 10.0, 11.0]);
        }
        assert_eq!(
            runs.len(),
            sizes.len(),
            "{locale:?} quick-add block painted an unexpected run count — \
             update the size sequence alongside the layout"
        );
        for ((text, point), size) in runs.iter().zip(sizes) {
            let painted_w = text_metrics::measure_chrome(&mut backend, text, size);
            assert!(
                point.x >= content.origin.x - 0.01
                    && point.x + painted_w <= content.origin.x + content.size.x + 0.01,
                "{locale:?} quick-add text overflows the ACP content row: {text:?}"
            );
        }
    }
}

#[test]
fn a_missing_binary_neither_hides_the_row_nor_blocks_the_press() {
    let mut state = EditorState::default();
    let preset_id = ACP_AGENT_PRESETS[0].id;
    state
        .editor_ui
        .agent_settings
        .acp_preset_installed
        .insert(preset_id.to_string(), false);

    assert_eq!(
        state
            .editor_ui
            .agent_settings
            .acp_preset_availability(preset_id),
        op_editor_core::AcpPresetAvailability::Missing
    );
    assert!(
        state
            .editor_ui
            .agent_settings
            .visible_acp_presets()
            .iter()
            .any(|preset| preset.id == preset_id),
        "PATH is a snapshot, not an authority — a missing binary must not \
         remove the row"
    );
    assert!(
        state
            .editor_ui
            .agent_settings
            .add_acp_agent_preset(preset_id)
            .is_some(),
        "the user may have installed the CLI since the probe ran"
    );
}

#[test]
fn availability_is_unknown_until_a_host_actually_looks() {
    let state = EditorState::default();
    assert_eq!(
        state
            .editor_ui
            .agent_settings
            .acp_preset_availability(ACP_AGENT_PRESETS[0].id),
        op_editor_core::AcpPresetAvailability::Unknown,
        "a host that cannot read PATH must not imply the CLI is missing"
    );
}
