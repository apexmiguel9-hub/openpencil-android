use crate::widgets::settings_form::ellipsize;
use crate::widgets::text_metrics;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};

pub(super) const HEADER_ACTION_RIGHT_INSET: f32 = 12.0;
const HEADER_ACTION_W: f32 = 208.0;
const HEADER_ACTION_PAD_X: f32 = 12.0;
const HEADER_ACTION_H: f32 = 24.0;
const HEADER_COPY_GAP: f32 = 12.0;

pub(super) struct FittedHeaderCopy {
    pub title: String,
    pub action: String,
    pub action_w: f32,
}

/// Keep the section title and trailing action on one fixed-height row without
/// overlap. Long-script locales may need to shorten both sides.
pub(super) fn fit_header_copy(
    cx: &mut PaintCx<'_>,
    title: &str,
    action: &str,
    content_w: f32,
) -> FittedHeaderCopy {
    let title_max_w =
        (content_w - HEADER_ACTION_RIGHT_INSET - HEADER_ACTION_W - HEADER_COPY_GAP).max(0.0);
    let title = ellipsize(cx, title, title_max_w, 15.0);
    let action_max_w = HEADER_ACTION_W - HEADER_ACTION_PAD_X * 2.0;
    let action = ellipsize(cx, action, action_max_w, 12.0);
    let action_w = text_metrics::measure_chrome(cx.backend, &action, 12.0);
    FittedHeaderCopy {
        title,
        action,
        action_w,
    }
}

pub(super) fn header_action_rect(content: Rect, y: f32) -> Rect {
    Rect {
        origin: Point2D::new(
            content.origin.x + content.size.x - HEADER_ACTION_RIGHT_INSET - HEADER_ACTION_W,
            y,
        ),
        size: Point2D::new(HEADER_ACTION_W, HEADER_ACTION_H),
    }
}

pub(super) fn header_action_text_x(rect: Rect, text_w: f32) -> f32 {
    rect.origin.x + (rect.size.x - text_w).max(0.0) / 2.0
}

pub(super) fn header_action_text_baseline_y(rect: Rect) -> f32 {
    rect.origin.y + rect.size.y / 2.0 + 4.0
}
