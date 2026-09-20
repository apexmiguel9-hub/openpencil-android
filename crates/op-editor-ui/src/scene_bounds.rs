//! Shared scene-bounds primitives for the SVG exporter (this crate) and the
//! raster/PDF export path (op-host-services).
//!
//! `BoundsAcc`, `normalize_rect`, and the per-`NodeKind` `own_paint_corners`
//! rules used to be maintained as near-identical copies in
//! `svg_export.rs` and `op-host-services/src/export.rs`. They are
//! single-sourced here; the one behavioral divergence between the two
//! callers (whether gradients / image fills count as "own paint") is
//! expressed through [`PaintCornerRules`] instead of a fork. The
//! `collect_bounds` traversals stay per-caller on purpose — the SVG
//! exporter intersects a clipping container's child union with the clip
//! rect (tight viewBox), while the raster exporter contributes the full
//! container rect and skips the subtree (mirrors the painter's surface
//! sizing).

use crate::layout_scene::{NodeKind, SceneNode};
use crate::{Point2D, Rect};

/// Min/max accumulator for world-space bounds.
pub struct BoundsAcc {
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
}

impl Default for BoundsAcc {
    fn default() -> Self {
        Self::new()
    }
}

impl BoundsAcc {
    pub fn new() -> Self {
        Self {
            min_x: f32::INFINITY,
            min_y: f32::INFINITY,
            max_x: f32::NEG_INFINITY,
            max_y: f32::NEG_INFINITY,
        }
    }

    pub fn add(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        self.min_x = self.min_x.min(x0);
        self.min_y = self.min_y.min(y0);
        self.max_x = self.max_x.max(x1);
        self.max_y = self.max_y.max(y1);
    }

    pub fn into_rect(self) -> Option<Rect> {
        if !self.min_x.is_finite() {
            return None;
        }
        Some(Rect {
            origin: Point2D::new(self.min_x, self.min_y),
            size: Point2D::new(self.max_x - self.min_x, self.max_y - self.min_y),
        })
    }
}

/// Which paint sources count towards a node's own silhouette. The SVG
/// exporter treats gradient and image fills as paint; the raster export
/// path historically considers solid fill / stroke only.
#[derive(Debug, Clone, Copy)]
pub struct PaintCornerRules {
    pub gradient_paints: bool,
    pub image_paints: bool,
}

/// Defensive normalisation — the layout pass yields positive-extent
/// rects, but a negative size would otherwise paint nothing.
pub fn normalize_rect(r: Rect) -> Rect {
    let x0 = r.origin.x.min(r.origin.x + r.size.x);
    let y0 = r.origin.y.min(r.origin.y + r.size.y);
    Rect {
        origin: Point2D::new(x0, y0),
        size: Point2D::new(r.size.x.abs(), r.size.y.abs()),
    }
}

fn has_own_paint(n: &SceneNode, rules: PaintCornerRules) -> bool {
    n.fill.is_some() || n.stroke.is_some() || (rules.gradient_paints && n.gradient.is_some())
}

