//! The context-chip row directly above the chat input.
//!
//! Two chips can be live at once — the pinned-style receipt ("what the next
//! turn will look like") and the selection chip ("what it will be pointed
//! at"). They used to own a stacked row each, so a user with both had 52 px
//! of chrome shoved between the transcript and the place they type, in two
//! full-width slabs. They are one line now: same information, one row, and
//! the row disappears entirely when neither chip has anything to say.
//!
//! Everything about where a chip sits is resolved by [`chip_row_layout`],
//! which paint and hit-test both call. The rects were previously computed
//! twice — once per painter, once per hit-test branch — and the two copies
//! only agreed because they were reading the same constants.

use crate::theme::Theme;
use crate::widgets::ai_chat_panel::{footer_label_width, AIChatPlaceholder};
use crate::widgets::text_metrics;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect, TextLayout};

/// Chip height. Small enough that the row reads as an annotation on the
/// input rather than another band of chrome.
pub(crate) const CHIP_H: f32 = 22.0;
/// Breathing room above and below the chips inside the row.
pub(crate) const CHIP_ROW_PAD_Y: f32 = 3.0;
/// Height the input block reserves when at least one chip is live.
pub(crate) const CHIP_ROW_HEIGHT: f32 = CHIP_H + CHIP_ROW_PAD_Y * 2.0;
/// Space between the two chips.
pub(crate) const CHIP_GAP: f32 = 6.0;
/// Horizontal inset from a chip's edge to its label.
pub(crate) const CHIP_PAD_X: f32 = 8.0;
/// One step below the 11 px the chips used to paint at.
pub(crate) const CHIP_FONT: f32 = 10.5;
/// Width of the trailing ✕ column. Also the ✕ hit target's width, which is
/// why it is 16 rather than the ~10 px the glyph needs: a clear button that
/// is hard to hit is worse than one that is slightly wider than it looks.
pub(crate) const CHIP_CLEAR_W: f32 = 16.0;
/// Corner radius — full pill, tied to the height so the two cannot drift.
pub(crate) const CHIP_RADIUS: f32 = CHIP_H / 2.0;
/// Narrowest a chip may be squeezed to before the row drops one instead.
const CHIP_MIN_W: f32 = 44.0;

/// Where each live chip sits on the row. `None` means that chip is either
/// absent or was dropped because the row could not hold both.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct ChipRowLayout {
    /// Pinned-style receipt — leftmost.
    pub(crate) style: Option<Rect>,
    /// Selection chip — right of the style receipt.
    pub(crate) selection: Option<Rect>,
}

/// The ✕ target inside `chip`. Anchored to the chip's right edge and given
/// the chip's full height, so the hit area is 16×22 — comfortably past the
/// 16×16 floor even though the glyph is smaller than that.
pub(crate) fn chip_clear_rect(chip: Rect) -> Rect {
    Rect {
        origin: Point2D::new(chip.origin.x + chip.size.x - CHIP_CLEAR_W, chip.origin.y),
        size: Point2D::new(CHIP_CLEAR_W, chip.size.y),
    }
}

/// Place the live chips left-to-right across the top of `input_rect`.
///
/// `style_w` / `selection_w` are the widths each chip would like. They are
/// honoured whenever both fit; past that the row shrinks them toward
/// [`CHIP_MIN_W`] (paint ellipsizes into whatever it gets), and only when
/// even two minimum chips will not fit does it drop the style receipt —
/// the selection chip carries an action, the receipt is a readout.
pub(crate) fn chip_row_layout(
    style_w: Option<f32>,
    selection_w: Option<f32>,
    input_rect: Rect,
) -> ChipRowLayout {
    let avail = input_rect.size.x.max(0.0);
    let top = input_rect.origin.y + CHIP_ROW_PAD_Y;
    let at = |x: f32, w: f32| Rect {
        origin: Point2D::new(x, top),
        size: Point2D::new(w, CHIP_H),
    };
    let left = input_rect.origin.x;

    match (style_w, selection_w) {
        (None, None) => ChipRowLayout::default(),
        (Some(w), None) => ChipRowLayout {
            style: Some(at(left, w.min(avail))),
            selection: None,
        },
        (None, Some(w)) => ChipRowLayout {
            style: None,
            selection: Some(at(left, w.min(avail))),
        },
        (Some(a), Some(b)) => {
            let (a, b) = fit_pair(a, b, avail);
            match a {
                Some(a) => ChipRowLayout {
                    style: Some(at(left, a)),
                    selection: b.map(|b| at(left + a + CHIP_GAP, b)),
                },
                None => ChipRowLayout {
                    style: None,
                    selection: b.map(|b| at(left, b)),
                },
            }
        }
    }
}

