use crate::layout_scene::SceneNode;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect, TextLayout};
use op_editor_core::Viewport;

const LABEL_SIDE_PADDING: f32 = 4.0;
const LABEL_FONT_SIZE: f32 = 12.0;
const GENERATING_ICON_SIZE: f32 = 14.0;
const GENERATING_ICON_GAP: f32 = 4.0;

/// Gap between the label text's visual bottom (its descender) and the top
/// edge of the frame the label names.
///
/// This band is SCREEN-space and deliberately does not scale with zoom —
/// chrome has to stay legible however far out the canvas is. But it was
/// built from a baseline 18 px above the frame, which left the text's
/// visual bottom 15 px clear of it, and at that distance the label reads as
/// floating in the canvas rather than belonging to its frame (user report
/// 2026-08-02). 6 px reads as attached at every zoom while still keeping
/// the glyphs off the frame's own border.
const LABEL_TEXT_GAP: f32 = 6.0;

/// Cap height and descender depth for [`LABEL_FONT_SIZE`], used to place the
/// baseline from the text's visual edges and to size the box around it.
/// A 12 px font sits roughly 9 px above the baseline and 3 px below.
const LABEL_CAP_HEIGHT: f32 = 9.0;
const LABEL_DESCENT: f32 = 3.0;

/// Padding between the text's visual bounds and the label's hit box. 3 px
/// also lets the box swallow the taller generating icon (14 px, centred on
/// the text line), so the "Generating:" chip stays clickable across its
/// whole height.
const LABEL_BOX_PADDING: f32 = 3.0;

/// `draw_text` baseline, as a distance ABOVE the frame's top edge.
const LABEL_BASELINE_OFFSET: f32 = LABEL_TEXT_GAP + LABEL_DESCENT;

/// Top of the label's hit box, as a distance above the frame's top edge.
const LABEL_BOX_TOP_OFFSET: f32 = LABEL_BASELINE_OFFSET + LABEL_CAP_HEIGHT + LABEL_BOX_PADDING;

/// Height of the label's hit box: the text's visual extent plus padding on
/// both sides.
const LABEL_BOX_HEIGHT: f32 = LABEL_CAP_HEIGHT + LABEL_DESCENT + LABEL_BOX_PADDING * 2.0;

/// `draw_text` takes a baseline while icons take a top-left corner. Keep the
/// existing frame-label baseline fixed and align the icon with the text line's
/// visual center using the same cap-height approximation as
/// `jian_widgets::centered_text_baseline_y`.
fn generating_icon_top(text_baseline_y: f32) -> f32 {
    let text_center_y = text_baseline_y - LABEL_FONT_SIZE * 0.35;
    text_center_y - GENERATING_ICON_SIZE / 2.0
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct FrameLabel {
    pub(super) id: String,
    pub(super) text: String,
    pub(super) color: Color,
    pub(super) generating: bool,
}

impl FrameLabel {
    pub(super) fn new(
        id: impl Into<String>,
        text: impl Into<String>,
        color: Color,
        generating: bool,
    ) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            color,
            generating,
        }
    }
}

#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static LABEL_MATCH_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_label_match_count() {
    LABEL_MATCH_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn label_match_count() -> usize {
    LABEL_MATCH_COUNT.with(Cell::get)
}

fn label_id_matches(node_id: &str, label_id: &str) -> bool {
    #[cfg(test)]
    LABEL_MATCH_COUNT.with(|count| count.set(count.get() + 1));

    node_id == label_id
}

fn label_width(label: &FrameLabel) -> f32 {
    let text_width = (label.text.chars().count() as f32 * 7.0).max(1.0);
    if label.generating {
        GENERATING_ICON_SIZE + GENERATING_ICON_GAP + text_width
    } else {
        text_width
    }
}

