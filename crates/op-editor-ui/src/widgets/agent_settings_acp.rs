//! ACP Agent section for the Agent settings panel.

use crate::theme::Theme;
use crate::widgets::agent_settings_acp_helpers::{
    connection_type_rect, field_input_rect, form_actions_y, form_card_h,
};
use crate::widgets::agent_settings_acp_presets as acp_presets;
use crate::widgets::agent_settings_caret::settings_input_text;
use crate::widgets::agent_settings_form_actions::{
    cancel_button_rect, paint_form_actions, save_button_rect,
};
use crate::widgets::agent_settings_header_action::{
    fit_header_copy, header_action_rect, header_action_text_baseline_y, header_action_text_x,
};
use crate::widgets::agent_settings_i18n::t as t_settings;
use crate::widgets::agent_settings_metrics::{self as metrics, ROW_AVATAR, ROW_PAD_X, ROW_TEXT_X};
use crate::widgets::agent_settings_rows::{
    fit_text, paint_row_hairline, paint_row_label_above_status_at, paint_row_status_line_at_fitted,
    paint_section_title, row_text_budget, ROW_LABEL_BASELINE, ROW_LABEL_FONT,
};
use crate::widgets::button::{paint_ghost_button_feedback, tokens_from_theme};
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::settings_form::{self, draw_text, ellipsize, paint_action};
use crate::widgets::text_metrics;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect, TextLayout};
use jian_widgets::components::button::{Button, ButtonVariant};
use jian_widgets::components::card::Card;
use op_editor_core::agent_settings::{
    AcpAgentConfig, AcpAgentConnectPhase, AcpAgentField, AcpConnectionType, AgentSettings,
    SettingsFocus,
};
use op_editor_core::editor_ui_state::EditorUiState;
use op_editor_core::{AgentSettingsButton, ButtonPressTarget};

const HEADER_H: f32 = metrics::SECTION_HEADER_H;
const SUBTITLE_H: f32 = metrics::SECTION_SUBTITLE_H;
const EMPTY_H: f32 = metrics::EMPTY_BLOCK_H;
/// A saved agent that is not being edited is a list row, not a card.
const COMPACT_CARD_H: f32 = metrics::ROW_H_TWO_LINE;
/// Rows sit flush — the hairline between them IS the gap.
const CARD_GAP: f32 = 0.0;
const ACTION_W: f32 = 24.0;
const CONNECT_BTN_W: f32 = 96.0;
const CONNECT_BTN_H: f32 = 28.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcpHit {
    AddAgent,
    /// Positional index into the visible quick-add preset rows.
    AddPreset(usize),
    Focus {
        index: usize,
        field: AcpAgentField,
    },
    FocusDraft(AcpAgentField),
    SaveDraft,
    CancelDraft,
    Edit(usize),
    Remove(usize),
    ToggleConnected(usize),
    None,
}

pub fn content_height(settings: &AgentSettings) -> f32 {
    HEADER_H + SUBTITLE_H + list_height(settings) + acp_presets::block_height(settings)
}

/// Height of the saved-card list (or the empty hint that stands in for it).
fn list_height(settings: &AgentSettings) -> f32 {
    let has_draft = settings.acp_agent_draft.is_some();
    if settings.acp_agents.is_empty() && !has_draft {
        return EMPTY_H;
    }
    let saved_h: f32 = settings
        .acp_agents
        .iter()
        .enumerate()
        .map(|(index, _)| card_height(settings, index) + CARD_GAP)
        .sum();
    saved_h
        + match settings.acp_agent_draft.as_ref() {
            Some(draft) => form_card_h(draft.connection_type) + CARD_GAP,
            None => 0.0,
        }
}

/// Y of the first saved card.
fn cards_y(y: f32) -> f32 {
    y + HEADER_H + SUBTITLE_H
}

/// Y of the quick-add block — *below* the saved cards, deliberately.
///
/// Putting it above would make every saved card's geometry depend on how
/// many presets are still unconfigured, so adding one would reflow the
/// list the user is looking at. Below, the block only ever shrinks off
/// the bottom.
fn presets_y(settings: &AgentSettings, y: f32) -> f32 {
    cards_y(y) + list_height(settings)
}