/// Local-space corner points that bound `n`'s own paint (NOT its
/// children — those visit through each caller's `collect_bounds`). The
/// caller applies the cumulative parent+self transform; each returned
/// point gets pushed into the `BoundsAcc` as a world-space coord.
///
/// Returns `None` for invisible kinds: Group never paints own content;
/// Frame/Other contribute only when a counted paint source is set (see
/// [`PaintCornerRules`]); Path with empty `points` is invisible.
pub fn own_paint_corners(n: &SceneNode, rules: PaintCornerRules) -> Option<Vec<glam::Vec2>> {
    let stroke_pad = n.stroke.map(|s| s.width * 0.5).unwrap_or(0.0);
    let (x0, y0, x1, y1) = match &n.kind {
        NodeKind::Rect | NodeKind::Ellipse | NodeKind::Polygon | NodeKind::Line => {
            let nr = normalize_rect(n.bounds);
            (
                nr.origin.x,
                nr.origin.y,
                nr.origin.x + nr.size.x,
                nr.origin.y + nr.size.y,
            )
        }
        NodeKind::Frame => {
            if !has_own_paint(n, rules) {
                return None;
            }
            let nr = normalize_rect(n.bounds);
            (
                nr.origin.x,
                nr.origin.y,
                nr.origin.x + nr.size.x,
                nr.origin.y + nr.size.y,
            )
        }
        NodeKind::Other(tag) if tag == "icon_font" => {
            if n.text.as_ref().is_none_or(|s| s.trim().is_empty()) {
                return None;
            }
            let nr = normalize_rect(n.bounds);
            (
                nr.origin.x,
                nr.origin.y,
                nr.origin.x + nr.size.x,
                nr.origin.y + nr.size.y,
            )
        }
        NodeKind::Other(_) => {
            // Unknown tagged kinds paint no own silhouette; their bounds
            // still contribute when authored with a counted paint source.
            if !(has_own_paint(n, rules) || (rules.image_paints && n.image_src.is_some())) {
                return None;
            }
            let nr = normalize_rect(n.bounds);
            (
                nr.origin.x,
                nr.origin.y,
                nr.origin.x + nr.size.x,
                nr.origin.y + nr.size.y,
            )
        }
        NodeKind::Text => {
            // Text bounds are the layout-resolved "where the glyphs sit"
            // rect. Real glyph extents can overshoot for tails / accents,
            // but `bounds` is the right approximation without a per-glyph
            // metric pass.
            let has_text = n.text.as_ref().is_some_and(|s| !s.is_empty());
            if !has_text {
                return None;
            }
            let nr = normalize_rect(n.bounds);
            (
                nr.origin.x,
                nr.origin.y,
                nr.origin.x + nr.size.x.max(1.0),
                nr.origin.y + nr.size.y.max(1.0),
            )
        }
        NodeKind::Path => {
            if n.svg_path.is_some() && has_own_paint(n, rules) {
                let nr = normalize_rect(n.bounds);
                return Some(vec![
                    glam::Vec2::new(nr.origin.x - stroke_pad, nr.origin.y - stroke_pad),
                    glam::Vec2::new(
                        nr.origin.x + nr.size.x + stroke_pad,
                        nr.origin.y - stroke_pad,
                    ),
                    glam::Vec2::new(
                        nr.origin.x + nr.size.x + stroke_pad,
                        nr.origin.y + nr.size.y + stroke_pad,
                    ),
                    glam::Vec2::new(
                        nr.origin.x - stroke_pad,
                        nr.origin.y + nr.size.y + stroke_pad,
                    ),
                ]);
            }
            if n.points.is_empty() {
                return None;
            }
            // Each polyline anchor + stroke-pad cardinal offsets so the
            // cumulative parent transform doesn't clip them.
            let mut out = Vec::with_capacity(n.points.len() * 4);
            for p in &n.points {
                out.push(glam::Vec2::new(p.x - stroke_pad, p.y - stroke_pad));
                out.push(glam::Vec2::new(p.x + stroke_pad, p.y - stroke_pad));
                out.push(glam::Vec2::new(p.x - stroke_pad, p.y + stroke_pad));
                out.push(glam::Vec2::new(p.x + stroke_pad, p.y + stroke_pad));
            }
            return Some(out);
        }
        NodeKind::Group => return None,
    };
    if (x1 - x0).abs() == 0.0 && (y1 - y0).abs() == 0.0 {
        return None;
    }
    Some(vec![
        glam::Vec2::new(x0 - stroke_pad, y0 - stroke_pad),
        glam::Vec2::new(x1 + stroke_pad, y0 - stroke_pad),
        glam::Vec2::new(x1 + stroke_pad, y1 + stroke_pad),
        glam::Vec2::new(x0 - stroke_pad, y1 + stroke_pad),
    ])
}
