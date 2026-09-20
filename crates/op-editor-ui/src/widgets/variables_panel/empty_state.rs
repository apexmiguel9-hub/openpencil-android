//! The variables panel's "no variables" line.
//!
//! Carved out of `paint.rs`, which sits at the repo's 800-line ceiling.

use crate::theme::Theme;
use crate::widgets::text_metrics;
use crate::widgets::PaintCx;
use crate::Rect;

/// Centred "no variables" line.
///
/// Fitted to the panel and centred on its real painted width. The previous
/// `centre - 52` hard-coded half of the English label, so every longer
/// localized string ("Chưa có biến nào được định nghĩa") started
/// left-of-centre and ran off the panel's right edge.
pub(super) fn paint_empty_state(cx: &mut PaintCx<'_>, theme: &Theme, rect: Rect, label: &str) {
    const FONT: f32 = 14.0;
    /// Clear space either side of the centred line.
    const INSET: f32 = 16.0;
    let label = text_metrics::fit_chrome(
        cx.backend,
        label,
        (rect.size.x - INSET * 2.0).max(0.0),
        FONT,
    );
    let x = text_metrics::centered_text_x(cx.backend, &label, FONT, rect);
    super::paint::paint_text(
        cx,
        &label,
        FONT,
        theme.muted_foreground,
        x,
        rect.origin.y + rect.size.y / 2.0,
    );
}
