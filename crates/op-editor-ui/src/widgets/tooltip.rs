//! Shared tooltip chrome — the small popover-coloured label that hangs
//! off a hovered control.
//!
//! Two layers, because the two callers need different amounts of it:
//!
//! - [`paint_tooltip_frame`] paints the surface + text into a rect the
//!   caller placed. The property panel's corner editor owns a bespoke
//!   placement rule (right-aligned above its anchor) that predates this
//!   module, so it keeps its own geometry and shares only the chrome —
//!   single-sourcing the look without moving a single pixel.
//! - [`paint_tooltip`] adds the placement rule the top bar wants:
//!   horizontally centred on the anchor, above or below it, clamped
//!   into the viewport so a button at either window edge still paints
//!   its whole tooltip.

use crate::theme::Theme;
use crate::widgets::text_metrics;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect, TextLayout};

/// Box height. 22 px holds 10 pt text with comfortable optical padding.
pub const TOOLTIP_HEIGHT: f32 = 22.0;
/// Label font size. Deliberately a step below the chrome's 11 pt so the
/// tooltip reads as an annotation rather than another chrome label.
pub const TOOLTIP_FONT_SIZE: f32 = 10.0;
pub const TOOLTIP_RADIUS: f32 = 5.0;
/// Padding either side of the text run.
pub const TOOLTIP_PAD_X: f32 = 7.0;
/// Gap between the label and its trailing shortcut hint.
pub const TOOLTIP_SHORTCUT_GAP: f32 = 8.0;
/// Distance between the anchor's edge and the tooltip's near edge.
pub const TOOLTIP_ANCHOR_GAP: f32 = 4.0;
/// Minimum distance the clamped tooltip keeps from the viewport edge.
pub const TOOLTIP_VIEWPORT_INSET: f32 = 4.0;

/// Which side of the anchor the tooltip hangs off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TooltipPlacement {
    /// Above the anchor — for controls at the bottom of the window.
    Above,
    /// Below the anchor — what the top bar uses.
    Below,
}

/// Painted width of `label` plus an optional trailing shortcut hint.
///
/// This is the bubble's own width, so it MUST be measured in the family the
/// runs below paint with ([`text_metrics::CHROME_FONT_FAMILY`]). Measured
/// family-blind, the bubble is born narrower than its label and the longest
/// top-bar tooltip runs out past its own rounded edge.
pub fn tooltip_width(cx: &mut PaintCx<'_>, label: &str, shortcut: Option<&str>) -> f32 {
    let mut text = text_metrics::measure_chrome(cx.backend, label, TOOLTIP_FONT_SIZE);
    if let Some(shortcut) = shortcut {
        text += TOOLTIP_SHORTCUT_GAP
            + text_metrics::measure_chrome(cx.backend, shortcut, TOOLTIP_FONT_SIZE);
    }
    text + TOOLTIP_PAD_X * 2.0
}

/// Place a `width`-wide tooltip against `anchor`.
///
/// `clamp_x` is the horizontal band the tooltip must stay inside
/// (usually the viewport, already inset). Without it the tooltip is
/// centred on the anchor whatever that costs.
pub fn tooltip_rect(
    width: f32,
    anchor: Rect,
    placement: TooltipPlacement,
    clamp_x: Option<(f32, f32)>,
) -> Rect {
    let y = match placement {
        TooltipPlacement::Above => anchor.origin.y - TOOLTIP_ANCHOR_GAP - TOOLTIP_HEIGHT,
        TooltipPlacement::Below => anchor.origin.y + anchor.size.y + TOOLTIP_ANCHOR_GAP,
    };
    let mut x = anchor.origin.x + (anchor.size.x - width) / 2.0;
    if let Some((min_x, max_x)) = clamp_x {
        // `max` guards the degenerate case where the tooltip is wider
        // than the band: the left edge wins so the label's start stays
        // readable instead of both ends running off-screen.
        x = x.clamp(min_x, (max_x - width).max(min_x));
    }
    Rect::xywh(x, y, width, TOOLTIP_HEIGHT)
}

