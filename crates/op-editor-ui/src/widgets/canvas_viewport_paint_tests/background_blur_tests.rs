//! Background (backdrop) blur save-layer tests for `canvas_viewport_paint.rs`.
//!
//! Split out of `canvas_viewport_paint_tests.rs` to keep every file
//! under the repository's 800-line cap.

use crate::layout_scene::{Effect, NodeKind, SceneNode};
use crate::widgets::canvas_viewport_paint::paint_node;
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect, RenderBackend, TextLayout};

#[derive(Default)]
struct BackdropCaptureBackend {
    ops: Vec<&'static str>,
}

impl RenderBackend for BackdropCaptureBackend {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, _: Rect, _: Color) {
        self.ops.push("fill");
    }
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn draw_text(&mut self, _: &TextLayout, _: Point2D) {}
    fn clip_rect(&mut self, _: Rect) {
        self.ops.push("clip");
    }
    fn clip_round_rect(&mut self, _: Rect, _: f32) {
        self.ops.push("clip_round");
    }
    fn save(&mut self) {
        self.ops.push("save");
    }
    fn restore(&mut self) {
        self.ops.push("restore");
    }
    fn push_backdrop_blur_layer(&mut self, _: f32) {
        self.ops.push("backdrop");
    }
    fn translate(&mut self, _: Point2D) {}
    fn stroke_line(&mut self, _: Point2D, _: Point2D, _: Color, _: f32) {}
    fn fill_round_rect(&mut self, _: Rect, _: f32, _: Color) {
        self.ops.push("fill");
    }
    fn stroke_round_rect(&mut self, _: Rect, _: f32, _: Color, _: f32) {}
    fn stroke_svg_path(&mut self, _: &str, _: Point2D, _: f32, _: Color, _: f32) {}
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
}

#[test]
fn background_blur_clips_and_filters_before_node_fill() {
    let mut node = SceneNode::leaf("glass", NodeKind::Rect);
    node.bounds = Rect::xywh(0.0, 0.0, 100.0, 60.0);
    node.corner_radius = 8.0;
    node.fill = Some(Color::BLACK);
    node.effects = vec![Effect::BackgroundBlur { radius: 12.0 }];
    let mut backend = BackdropCaptureBackend::default();
    paint_node(
        &mut PaintCx {
            backend: &mut backend,
        },
        &node,
        Point2D::ZERO,
        1.0,
        Rect::xywh(-100.0, -100.0, 1000.0, 1000.0),
    );
    assert_eq!(
        backend.ops,
        vec![
            "save",
            "clip_round",
            "backdrop",
            "fill",
            "restore",
            "restore"
        ]
    );
}