fn label_rect(
    node: &SceneNode,
    label: &FrameLabel,
    viewport_origin: Point2D,
    viewport: &Viewport,
) -> Rect {
    let b = node.aggregate_bounds();
    let sx = viewport_origin.x + b.origin.x * viewport.zoom;
    let sy = viewport_origin.y + b.origin.y * viewport.zoom;
    Rect::xywh(
        sx - LABEL_SIDE_PADDING,
        sy - LABEL_BOX_TOP_OFFSET,
        label_width(label) + LABEL_SIDE_PADDING * 2.0,
        LABEL_BOX_HEIGHT,
    )
}

pub(super) fn frame_label_at_point(
    roots: &[SceneNode],
    labels: &[FrameLabel],
    viewport_origin: Point2D,
    viewport: &Viewport,
    clip: Rect,
    point: Point2D,
) -> Option<String> {
    if !(clip).contains(point) {
        return None;
    }
    let mut labels = labels.iter().peekable();
    for node in roots {
        let Some(label) = labels.peek() else {
            break;
        };
        if !label_id_matches(&node.id, &label.id) {
            continue;
        }
        let label = labels.next().expect("peeked label exists");
        if (label_rect(node, label, viewport_origin, viewport)).contains(point) {
            return Some(label.id.clone());
        }
    }
    None
}