pub fn hit_test(content: Rect, settings: &AgentSettings, point: Point2D, y: f32) -> AcpHit {
    if (add_agent_rect(content, y)).contains(point) {
        return AcpHit::AddAgent;
    }
    if let Some(index) = acp_presets::hit_test(content, settings, point, presets_y(settings, y)) {
        return AcpHit::AddPreset(index);
    }
    let mut card_y = cards_y(y);
    for (index, agent) in settings.acp_agents.iter().enumerate() {
        let card = card_rect(
            content.origin.x,
            card_y,
            content.size.x,
            card_height(settings, index),
        );
        if is_editing(settings, index) {
            for field in form_fields(agent.connection_type) {
                if (field_input_rect(card, *field)).contains(point) {
                    return AcpHit::Focus {
                        index,
                        field: *field,
                    };
                }
            }
        } else if settings.hover_acp_agent == index && (compact_edit_rect(card)).contains(point) {
            return AcpHit::Edit(index);
        } else if settings.hover_acp_agent == index && (compact_remove_rect(card)).contains(point) {
            return AcpHit::Remove(index);
        } else if (connection_button_rect(card)).contains(point) {
            return AcpHit::ToggleConnected(index);
        }
        card_y += card.size.y + CARD_GAP;
    }
    if let Some(agent) = settings.acp_agent_draft.as_ref() {
        let actions_y = form_actions_y(agent.connection_type);
        let card = card_rect(
            content.origin.x,
            card_y,
            content.size.x,
            form_card_h(agent.connection_type),
        );
        for field in form_fields(agent.connection_type) {
            if (field_input_rect(card, *field)).contains(point) {
                return AcpHit::FocusDraft(*field);
            }
        }
        if (save_button_rect(card, actions_y)).contains(point) {
            return AcpHit::SaveDraft;
        }
        if (cancel_button_rect(card, actions_y)).contains(point) {
            return AcpHit::CancelDraft;
        }
    }
    AcpHit::None
}

pub fn card_at(
    content: Rect,
    settings: &AgentSettings,
    point: Point2D,
    section_y: f32,
) -> Option<usize> {
    settings_form::card_index_at(
        content.origin.x,
        content.size.x,
        cards_y(section_y),
        CARD_GAP,
        (0..settings.acp_agents.len()).map(|index| card_height(settings, index)),
        point,
    )
}

/// Hover resolution for the quick-add rows — the twin of [`card_at`] that
/// both hosts call from their cursor-move ladder.
pub fn preset_row_at(
    content: Rect,
    settings: &AgentSettings,
    point: Point2D,
    section_y: f32,
) -> Option<usize> {
    acp_presets::row_at(content, settings, point, presets_y(settings, section_y))
}

pub fn paint_acp_section(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    ui: &EditorUiState,
    content: Rect,
    y: f32,
    now_ms: u64,
) -> f32 {
    let mut y = paint_header(
        cx,
        theme,
        t_settings(ui, "settings.agents.acp"),
        t_settings(ui, "settings.agents.addAcp"),
        content,
        y,
        settings.hover_add_acp_agent,
        ui.button_pressed(ButtonPressTarget::AgentSettings(
            AgentSettingsButton::AddAcpAgent,
        )),
    );
    y = settings_form::paint_subtitle(
        cx,
        theme,
        t_settings(ui, "settings.agents.acpSubtitle"),
        content.origin.x,
        y,
        content.size.x,
    );
    if settings.acp_agents.is_empty() && settings.acp_agent_draft.is_none() {
        let y = settings_form::paint_empty(
            cx,
            theme,
            t_settings(ui, "settings.agents.acpEmpty"),
            content.origin.x,
            y,
            content.size.x,
        );
        return acp_presets::paint(cx, theme, settings, ui, content, y);
    }
    for (index, agent) in settings.acp_agents.iter().enumerate() {
        let card = card_rect(
            content.origin.x,
            y,
            content.size.x,
            card_height(settings, index),
        );
        paint_acp_card(cx, theme, settings, ui, agent, index, card, now_ms);
        y += card.size.y + CARD_GAP;
    }
    if let Some(draft) = settings.acp_agent_draft.as_ref() {
        let card = card_rect(
            content.origin.x,
            y,
            content.size.x,
            form_card_h(draft.connection_type),
        );
        paint_acp_form(cx, theme, settings, ui, draft, None, card, now_ms);
        paint_form_actions(
            cx,
            theme,
            ui,
            card,
            form_actions_y(draft.connection_type),
            ui.acp_agent_draft_ready(),
            ui.button_pressed(ButtonPressTarget::AgentSettings(
                AgentSettingsButton::AcpCancelDraft,
            )),
            ui.button_pressed(ButtonPressTarget::AgentSettings(
                AgentSettingsButton::AcpSaveDraft,
            )),
        );
        y += card.size.y + CARD_GAP;
    }
    acp_presets::paint(cx, theme, settings, ui, content, y)
}

