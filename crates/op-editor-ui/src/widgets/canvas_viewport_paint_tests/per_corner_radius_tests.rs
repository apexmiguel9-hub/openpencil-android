//! Per-corner corner-radius fill / stroke / gradient tests for `canvas_viewport_paint.rs`.
//!
//! Split out of `canvas_viewport_paint_tests.rs` to keep every file
//! under the repository's 800-line cap.

use crate::layout_scene::{
    NodeKind, SceneGradient, SceneGradientStop, SceneNode, SceneStroke, SceneStrokeAlign,
};
use crate::widgets::canvas_viewport_overlay::paint_fill_then_stroke;
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect, RenderBackend, TextLayout};

#[derive(Default)]
struct RadiusCaptureBackend {
    uniform_fills: usize,
    per_corner_fills: Vec<[f32; 4]>,
    uniform_gradient_radii: Vec<f32>,
    per_corner_gradient_radii: Vec<[f32; 4]>,
    uniform_strokes: usize,
    per_corner_strokes: Vec<[f32; 4]>,
}

impl RenderBackend for RadiusCaptureBackend {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, _: Rect, _: Color) {}
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn draw_text(&mut self, _: &TextLayout, _: Point2D) {}
    fn clip_rect(&mut self, _: Rect) {}
    fn save(&mut self) {}
    fn restore(&mut self) {}
    fn translate(&mut self, _: Point2D) {}
    fn stroke_line(&mut self, _: Point2D, _: Point2D, _: Color, _: f32) {}
    fn fill_round_rect(&mut self, _: Rect, _: f32, _: Color) {
        self.uniform_fills += 1;
    }
    fn fill_round_rect_per_corner(&mut self, _: Rect, radii: [f32; 4], _: Color) {
        self.per_corner_fills.push(radii);
    }
    fn fill_round_rect_linear_gradient(
        &mut self,
        _: Rect,
        radius: f32,
        _: &[(f32, Color)],
        _: f32,
        _: f32,
    ) {
        self.uniform_gradient_radii.push(radius);
    }
    fn fill_round_rect_linear_gradient_per_corner(
        &mut self,
        _: Rect,
        radii: [f32; 4],
        _: &[(f32, Color)],
        _: f32,
        _: f32,
    ) {
        self.per_corner_gradient_radii.push(radii);
    }
    fn stroke_round_rect(&mut self, _: Rect, _: f32, _: Color, _: f32) {
        self.uniform_strokes += 1;
    }
    fn stroke_round_rect_per_corner(&mut self, _: Rect, radii: [f32; 4], _: Color, _: f32) {
        self.per_corner_strokes.push(radii);
    }
    fn stroke_svg_path(&mut self, _: &str, _: Point2D, _: f32, _: Color, _: f32) {}
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
}

fn painted(radii: [f32; 4]) -> RadiusCaptureBackend {
    let mut node = SceneNode::leaf("r", NodeKind::Rect);
    node.corner_radius = radii.iter().copied().fold(0.0, f32::max);
    node.corner_radii = Some(radii);
    node.fill = Some(Color::BLACK);
    node.stroke = Some(SceneStroke {
        color: Color::RED,
        width: 2.0,
        sides: None,
        align: SceneStrokeAlign::Center,
    });
    let mut backend = RadiusCaptureBackend::default();
    paint_fill_then_stroke(
        &mut PaintCx {
            backend: &mut backend,
        },
        &node,
        Rect::xywh(0.0, 0.0, 100.0, 50.0),
        1.0,
        node.fill,
    );
    backend
}

#[test]
fn differing_radii_use_per_corner_backend_calls() {
    let backend = painted([8.0, 0.0, 8.0, 0.0]);
    assert_eq!(backend.per_corner_fills, vec![[8.0, 0.0, 8.0, 0.0]]);
    assert_eq!(backend.per_corner_strokes, vec![[8.0, 0.0, 8.0, 0.0]]);
    assert_eq!((backend.uniform_fills, backend.uniform_strokes), (0, 0));
}

#[test]
fn equal_radii_keep_uniform_backend_calls() {
    let backend = painted([8.0; 4]);
    assert!(backend.per_corner_fills.is_empty());
    assert!(backend.per_corner_strokes.is_empty());
    assert_eq!((backend.uniform_fills, backend.uniform_strokes), (1, 1));
}

#[test]
fn differing_radii_do_not_use_uniform_gradient_fill() {
    let mut node = SceneNode::leaf("gradient", NodeKind::Rect);
    node.corner_radius = 8.0;
    node.corner_radii = Some([8.0, 0.0, 8.0, 0.0]);
    node.gradient = Some(SceneGradient::Linear {
        angle_deg: 90.0,
        opacity: 1.0,
        stops: vec![
            SceneGradientStop {
                offset: 0.0,
                color: Color::BLACK,
            },
            SceneGradientStop {
                offset: 1.0,
                color: Color::WHITE,
            },
        ],
    });
    let mut backend = RadiusCaptureBackend::default();
    paint_fill_then_stroke(
        &mut PaintCx {
            backend: &mut backend,
        },
        &node,
        Rect::xywh(0.0, 0.0, 100.0, 50.0),
        1.0,
        node.fill,
    );

    assert!(
        backend.uniform_gradient_radii.is_empty(),
        "a per-corner gradient must not go through the scalar-radius fill"
    );
    assert_eq!(
        backend.per_corner_gradient_radii,
        vec![[8.0, 0.0, 8.0, 0.0]]
    );
}
