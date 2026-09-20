//! The chat footer's thinking-mode toggle — a bare 🧠 button that cycles
//! [`ChatState::thinking_mode`] Adaptive → Disabled → Enabled → Adaptive.
//!
//! The backend has always honoured `thinking_mode` (every provider maps it
//! onto its own wire shape), but the only control that ever set it was a
//! 92 px chip in a controls row no paint path called — so the setting was
//! reachable from code and from nowhere else. The footer had no room for
//! that chip next to the model pill, ⚡Nx, 📎 and send, so the affordance is
//! an icon button instead and the three states are carried by the glyph:
//!
//! | Mode     | Reads as                                            |
//! | -------- | --------------------------------------------------- |
//! | Adaptive | muted brain — same weight as its neighbours          |
//! | Enabled  | accent-coloured brain — deliberately on              |
//! | Disabled | dimmed brain with a slash — deliberately off         |
//!
//! The slash is what makes Disabled legible: colour alone would put the
//! whole distinction on a muted-vs-muted-dimmer step that neither a
//! colour-blind reader nor a light theme reliably resolves. `EyeOff` in the
//! layer panel earns its keep the same way.
//!
//! Note the state this paints is the *user's* choice. A model whose profile
//! is `thinking_disabled` (glm-5.x, MiniMax) is forced off at turn launch by
//! `design_turn_thinking_mode` no matter what this shows — see the report
//! note on surfacing that here.

use crate::theme::Theme;
use crate::widgets::ai_chat_panel_controls::chat_neutral_feedback_color;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect};
use op_editor_core::chat::ThinkingMode;

/// Painted glyph size inside the button's slot.
pub(crate) const THINKING_ICON_SIZE: f32 = 14.0;

/// Translation key for the toggle's tooltip in `mode`.
pub(crate) fn thinking_tooltip_key(mode: ThinkingMode) -> &'static str {
    match mode {
        ThinkingMode::Adaptive => "ai.thinking.adaptive",
        ThinkingMode::Disabled => "ai.thinking.disabled",
        ThinkingMode::Enabled => "ai.thinking.enabled",
    }
}

/// Glyph colour for `mode`.
pub(crate) fn thinking_icon_color(theme: &Theme, mode: ThinkingMode) -> Color {
    match mode {
        ThinkingMode::Adaptive => theme.muted_foreground,
        ThinkingMode::Enabled => theme.primary,
        ThinkingMode::Disabled => (theme.muted_foreground).with_alpha(0.45),
    }
}

/// Whether `mode` paints the "off" slash across the glyph.
pub(crate) fn thinking_shows_slash(mode: ThinkingMode) -> bool {
    matches!(mode, ThinkingMode::Disabled)
}

/// Paint the toggle into its footer slot. A zero-width slot means the row
/// dropped the control (see `footer_layout`) and nothing is painted.
pub(crate) fn paint_thinking_toggle(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    rect: Rect,
    mode: ThinkingMode,
    hovered: bool,
    pressed: bool,
) {
    if rect.size.x <= 0.0 {
        return;
    }
    if hovered || pressed {
        cx.backend
            .fill_round_rect(rect, 6.0, chat_neutral_feedback_color(theme, pressed));
    }
    let color = thinking_icon_color(theme, mode);
    let origin = Point2D::new(
        rect.origin.x + (rect.size.x - THINKING_ICON_SIZE) / 2.0,
        rect.origin.y + (rect.size.y - THINKING_ICON_SIZE) / 2.0,
    );
    draw_icon(
        cx.backend,
        Icon::Brain,
        origin,
        THINKING_ICON_SIZE,
        color,
        1.4,
    );
    if thinking_shows_slash(mode) {
        cx.backend.stroke_line(
            Point2D::new(origin.x + 1.0, origin.y + THINKING_ICON_SIZE - 1.0),
            Point2D::new(origin.x + THINKING_ICON_SIZE - 1.0, origin.y + 1.0),
            color,
            1.4,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::test_capture_backend::CaptureBackend;

    fn theme() -> Theme {
        Theme::dark()
    }

    #[test]
    fn every_mode_has_its_own_tooltip_key() {
        let keys = [
            thinking_tooltip_key(ThinkingMode::Adaptive),
            thinking_tooltip_key(ThinkingMode::Disabled),
            thinking_tooltip_key(ThinkingMode::Enabled),
        ];
        for key in keys {
            assert!(
                op_i18n::translate(op_editor_core::Locale::EnUs, key) != key,
                "`{key}` must resolve to a real English string"
            );
        }
        assert_eq!(
            keys.iter().collect::<std::collections::BTreeSet<_>>().len(),
            3,
            "each mode needs a distinct tooltip"
        );
    }

    #[test]
    fn the_three_modes_are_visually_distinct() {
        // Adaptive vs Enabled differ by colour; Disabled additionally carries
        // the slash, so it does not rely on the colour step alone.
        let theme = theme();
        let adaptive = thinking_icon_color(&theme, ThinkingMode::Adaptive);
        let enabled = thinking_icon_color(&theme, ThinkingMode::Enabled);
        let disabled = thinking_icon_color(&theme, ThinkingMode::Disabled);
        assert_ne!(
            (adaptive.r, adaptive.g, adaptive.b),
            (enabled.r, enabled.g, enabled.b)
        );
        assert!(disabled.a < adaptive.a);
        assert!(thinking_shows_slash(ThinkingMode::Disabled));
        assert!(!thinking_shows_slash(ThinkingMode::Adaptive));
        assert!(!thinking_shows_slash(ThinkingMode::Enabled));
    }

    #[test]
    fn hover_paints_the_neutral_wash_and_rest_paints_none() {
        let theme = theme();
        let rect = Rect::xywh(100.0, 200.0, 24.0, 24.0);
        for (hovered, pressed, expected) in [(false, false, 0), (true, false, 1), (true, true, 1)] {
            let mut backend = CaptureBackend::default();
            let mut cx = PaintCx {
                backend: &mut backend,
            };
            paint_thinking_toggle(
                &mut cx,
                &theme,
                rect,
                ThinkingMode::Adaptive,
                hovered,
                pressed,
            );
            assert_eq!(
                backend.round_fills.len(),
                expected,
                "hovered={hovered} pressed={pressed}"
            );
        }
    }

    #[test]
    fn a_dropped_slot_paints_nothing() {
        // The narrow-panel degradation hands back a zero-width rect; painting
        // a 14 px glyph into it would put a brain on top of the ⚡ chip.
        let mut backend = CaptureBackend::default();
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        paint_thinking_toggle(
            &mut cx,
            &theme(),
            Rect::xywh(100.0, 200.0, 0.0, 24.0),
            ThinkingMode::Enabled,
            true,
            false,
        );
        assert!(backend.round_fills.is_empty());
    }
}