#[allow(clippy::too_many_arguments)]
fn paint_header(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    title: &str,
    action: &str,
    content: Rect,
    y: f32,
    action_hover: bool,
    action_pressed: bool,
) -> f32 {
    let copy = fit_header_copy(cx, title, action, content.size.x);
    paint_section_title(cx, theme, content.origin.x, y, &copy.title);
    let action_rect = header_action_rect(content, y);
    paint_ghost_button_feedback(cx.backend, theme, action_rect, action_hover, action_pressed);
    draw_text(
        cx,
        &copy.action,
        12.0,
        theme.primary,
        header_action_text_x(action_rect, copy.action_w),
        header_action_text_baseline_y(action_rect),
    );
    y + HEADER_H
}

#[allow(clippy::too_many_arguments)]
fn paint_acp_card(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    ui: &EditorUiState,
    agent: &AcpAgentConfig,
    index: usize,
    card: Rect,
    now_ms: u64,
) {
    if is_editing(settings, index) {
        paint_acp_form(cx, theme, settings, ui, agent, Some(index), card, now_ms);
    } else {
        paint_compact_acp_card(cx, theme, settings, ui, agent, index, card);
    }
}

/// A saved ACP agent, not being edited: a hairline-separated list row —
/// monogram, name over its connection detail, Connect on the right. The
/// tinted, bordered card it used to be spoke a different visual language
/// from the rest of the modal and stood 60 px tall to do it.
fn paint_compact_acp_card(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    ui: &EditorUiState,
    agent: &AcpAgentConfig,
    index: usize,
    card: Rect,
) {
    let hovered = settings.hover_acp_agent == index;
    if hovered {
        cx.backend.fill_round_rect(card, 8.0, theme.button_hover);
    }
    let last = index + 1 == settings.acp_agents.len() && settings.acp_agent_draft.is_none();
    if !last {
        paint_row_hairline(cx, theme, card);
    }
    paint_avatar(cx, theme, agent, card);

    let text_x = card.origin.x + ROW_TEXT_X;
    let reserved = card.origin.x + card.size.x - compact_edit_rect(card).origin.x;
    // The transport label rides after the name on the same line, so the
    // name gets whatever the label does not need.
    let kind_label = connection_type_label(ui, agent.connection_type);
    let kind_w = text_metrics::measure_chrome(cx.backend, kind_label, 10.0) + 8.0;
    paint_row_label_above_status_at(
        cx,
        theme,
        card,
        text_x,
        &agent.display_name,
        reserved + kind_w,
    );
    // Re-fit the name against the SAME budget the label painter used, so
    // the badge lands right after the ellipsized name rather than after
    // the name it would have painted under a slightly different rule.
    let name = fit_text(
        cx,
        &agent.display_name,
        row_text_budget(card, text_x, reserved + kind_w),
        ROW_LABEL_FONT,
    );
    let name_w = text_metrics::measure_chrome(cx.backend, &name, ROW_LABEL_FONT);
    draw_text(
        cx,
        kind_label,
        10.0,
        theme.muted_foreground,
        text_x + name_w + 8.0,
        card.origin.y + ROW_LABEL_BASELINE,
    );
    let detail_color = match settings.acp_agent_connection_for(&agent.id).phase {
        AcpAgentConnectPhase::Connected => theme.status_success,
        AcpAgentConnectPhase::Error => theme.destructive,
        _ => theme.muted_foreground,
    };
    paint_row_status_line_at_fitted(
        cx,
        card,
        text_x,
        &acp_detail(settings, agent),
        detail_color,
        reserved,
    );

    if hovered {
        paint_action(
            cx,
            theme,
            compact_edit_rect(card),
            Icon::Pencil,
            theme.muted_foreground,
            ui.button_pressed(ButtonPressTarget::AgentSettings(
                AgentSettingsButton::AcpEdit(index),
            )),
        );
        paint_action(
            cx,
            theme,
            compact_remove_rect(card),
            Icon::Trash,
            theme.muted_foreground,
            ui.button_pressed(ButtonPressTarget::AgentSettings(
                AgentSettingsButton::AcpRemove(index),
            )),
        );
    }
    paint_connection_button(cx, theme, settings, ui, agent, index, card);
}

