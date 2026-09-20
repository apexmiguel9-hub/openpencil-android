//! Quick-add rows for the ACP section — one prefilled row per
//! [`op_editor_core::acp_agent_presets::ACP_AGENT_PRESETS`] entry that is
//! not configured yet.
//!
//! Split out of `agent_settings_acp.rs` to keep both files under the
//! repo's 800-line cap. The rows are pure chrome over the ordinary ACP
//! add path: pressing one saves a normal `AcpAgentConfig` and starts the
//! same handshake a hand-typed agent gets.

use crate::theme::Theme;
use crate::widgets::agent_settings_i18n::t as t_settings;
use crate::widgets::button::paint_ghost_button_feedback;
use crate::widgets::settings_form::{draw_text, ellipsize};
use crate::widgets::text_metrics;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect, TextLayout};
use op_editor_core::acp_agent_presets::{AcpAgentPreset, AcpPresetAvailability};
use op_editor_core::agent_settings::AgentSettings;
use op_editor_core::editor_ui_state::EditorUiState;
use op_editor_core::{AgentSettingsButton, ButtonPressTarget};

use crate::widgets::agent_settings_metrics::{ROW_AVATAR, ROW_H_TWO_LINE, ROW_PAD_X, ROW_TEXT_X};
use crate::widgets::agent_settings_rows::{paint_row_hairline, paint_row_label_at};

const LABEL_H: f32 = 22.0;
/// Quick-add rows are the modal's two-line list row, like every other
/// entry — they used to be 44 px bordered cards with a 6 px gap, which
/// made the block read as a second, denser design inside the section.
const ROW_H: f32 = ROW_H_TWO_LINE;
const ROW_GAP: f32 = 0.0;
const MONOGRAM: f32 = ROW_AVATAR;
const ADD_BTN_W: f32 = 64.0;
const ADD_BTN_H: f32 = 24.0;

/// Height of the whole quick-add block, `0.0` once every preset has been
/// added — a section that has served its purpose stops taking up space.
pub fn block_height(settings: &AgentSettings) -> f32 {
    let count = settings.visible_acp_presets().len();
    if count == 0 {
        return 0.0;
    }
    LABEL_H + count as f32 * (ROW_H + ROW_GAP)
}

/// Index into the *visible* preset rows under `point`, if any. The index
/// is positional, so callers must resolve it against
/// [`AgentSettings::visible_acp_presets`] from the same frame.
pub fn row_at(content: Rect, settings: &AgentSettings, point: Point2D, y: f32) -> Option<usize> {
    let count = settings.visible_acp_presets().len();
    if count == 0 {
        return None;
    }
    let mut row_y = y + LABEL_H;
    for index in 0..count {
        if (row_rect(content, row_y)).contains(point) {
            return Some(index);
        }
        row_y += ROW_H + ROW_GAP;
    }
    None
}

/// Index of the row whose Add button is under `point`.
///
/// The whole row is the target, not just the button: a 64×24 hit area is
/// needlessly fussy for a row that has exactly one action, and pressing
/// the name is the same intent as pressing the button.
pub fn hit_test(content: Rect, settings: &AgentSettings, point: Point2D, y: f32) -> Option<usize> {
    row_at(content, settings, point, y)
}

pub fn paint(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    ui: &EditorUiState,
    content: Rect,
    y: f32,
) -> f32 {
    let presets = settings.visible_acp_presets();
    if presets.is_empty() {
        return y;
    }
    draw_text(
        cx,
        t_settings(ui, "settings.agents.acpQuickAdd"),
        11.0,
        theme.muted_foreground,
        content.origin.x,
        y + 14.0,
    );
    let mut row_y = y + LABEL_H;
    for (index, preset) in presets.iter().enumerate() {
        paint_row(
            cx,
            theme,
            settings,
            ui,
            preset,
            index,
            row_rect(content, row_y),
        );
        row_y += ROW_H + ROW_GAP;
    }
    row_y
}

