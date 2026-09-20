use crate::widgets::agent_settings_panel::AgentSettingsPanel;
use crate::widgets::agent_settings_panel_geometry::nav_item_rect;
use crate::Rect;
use op_editor_core::agent_settings::AgentSettingsTab;
use op_editor_core::EditorState;

fn tab_center(panel: Rect, index: usize, count: usize) -> crate::Point2D {
    let pill = nav_item_rect(panel, index, count);
    crate::Point2D::new(
        pill.origin.x + pill.size.x / 2.0,
        pill.origin.y + pill.size.y / 2.0,
    )
}

#[test]
fn account_tab_gate_off_hides_nav_and_falls_back_from_stale_state() {
    let default_state = EditorState::default();
    assert!(
        !default_state.editor_ui.account_ui_available,
        "the account gate must default to off"
    );
    let default_panel = AgentSettingsPanel::for_editor(&default_state);
    let expected_agents_height = default_panel.content_total_height();

    let mut state = EditorState::default();
    state.editor_ui.agent_settings.tab = AgentSettingsTab::Account;
    let panel = AgentSettingsPanel::for_editor(&state);
    let rect = panel.rect(1200.0, 800.0);

    let expected_tabs = [
        AgentSettingsTab::Agents,
        AgentSettingsTab::Mcp,
        AgentSettingsTab::Images,
        AgentSettingsTab::Fonts,
        AgentSettingsTab::System,
    ];
    for (index, expected) in expected_tabs.into_iter().enumerate() {
        assert_eq!(
            panel.nav_at(rect, tab_center(rect, index, expected_tabs.len())),
            Some(expected)
        );
    }

    // One pill further along the same strip is past its right end.
    let hidden_account_row = tab_center(rect, expected_tabs.len(), expected_tabs.len());
    assert_eq!(panel.nav_at(rect, hidden_account_row), None);
    assert_eq!(
        panel.content_total_height(),
        expected_agents_height,
        "a persisted Account tab must fall back to the first visible tab"
    );
}

#[test]
fn account_tab_gate_on_restores_nav_row() {
    let mut state = EditorState::default();
    state.editor_ui.account_ui_available = true;
    let panel = AgentSettingsPanel::for_editor(&state);
    let rect = panel.rect(1200.0, 800.0);

    let account_row_index = AgentSettingsTab::ALL.len() - 1;
    assert_eq!(
        panel.nav_at(
            rect,
            tab_center(rect, account_row_index, AgentSettingsTab::ALL.len())
        ),
        Some(AgentSettingsTab::Account),
        "enabling the runtime gate must add the Account nav row back"
    );
}