#[allow(clippy::too_many_arguments)]
fn paint_acp_form(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    ui: &EditorUiState,
    agent: &AcpAgentConfig,
    index: Option<usize>,
    card: Rect,
    now_ms: u64,
) {
    Card {
        fill: Some(theme.muted),
        border: Some(theme.border),
        radius: 10.0,
    }
    .paint(cx.backend, card, &tokens_from_theme(theme));
    draw_text(
        cx,
        t_settings(ui, "acp.connectionType"),
        11.0,
        theme.muted_foreground,
        card.origin.x + 12.0,
        connection_type_rect(card).origin.y - 8.0,
    );
    paint_connection_type_badge(cx, theme, ui, agent, card);
    for field in form_fields(agent.connection_type) {
        paint_field(cx, theme, settings, ui, agent, index, *field, card, now_ms);
    }
}

fn paint_avatar(cx: &mut PaintCx<'_>, theme: &Theme, agent: &AcpAgentConfig, card: Rect) {
    let avatar = Rect {
        origin: Point2D::new(
            card.origin.x + ROW_PAD_X,
            card.origin.y + (card.size.y - ROW_AVATAR) / 2.0,
        ),
        size: Point2D::new(ROW_AVATAR, ROW_AVATAR),
    };
    cx.backend.fill_round_rect(avatar, 8.0, theme.background);
    let monogram = agent_monogram(&agent.display_name);
    let monogram_w = text_metrics::measure_chrome_weighted(cx.backend, &monogram, 15.0, 600);
    let layout = TextLayout::single_run(
        &monogram,
        "system-ui",
        15.0,
        theme.foreground.to_jian(),
        Point2D::ZERO,
    )
    .with_font_weight(600);
    cx.backend.draw_text(
        &layout,
        Point2D::new(
            avatar.origin.x + (avatar.size.x - monogram_w) / 2.0,
            jian_widgets::centered_text_baseline_y(avatar, 15.0),
        ),
    );
}

pub(crate) fn agent_monogram(display_name: &str) -> String {
    display_name
        .chars()
        .find(|ch| !ch.is_whitespace() && !ch.is_control())
        .map(|ch| ch.to_uppercase().collect())
        .unwrap_or_else(|| "?".to_string())
}

fn paint_connection_button(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    ui: &EditorUiState,
    agent: &AcpAgentConfig,
    index: usize,
    card: Rect,
) {
    let btn = connection_button_rect(card);
    let phase = settings.acp_agent_connection_for(&agent.id).phase;
    let probing = phase == AcpAgentConnectPhase::Probing;
    let connected = settings.acp_agent_verified_connected(&agent.id);
    let enabled = !probing && (agent.ready() || connected);
    if connected {
        cx.backend.fill_round_rect(btn, 6.0, theme.muted);
        cx.backend.stroke_round_rect(btn, 6.0, theme.border, 1.0);
    } else {
        Button {
            label: "",
            icon_paths: None,
            variant: if enabled {
                ButtonVariant::Primary
            } else {
                ButtonVariant::Outline
            },
            enabled: true,
            hovered: !enabled,
            pressed: false,
            font_size: 12.0,
        }
        .paint(cx.backend, btn, &tokens_from_theme(theme));
    }
    if ui.button_pressed(ButtonPressTarget::AgentSettings(
        AgentSettingsButton::AcpConnection(index),
    )) {
        paint_ghost_button_feedback(cx.backend, theme, btn, false, true);
    }
    let fg = if connected {
        theme.destructive
    } else if enabled || probing {
        theme.primary_foreground
    } else {
        theme.muted_foreground
    };
    let label = if connected {
        t_settings(ui, "settings.agents.disconnect")
    } else if probing {
        "Connecting..."
    } else if agent.ready() {
        t_settings(ui, "settings.agents.connect")
    } else {
        "Configure"
    };
    let lw = text_metrics::measure_chrome(cx.backend, label, 12.0);
    draw_text(
        cx,
        label,
        12.0,
        fg,
        btn.origin.x + (btn.size.x - lw) / 2.0,
        btn.origin.y + 18.0,
    );
}

