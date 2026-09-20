//! Lightweight placeholder glyph used while an image is unavailable.

use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect};

/// Minimal "picture" glyph — frame + sun + mountain strokes scaled
/// into the centre of `rect` (24px reference art, like the lucide
/// `image` icon but hand-stroked to avoid an icon-catalog dependency
/// in the paint path).
pub(super) fn paint_picture_glyph(cx: &mut PaintCx<'_>, rect: Rect, color: Color) {
    let size = (rect.size.x.min(rect.size.y) * 0.4).clamp(12.0, 48.0);
    let cx0 = rect.origin.x + rect.size.x / 2.0 - size / 2.0;
    let cy0 = rect.origin.y + rect.size.y / 2.0 - size / 2.0;
    let width = 1.5;
    cx.backend.stroke_round_rect(
        Rect {
            origin: Point2D::new(cx0, cy0),
            size: Point2D::new(size, size),
        },
        size * 0.12,
        color,
        width,
    );
    let sun = size * 0.16;
    cx.backend.stroke_round_rect(
        Rect {
            origin: Point2D::new(cx0 + size * 0.2, cy0 + size * 0.2),
            size: Point2D::new(sun, sun),
        },
        sun / 2.0,
        color,
        width,
    );
    cx.backend.stroke_line(
        Point2D::new(cx0 + size * 0.12, cy0 + size * 0.85),
        Point2D::new(cx0 + size * 0.45, cy0 + size * 0.45),
        color,
        width,
    );
    cx.backend.stroke_line(
        Point2D::new(cx0 + size * 0.45, cy0 + size * 0.45),
        Point2D::new(cx0 + size * 0.7, cy0 + size * 0.7),
        color,
        width,
    );
    cx.backend.stroke_line(
        Point2D::new(cx0 + size * 0.7, cy0 + size * 0.7),
        Point2D::new(cx0 + size * 0.88, cy0 + size * 0.52),
        color,
        width,
    );
}