#[allow(clippy::too_many_arguments)]
fn paint_row(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    ui: &EditorUiState,
    preset: &AcpAgentPreset,
    index: usize,
    row: Rect,
) {
    let hovered = settings.hover_acp_preset == Some(index);
    let missing = settings.acp_preset_availability(preset.id) == AcpPresetAvailability::Missing;
    if hovered {
        cx.backend.fill_round_rect(row, 8.0, theme.button_hover);
    }
    let last = index + 1 == settings.visible_acp_presets().len();
    if !last {
        paint_row_hairline(cx, theme, row);
    }

    // A missing binary mutes the row's identity but never removes the
    // action. PATH was read once, on this host, at some earlier moment —
    // treating that snapshot as authority would lock the user out of a CLI
    // they installed two minutes ago, and the handshake reports the real
    // answer anyway.
    paint_monogram(cx, theme, preset.display_name, row, missing);

    // The install hint replaces the command preview rather than the Add
    // label: the button still works, so labelling it "Not installed"
    // would describe the CLI while looking like it describes the action.
    let detail = if missing {
        format!(
            "{} · {}",
            t_settings(ui, "settings.agents.acpNotInstalled"),
            preset.install_hint
        )
    } else {
        format!("{} {}", preset.command, preset.args.join(" "))
    };
    let text_x = row.origin.x + ROW_TEXT_X;
    let reserved = ADD_BTN_W + ROW_PAD_X;
    paint_row_label_at(
        cx,
        theme,
        row,
        text_x,
        preset.display_name,
        Some(&detail),
        reserved,
    );

    let btn = add_button_rect(row);
    paint_ghost_button_feedback(
        cx.backend,
        theme,
        btn,
        hovered,
        ui.button_pressed(ButtonPressTarget::AgentSettings(
            AgentSettingsButton::AcpAddPreset(index),
        )),
    );
    let label = ellipsize(
        cx,
        t_settings(ui, "settings.agents.acpPresetAdd"),
        ADD_BTN_W - 8.0,
        11.0,
    );
    let label_w = text_metrics::measure_chrome(cx.backend, &label, 11.0);
    draw_text(
        cx,
        &label,
        11.0,
        theme.primary,
        btn.origin.x + (btn.size.x - label_w) / 2.0,
        jian_widgets::centered_text_baseline_y(btn, 11.0),
    );
}

fn paint_monogram(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    display_name: &str,
    row: Rect,
    missing: bool,
) {
    let badge = Rect {
        origin: Point2D::new(
            row.origin.x + ROW_PAD_X,
            row.origin.y + (row.size.y - MONOGRAM) / 2.0,
        ),
        size: Point2D::new(MONOGRAM, MONOGRAM),
    };
    cx.backend.fill_round_rect(badge, 8.0, theme.background);
    let monogram = super::agent_settings_acp::agent_monogram(display_name);
    let monogram_w = text_metrics::measure_chrome_weighted(cx.backend, &monogram, 13.0, 600);
    let color = if missing {
        theme.muted_foreground
    } else {
        theme.foreground
    };
    let layout =
        TextLayout::single_run(&monogram, "system-ui", 13.0, color.to_jian(), Point2D::ZERO)
            .with_font_weight(600);
    cx.backend.draw_text(
        &layout,
        Point2D::new(
            badge.origin.x + (badge.size.x - monogram_w) / 2.0,
            jian_widgets::centered_text_baseline_y(badge, 13.0),
        ),
    );
}

fn row_rect(content: Rect, y: f32) -> Rect {
    Rect {
        origin: Point2D::new(content.origin.x, y),
        size: Point2D::new(content.size.x, ROW_H),
    }
}

fn add_button_rect(row: Rect) -> Rect {
    Rect {
        origin: Point2D::new(
            row.origin.x + row.size.x - ROW_PAD_X - ADD_BTN_W,
            row.origin.y + (row.size.y - ADD_BTN_H) / 2.0,
        ),
        size: Point2D::new(ADD_BTN_W, ADD_BTN_H),
    }
}