#[allow(clippy::too_many_arguments)]
fn paint_field(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    ui: &EditorUiState,
    agent: &AcpAgentConfig,
    index: Option<usize>,
    field: AcpAgentField,
    card: Rect,
    now_ms: u64,
) {
    let focus = match index {
        Some(index) => SettingsFocus::AcpAgent { index, field },
        None => SettingsFocus::AcpAgentDraft(field),
    };
    let focused = settings.focus == Some(focus);
    let fallback = match field {
        AcpAgentField::DisplayName => agent.display_name.clone(),
        AcpAgentField::Command => agent.command.clone(),
        AcpAgentField::Args => agent.args_text(),
        AcpAgentField::Env => agent.env_text(),
        AcpAgentField::Url => agent.url.clone().unwrap_or_default(),
    };
    let value = settings_input_text(settings, ui, focus, &fallback);
    let label = match field {
        AcpAgentField::DisplayName => t_settings(ui, "acp.displayName"),
        AcpAgentField::Command => t_settings(ui, "acp.command"),
        AcpAgentField::Args => t_settings(ui, "acp.args"),
        AcpAgentField::Env => t_settings(ui, "acp.env"),
        AcpAgentField::Url => "URL",
    };
    let input = field_input_rect(card, field);
    let placeholder = match field {
        AcpAgentField::DisplayName => t_settings(ui, "acp.displayNamePlaceholder"),
        AcpAgentField::Command => t_settings(ui, "acp.commandPlaceholder"),
        AcpAgentField::Args => t_settings(ui, "acp.argsPlaceholder"),
        AcpAgentField::Env => t_settings(ui, "acp.envPlaceholder"),
        AcpAgentField::Url => "https://agent.example.com",
    };
    let text_x = input.origin.x + 6.0;
    let painted_focused = settings_form::paint_field_frame(
        cx,
        theme,
        ui,
        label,
        card.origin.x + 12.0,
        input.origin.y - 8.0,
        input,
        focused,
        placeholder,
        now_ms,
    );
    if !painted_focused {
        let shown = if value.is_empty() { placeholder } else { value };
        let text_color = if value.is_empty() {
            theme.muted_foreground
        } else {
            theme.foreground
        };
        let clipped = ellipsize(cx, shown, input.size.x - 12.0, 11.0);
        draw_text(
            cx,
            &clipped,
            11.0,
            text_color,
            text_x,
            input.origin.y + 16.0,
        );
    }
}

fn paint_connection_type_badge(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    ui: &EditorUiState,
    agent: &AcpAgentConfig,
    card: Rect,
) {
    let r = connection_type_rect(card);
    cx.backend.fill_round_rect(r, 6.0, theme.card);
    cx.backend.stroke_round_rect(r, 6.0, theme.border, 1.0);
    let kind = agent.connection_type;
    let label = connection_type_label(ui, kind);
    let icon = match kind {
        AcpConnectionType::Local => Icon::Terminal,
        AcpConnectionType::Remote => Icon::Globe,
    };
    let tw = text_metrics::measure_chrome(cx.backend, label, 11.0);
    let group_w = 16.0 + 6.0 + tw;
    let group_x = r.origin.x + (r.size.x - group_w) / 2.0;
    draw_icon(
        cx.backend,
        icon,
        Point2D::new(group_x, r.origin.y + 6.0),
        14.0,
        theme.muted_foreground,
        1.5,
    );
    draw_text(
        cx,
        label,
        11.0,
        theme.muted_foreground,
        group_x + 22.0,
        r.origin.y + 18.0,
    );
}