/// Widths for two chips sharing one row, or `(None, Some(_))` when the row
/// is too narrow to keep both.
fn fit_pair(natural_a: f32, natural_b: f32, avail: f32) -> (Option<f32>, Option<f32>) {
    if natural_a + CHIP_GAP + natural_b <= avail {
        return (Some(natural_a), Some(natural_b));
    }
    if avail < CHIP_MIN_W * 2.0 + CHIP_GAP {
        return (None, Some(natural_b.min(avail)));
    }
    // Both survive: split the row in proportion to what each asked for,
    // then hand back whatever the second chip did not need.
    let room = avail - CHIP_GAP;
    let share = room * natural_a / (natural_a + natural_b).max(f32::EPSILON);
    let a = share.clamp(CHIP_MIN_W, room - CHIP_MIN_W).min(natural_a);
    let b = (room - a).min(natural_b);
    let a = (room - b).min(natural_a).max(CHIP_MIN_W);
    (Some(a), Some(b))
}

/// Paint every live chip on the row.
pub(crate) fn paint_chip_row(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    widget: &AIChatPlaceholder<'_>,
    input_rect: Rect,
) {
    let layout = widget.chip_row(input_rect);
    if let (Some(chip), Some(receipt), Some(label)) = (
        layout.style,
        widget.style_receipt.as_ref(),
        widget.style_receipt_label(),
    ) {
        crate::widgets::ai_chat_style_receipt::paint_style_chip(cx, theme, receipt, &label, chip);
    }
    if let Some(chip) = layout.selection {
        paint_selection_chip(cx, theme, &widget.selection_chip_label(), chip);
    }
}

/// The selection chip: label plus a ✕ that clears the canvas selection.
fn paint_selection_chip(cx: &mut PaintCx<'_>, theme: &Theme, label: &str, chip: Rect) {
    fill_chip(cx, theme, chip);
    let label_max = (chip.size.x - CHIP_PAD_X - CHIP_CLEAR_W).max(0.0);
    let fitted = text_metrics::fit_chrome(cx.backend, label, label_max, CHIP_FONT);
    draw_chip_text(cx, theme, &fitted, chip.origin.x + CHIP_PAD_X, chip);
    draw_chip_clear(cx, theme, chip);
}

/// The chip's background. Shared so both chips cannot drift in radius or
/// fill the way two separately-written painters would.
pub(crate) fn fill_chip(cx: &mut PaintCx<'_>, theme: &Theme, chip: Rect) {
    cx.backend.fill_round_rect(chip, CHIP_RADIUS, theme.muted);
}

/// A chip's ✕, centred in the clear column.
pub(crate) fn draw_chip_clear(cx: &mut PaintCx<'_>, theme: &Theme, chip: Rect) {
    let clear = chip_clear_rect(chip);
    let x = text_metrics::centered_text_x(cx.backend, "×", CHIP_FONT, clear);
    draw_chip_text(cx, theme, "×", x, chip);
}

/// Draw one chip run at `x`, vertically centred in the chip.
pub(crate) fn draw_chip_text(cx: &mut PaintCx<'_>, theme: &Theme, text: &str, x: f32, chip: Rect) {
    let layout = TextLayout::single_run(
        text,
        text_metrics::CHROME_FONT_FAMILY,
        CHIP_FONT,
        (theme.muted_foreground).to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend.draw_text(
        &layout,
        Point2D::new(x, jian_widgets::centered_text_baseline_y(chip, CHIP_FONT)),
    );
}

/// Natural width of a chip holding `label` plus a trailing ✕.
pub(crate) fn label_chip_width(label: &str) -> f32 {
    CHIP_PAD_X + footer_label_width(label, CHIP_FONT) + 4.0 + CHIP_CLEAR_W
}

#[cfg(test)]
#[path = "ai_chat_chip_row_tests.rs"]
mod ai_chat_chip_row_tests;
