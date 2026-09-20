//! Guest-side owner-identity confirmation body.
//!
//! Every authoritative row is painted with the same weight and colour above a
//! visible separator; the peer-chosen name sits below it, muted and quoted,
//! under its own label. The layout is the defence: a chosen display name is
//! never given the position, colour, or weight of the account line, so it
//! cannot be mistaken for the identity the guest is being asked to accept.

use super::*;
use crate::widgets::collab_ui::CollabOwnerConfirmModel;
use crate::widgets::text_metrics;

pub(super) const CONFIRM_OWNER_HEAD_HEIGHT: f32 = 72.0;
pub(super) const CONFIRM_OWNER_ROW_HEIGHT: f32 = 32.0;

impl CollabPanel<'_> {
    pub(super) fn paint_owner_confirmation(
        &self,
        cx: &mut PaintCx<'_>,
        rect: Rect,
        body_top: f32,
        confirm: &CollabOwnerConfirmModel,
    ) {
        let left = rect.origin.x + PAD;
        let width = rect.size.x - PAD * 2.0;
        draw_icon(
            cx.backend,
            Icon::Users,
            Point2D::new(left, body_top + 8.0),
            16.0,
            self.theme.primary,
            1.5,
        );
        paint_text(
            cx,
            &confirm.title,
            13.0,
            self.theme.foreground,
            Point2D::new(left + 24.0, body_top + 21.0),
            600,
        );
        paint_text(
            cx,
            &confirm.hint,
            10.0,
            self.theme.muted_foreground,
            Point2D::new(left, body_top + 41.0),
            400,
        );

        let mut row_y = body_top + CONFIRM_OWNER_HEAD_HEIGHT - CONFIRM_OWNER_ROW_HEIGHT;
        for row in &confirm.authoritative {
            paint_text(
                cx,
                &row.label,
                9.0,
                self.theme.muted_foreground,
                Point2D::new(left, row_y + 11.0),
                500,
            );
            let shown = crate::util::ellipsize_to_width(&row.value, width, |text| {
                text_metrics::measure_chrome(cx.backend, text, 11.0)
            });
            paint_text(
                cx,
                &shown,
                11.0,
                self.theme.foreground,
                Point2D::new(left, row_y + 26.0),
                500,
            );
            row_y += CONFIRM_OWNER_ROW_HEIGHT;
        }

        let Some(claimed) = confirm.claimed_name.as_ref() else {
            return;
        };
        // Separator: everything below it is the peer's own claim about itself.
        cx.backend
            .fill_rect(Rect::xywh(left, row_y + 2.0, width, 1.0), self.theme.border);
        paint_text(
            cx,
            &claimed.label,
            9.0,
            self.theme.muted_foreground,
            Point2D::new(left, row_y + 15.0),
            500,
        );
        let quoted = format!("“{}”", claimed.value);
        let shown = crate::util::ellipsize_to_width(&quoted, width, |text| {
            text_metrics::measure_chrome(cx.backend, text, 11.0)
        });
        paint_text(
            cx,
            &shown,
            11.0,
            self.theme.muted_foreground,
            Point2D::new(left, row_y + 29.0),
            400,
        );
    }
}