fn acp_detail(settings: &AgentSettings, agent: &AcpAgentConfig) -> String {
    let conn = settings.acp_agent_connection_for(&agent.id);
    match conn.phase {
        AcpAgentConnectPhase::Probing => return "Connecting to ACP agent...".into(),
        AcpAgentConnectPhase::Connected => {
            if let Some(info) = conn.info {
                return info;
            }
        }
        AcpAgentConnectPhase::Error => {
            if let Some(error) = conn.error {
                return error;
            }
        }
        AcpAgentConnectPhase::Idle => {}
    }
    match agent.connection_type {
        AcpConnectionType::Local if agent.command.trim().is_empty() => "Command required".into(),
        AcpConnectionType::Local if agent.args.is_empty() => agent.command.clone(),
        AcpConnectionType::Local => format!("{} {}", agent.command, agent.args.join(" ")),
        AcpConnectionType::Remote => agent.url.clone().unwrap_or_else(|| "URL required".into()),
    }
}

fn connection_type_label(ui: &EditorUiState, kind: AcpConnectionType) -> &'static str {
    match kind {
        AcpConnectionType::Local => t_settings(ui, "acp.local"),
        AcpConnectionType::Remote => t_settings(ui, "acp.remote"),
    }
}

fn form_fields(kind: AcpConnectionType) -> &'static [AcpAgentField] {
    const LOCAL: &[AcpAgentField] = &[
        AcpAgentField::DisplayName,
        AcpAgentField::Command,
        AcpAgentField::Args,
        AcpAgentField::Env,
    ];
    const REMOTE: &[AcpAgentField] = &[AcpAgentField::DisplayName, AcpAgentField::Url];
    match kind {
        AcpConnectionType::Local => LOCAL,
        AcpConnectionType::Remote => REMOTE,
    }
}

fn is_editing(settings: &AgentSettings, index: usize) -> bool {
    matches!(
        settings.focus,
        Some(SettingsFocus::AcpAgent { index: i, .. }) if i == index
    )
}

fn card_height(settings: &AgentSettings, index: usize) -> f32 {
    if is_editing(settings, index) {
        // A saved card being edited shows the fields but NO Save/Cancel row, so
        // it wraps the fields only (form_actions_y), not the draft's action tail.
        form_actions_y(settings.acp_agents[index].connection_type)
    } else {
        COMPACT_CARD_H
    }
}

fn add_agent_rect(content: Rect, y: f32) -> Rect {
    header_action_rect(content, y)
}

fn card_rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect {
        origin: Point2D::new(x, y),
        size: Point2D::new(w, h),
    }
}

fn compact_edit_rect(card: Rect) -> Rect {
    Rect {
        origin: Point2D::new(
            connection_button_rect(card).origin.x - 8.0 - ACTION_W * 2.0 - 4.0,
            card.origin.y + (card.size.y - ACTION_W) / 2.0,
        ),
        size: Point2D::new(ACTION_W, ACTION_W),
    }
}

fn compact_remove_rect(card: Rect) -> Rect {
    Rect {
        origin: Point2D::new(
            compact_edit_rect(card).origin.x + ACTION_W + 4.0,
            card.origin.y + (card.size.y - ACTION_W) / 2.0,
        ),
        size: Point2D::new(ACTION_W, ACTION_W),
    }
}

fn connection_button_rect(card: Rect) -> Rect {
    Rect {
        origin: Point2D::new(
            card.origin.x + card.size.x - ROW_PAD_X - CONNECT_BTN_W,
            card.origin.y + (card.size.y - CONNECT_BTN_H) / 2.0,
        ),
        size: Point2D::new(CONNECT_BTN_W, CONNECT_BTN_H),
    }
}

// `draw_text` / `ellipsize` / `paint_action` / the field-frame chrome live in
// the shared `settings_form` module; geometry helpers in `agent_settings_acp_helpers`.
