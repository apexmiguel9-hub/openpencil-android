//! Silhouette geometry helpers for canvas cursors: the uniform outset used
//! for both the contact shadow and the white rim. Split out of
//! `canvas_agent_cursor.rs` at the 800-line cap; pure code motion.

use crate::widgets::PaintCx;
use crate::{Color, Point2D};

/// Uniformly outset a silhouette by `offset` px about its centroid. Used for
/// both the shadow layers and the white rim: a filled outset paints the rim as
/// GEOMETRY, so its width is exact everywhere and no stroke joins can notch it
/// (the trait's fallback polygon stroke drew each edge as its own capped
/// segment — every vertex of the densely-sampled arc showed a jaggy).
pub(super) fn outset(body: &[Point2D], offset: f32, shift_x: f32, shift_y: f32) -> Vec<Point2D> {
    let n = body.len() as f32;
    let (mut sum_x, mut sum_y) = (0.0f32, 0.0f32);
    for p in body {
        sum_x += p.x;
        sum_y += p.y;
    }
    let (cx, cy) = (sum_x / n, sum_y / n);
    let radius = body
        .iter()
        .map(|p| ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt())
        .fold(1.0f32, f32::max);
    let k = 1.0 + offset / radius;
    body.iter()
        .map(|p| Point2D::new(cx + (p.x - cx) * k + shift_x, cy + (p.y - cy) * k + shift_y))
        .collect()
}

/// Width of the white rim (px of outset beyond the body silhouette).
pub(super) const RIM: f32 = 1.6;

/// Pencil-style contact shadow: narrow neutral-black feather, shifted a
/// half-pixel left/down. Filled expansions keep the fallback painter soft
/// without introducing the jagged polygon joins produced by strokes.
pub(super) fn paint_soft_shadow(cx: &mut PaintCx<'_>, body: &[Point2D], alpha_scale: f32) {
    // The shadow sits outside the white rim; largest/faintest paints first.
    for (offset, alpha) in [
        (RIM + 1.6, 0.030),
        (RIM + 1.2, 0.040),
        (RIM + 0.8, 0.050),
        (RIM + 0.4, 0.060),
    ] {
        let ring = outset(body, offset, -0.5, 0.5);
        cx.backend
            .fill_polygon(&ring, Color::BLACK.with_alpha(alpha * alpha_scale));
    }
}

/// The white rim, painted as a filled outset the body then covers — a solid
/// ring of exactly `RIM` px with no stroke joins to notch it.
pub(super) fn paint_rim(cx: &mut PaintCx<'_>, body: &[Point2D], alpha: f32) {
    cx.backend
        .fill_polygon(&outset(body, RIM, 0.0, 0.0), Color::WHITE.with_alpha(alpha));
}