pub(super) fn paint_frame_labels(
    cx: &mut PaintCx<'_>,
    roots: &[SceneNode],
    labels: &[FrameLabel],
    hidden_ids: &[String],
    viewport_origin: Point2D,
    viewport: &Viewport,
    clip: Rect,
) {
    let mut labels = labels.iter().peekable();
    for node in roots {
        let Some(label) = labels.peek() else {
            break;
        };
        if !label_id_matches(&node.id, &label.id) {
            continue;
        }
        let label = labels.next().expect("peeked label exists");
        if hidden_ids.iter().any(|hidden| hidden == &node.id) {
            continue;
        }
        let b = node.aggregate_bounds();
        let sx = viewport_origin.x + b.origin.x * viewport.zoom;
        let sy = viewport_origin.y + b.origin.y * viewport.zoom;
        if sy < clip.origin.y || sy > clip.origin.y + clip.size.y + 18.0 {
            continue;
        }
        if sx > clip.origin.x + clip.size.x || sx + 600.0 < clip.origin.x {
            continue;
        }
        let hit_rect = label_rect(node, label, viewport_origin, viewport);
        let layout = TextLayout::single_run(
            &label.text,
            "system-ui",
            LABEL_FONT_SIZE,
            jian_core::scene::Color::rgba(
                (label.color.r * 255.0) as u8,
                (label.color.g * 255.0) as u8,
                (label.color.b * 255.0) as u8,
                255,
            ),
            Point2D::ZERO,
        )
        .with_font_weight(500);
        let text_baseline_y = sy - LABEL_BASELINE_OFFSET;
        let mut text_x = hit_rect.origin.x + LABEL_SIDE_PADDING;
        if label.generating {
            draw_icon(
                cx.backend,
                Icon::Sparkles,
                Point2D::new(text_x, generating_icon_top(text_baseline_y)),
                GENERATING_ICON_SIZE,
                label.color,
                1.5,
            );
            text_x += GENERATING_ICON_SIZE + GENERATING_ICON_GAP;
        }
        cx.backend
            .draw_text(&layout, Point2D::new(text_x, text_baseline_y));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generating_icon_center_matches_text_line_center() {
        let baseline_y = 40.0;
        let icon_center_y = generating_icon_top(baseline_y) + GENERATING_ICON_SIZE / 2.0;
        let text_center_y = baseline_y - LABEL_FONT_SIZE * 0.35;
        assert!((icon_center_y - text_center_y).abs() < f32::EPSILON);
    }

    fn frame_at(doc_y: f32) -> SceneNode {
        use crate::layout_scene::NodeKind;
        let mut frame = SceneNode::leaf("n1", NodeKind::Frame);
        frame.bounds = Rect::xywh(40.0, doc_y, 120.0, 80.0);
        frame
    }

    fn label() -> FrameLabel {
        FrameLabel::new(
            "n1",
            "Frame",
            Color {
                r: 0.6,
                g: 0.6,
                b: 0.6,
                a: 1.0,
            },
            false,
        )
    }

    /// The label's hit box must stop ABOVE the frame's own top edge. If it
    /// reached `sy`, a click just inside the frame near its top would select
    /// the root via its label instead of the node actually under the cursor
    /// — `frame_label_at_point` runs before the normal canvas hit-test in
    /// both hosts' `press_canvas_tiers`.
    #[test]
    fn label_hit_box_never_reaches_the_frame_top_edge() {
        for zoom in [0.1_f32, 0.25, 0.5, 1.0, 2.0, 8.0] {
            for doc_y in [-400.0_f32, 0.0, 37.5, 900.0] {
                let mut viewport = Viewport::IDENTITY;
                viewport.zoom = zoom;
                let node = frame_at(doc_y);
                let label = label();
                let rect = label_rect(&node, &label, Point2D::ZERO, &viewport);

                let frame_top = doc_y * zoom;
                let box_bottom = rect.origin.y + rect.size.y;
                assert!(
                    box_bottom < frame_top,
                    "label box bottom {box_bottom} must stay above frame top {frame_top} \
                     (zoom {zoom}, doc_y {doc_y})"
                );
            }
        }
    }

    /// The behavioural half of the invariant above: a point inside the
    /// frame, right against its top edge, is not a label hit.
    #[test]
    fn a_point_just_inside_the_frame_does_not_hit_its_label() {
        let viewport = Viewport::IDENTITY;
        let node = frame_at(100.0);
        let labels = vec![label()];
        let clip = Rect::xywh(0.0, 0.0, 400.0, 400.0);

        // Directly on the frame's top edge, and one pixel inside it.
        for probe_y in [100.0_f32, 101.0] {
            assert_eq!(
                frame_label_at_point(
                    std::slice::from_ref(&node),
                    &labels,
                    Point2D::ZERO,
                    &viewport,
                    clip,
                    Point2D::new(60.0, probe_y),
                ),
                None,
                "y {probe_y} is inside the frame, not on its label"
            );
        }

        // The band the label does own still hits.
        assert_eq!(
            frame_label_at_point(
                std::slice::from_ref(&node),
                &labels,
                Point2D::ZERO,
                &viewport,
                clip,
                Point2D::new(60.0, 100.0 - LABEL_BASELINE_OFFSET),
            ),
            Some("n1".into())
        );
    }

    /// The glyphs must not escape the box that hit-tests them: cap height
    /// above the baseline, descender below, both inside the padded box.
    #[test]
    fn label_text_stays_inside_its_hit_box() {
        let viewport = Viewport::IDENTITY;
        let node = frame_at(100.0);
        let label = label();
        let rect = label_rect(&node, &label, Point2D::ZERO, &viewport);

        let baseline = 100.0 - LABEL_BASELINE_OFFSET;
        let text_top = baseline - LABEL_CAP_HEIGHT;
        let text_bottom = baseline + LABEL_DESCENT;
        assert!(text_top >= rect.origin.y, "cap height escapes the box top");
        assert!(
            text_bottom <= rect.origin.y + rect.size.y,
            "descenders escape the box bottom"
        );

        // The generating icon is taller than the text line; the box has to
        // cover it too or the chip is only partly clickable.
        let icon_top = generating_icon_top(baseline);
        assert!(icon_top >= rect.origin.y, "generating icon escapes the box");
        assert!(icon_top + GENERATING_ICON_SIZE <= rect.origin.y + rect.size.y);
    }

    /// The gap the user actually sees between the name and its frame.
    #[test]
    fn label_text_sits_close_to_the_frame_it_names() {
        let text_bottom_to_frame = LABEL_BASELINE_OFFSET - LABEL_DESCENT;
        assert_eq!(text_bottom_to_frame, LABEL_TEXT_GAP);
        assert!(
            text_bottom_to_frame <= 8.0,
            "a label further than ~8px from its frame reads as floating"
        );
    }
}