/// Paint the tooltip surface and its text into an already-placed rect.
///
/// `baseline_y` is the text baseline (draw_text is baseline-relative);
/// callers using [`tooltip_rect`] pass
/// `jian_widgets::centered_text_baseline_y(rect, TOOLTIP_FONT_SIZE)`.
pub fn paint_tooltip_frame(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    rect: Rect,
    label: &str,
    shortcut: Option<&str>,
    baseline_y: f32,
) {
    cx.backend
        .fill_round_rect(rect, TOOLTIP_RADIUS, theme.popover);
    cx.backend
        .stroke_round_rect(rect, TOOLTIP_RADIUS, theme.border, 1.0);
    let text = TextLayout::single_run(
        label,
        "system-ui",
        TOOLTIP_FONT_SIZE,
        theme.popover_foreground.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend.draw_text(
        &text,
        Point2D::new(rect.origin.x + TOOLTIP_PAD_X, baseline_y),
    );
    // The shortcut hint is right-aligned and muted: it answers "how
    // else" after the label has answered "what", so it must not compete
    // with the label for the first read.
    if let Some(shortcut) = shortcut {
        let width = text_metrics::measure_chrome(cx.backend, shortcut, TOOLTIP_FONT_SIZE);
        let hint = TextLayout::single_run(
            shortcut,
            "system-ui",
            TOOLTIP_FONT_SIZE,
            theme.muted_foreground.to_jian(),
            Point2D::new(0.0, 0.0),
        );
        cx.backend.draw_text(
            &hint,
            Point2D::new(
                rect.origin.x + rect.size.x - TOOLTIP_PAD_X - width,
                baseline_y,
            ),
        );
    }
}

/// Measure, place, and paint a tooltip against `anchor`. Returns the
/// painted rect so callers can assert placement in tests.
pub fn paint_tooltip(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    anchor: Rect,
    label: &str,
    shortcut: Option<&str>,
    placement: TooltipPlacement,
    clamp_x: Option<(f32, f32)>,
) -> Rect {
    let width = tooltip_width(cx, label, shortcut);
    let rect = tooltip_rect(width, anchor, placement, clamp_x);
    let baseline_y = jian_widgets::centered_text_baseline_y(rect, TOOLTIP_FONT_SIZE);
    paint_tooltip_frame(cx, theme, rect, label, shortcut, baseline_y);
    rect
}

/// The horizontal band a viewport-wide tooltip may occupy.
pub fn viewport_clamp(viewport_width: f32) -> (f32, f32) {
    (
        TOOLTIP_VIEWPORT_INSET,
        (viewport_width - TOOLTIP_VIEWPORT_INSET).max(TOOLTIP_VIEWPORT_INSET),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn below_placement_centres_on_the_anchor() {
        let anchor = Rect::xywh(100.0, 8.0, 28.0, 28.0);
        let rect = tooltip_rect(60.0, anchor, TooltipPlacement::Below, None);
        assert_eq!(rect.origin.x, 100.0 + (28.0 - 60.0) / 2.0);
        assert_eq!(rect.origin.y, 8.0 + 28.0 + TOOLTIP_ANCHOR_GAP);
        assert_eq!(rect.size.y, TOOLTIP_HEIGHT);
    }

    #[test]
    fn above_placement_sits_over_the_anchor() {
        let anchor = Rect::xywh(100.0, 200.0, 28.0, 28.0);
        let rect = tooltip_rect(60.0, anchor, TooltipPlacement::Above, None);
        assert_eq!(rect.origin.y, 200.0 - TOOLTIP_ANCHOR_GAP - TOOLTIP_HEIGHT);
    }

    #[test]
    fn clamp_keeps_a_left_edge_tooltip_inside_the_viewport() {
        let anchor = Rect::xywh(2.0, 8.0, 28.0, 28.0);
        let rect = tooltip_rect(
            80.0,
            anchor,
            TooltipPlacement::Below,
            Some(viewport_clamp(1200.0)),
        );
        assert_eq!(rect.origin.x, TOOLTIP_VIEWPORT_INSET);
    }

    #[test]
    fn clamp_keeps_a_right_edge_tooltip_inside_the_viewport() {
        let viewport = 1200.0_f32;
        let anchor = Rect::xywh(viewport - 30.0, 8.0, 28.0, 28.0);
        let rect = tooltip_rect(
            120.0,
            anchor,
            TooltipPlacement::Below,
            Some(viewport_clamp(viewport)),
        );
        assert!(
            rect.origin.x + rect.size.x <= viewport - TOOLTIP_VIEWPORT_INSET + f32::EPSILON,
            "tooltip right edge {} ran past the viewport",
            rect.origin.x + rect.size.x
        );
    }

    #[test]
    fn a_tooltip_wider_than_the_band_pins_to_its_left_edge() {
        let anchor = Rect::xywh(100.0, 8.0, 28.0, 28.0);
        let rect = tooltip_rect(400.0, anchor, TooltipPlacement::Below, Some((4.0, 200.0)));
        assert_eq!(rect.origin.x, 4.0);
    }
}
