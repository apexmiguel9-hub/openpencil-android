use crate::theme::Theme;
use crate::widgets::button::tokens_from_theme;
use crate::widgets::PaintCx;
use crate::Rect;
use jian_widgets::components::switch::Switch;

pub(super) const SETTINGS_SWITCH_W: f32 = 36.0;
pub(super) const SETTINGS_SWITCH_H: f32 = 20.0;

/// Paint the settings switch via the canonical jian `Switch` (pill track,
/// white round knob). `enabled` here is the switch's ON state — the
/// control is always interactive, so jian's `enabled` is fixed `true`.
///
/// The ON track reads `status_success` rather than `primary`: across the
/// settings modal a switch answers "is this on / connected", which is the
/// same green the status pills and the MCP running dot use, and it keeps
/// primary reserved for actions. jian's `Switch` paints its ON track from
/// the `primary` token, so the green is injected by overriding that one
/// token instead of forking the component. This is the only caller of
/// jian's `Switch` in the workspace, so every settings toggle moves
/// together and nothing outside the modal is touched.
pub(super) fn paint_settings_switch(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    rect: Rect,
    enabled: bool,
) {
    let mut tokens = tokens_from_theme(theme);
    tokens.primary = theme.status_success;
    Switch {
        on: enabled,
        enabled: true,
        hovered: false,
        pressed: false,
    }
    .paint(cx.backend, rect, &tokens);
}
