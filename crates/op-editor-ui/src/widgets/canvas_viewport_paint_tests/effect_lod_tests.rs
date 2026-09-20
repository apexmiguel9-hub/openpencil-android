//! Sub-pixel effect LOD skip tests for `canvas_viewport_paint.rs`.
//!
//! Split out of `canvas_viewport_paint_tests.rs` to keep every file
//! under the repository's 800-line cap.

use crate::layout_scene::{DropShadow, Effect, NodeKind, SceneNode};
use crate::widgets::canvas_viewport_paint::paint_node;
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect, RenderBackend, TextLayout};

/// Records the expensive effect ops so tests can assert sub-pixel
/// effects skip their save-layers at low zoom.
#[derive(Default)]
struct EffectCaptureBackend {
    ops: Vec<&'static str>,
}

impl RenderBackend for EffectCaptureBackend {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, _: Rect, _: Color) {
        self.ops.push("fill");
    }
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn draw_text(&mut self, _: &TextLayout, _: Point2D) {}
    fn clip_rect(&mut self, _: Rect) {}
    fn save(&mut self) {}
    fn restore(&mut self) {}
    fn push_blur_layer(&mut self, _: f32) {
        self.ops.push("blur");
    }
    fn push_backdrop_blur_layer(&mut self, _: f32) {
        self.ops.push("backdrop");
    }
    fn fill_drop_shadow(&mut self, _: Rect, _: f32, _: f32, _: Color) {
        self.ops.push("shadow");
    }
    fn translate(&mut self, _: Point2D) {}
    fn stroke_line(&mut self, _: Point2D, _: Point2D, _: Color, _: f32) {}
    fn fill_round_rect(&mut self, _: Rect, _: f32, _: Color) {}
    fn stroke_round_rect(&mut self, _: Rect, _: f32, _: Color, _: f32) {}
    fn stroke_svg_path(&mut self, _: &str, _: Point2D, _: f32, _: Color, _: f32) {}
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
}

fn effect_node(effects: Vec<Effect>) -> SceneNode {
    let mut node = SceneNode::leaf("fx", NodeKind::Rect);
    node.bounds = Rect::xywh(0.0, 0.0, 100.0, 60.0);
    node.fill = Some(Color::BLACK);
    node.effects = effects;
    node
}

fn ops_at_zoom(node: &SceneNode, zoom: f32) -> Vec<&'static str> {
    let mut backend = EffectCaptureBackend::default();
    paint_node(
        &mut PaintCx {
            backend: &mut backend,
        },
        node,
        Point2D::ZERO,
        zoom,
        Rect::xywh(-1000.0, -1000.0, 4000.0, 4000.0),
    );
    backend.ops
}

fn ops_fast(node: SceneNode) -> Vec<&'static str> {
    let mut backend = EffectCaptureBackend::default();
    let nodes = vec![node];
    let _ = crate::widgets::canvas_viewport_paint::paint_scene_nodes_with_options_hiding(
        &mut PaintCx {
            backend: &mut backend,
        },
        &nodes,
        Point2D::ZERO,
        1.0,
        None,
        Rect::xywh(-1000.0, -1000.0, 4000.0, 4000.0),
        None,
        None,
        None,
        None,
        None,
        0,
        None,
        None,
        None,
        true,
    );
    backend.ops
}

#[test]
fn fast_interaction_skips_every_effect_layer_at_any_zoom() {
    // During an active pan/zoom gesture the frame budget matters
    // more than effect fidelity (Figma-style interactive degrade):
    // shadows, layer blurs, and backdrop blurs all skip even at
    // fully visible sizes. Quality returns on gesture end.
    let node = effect_node(vec![
        Effect::DropShadow(DropShadow {
            offset_x: 4.0,
            offset_y: 4.0,
            blur: 12.0,
            color: Color::BLACK,
            inner: false,
        }),
        Effect::Blur(crate::layout_scene::BlurEffect { radius: 12.0 }),
        Effect::BackgroundBlur { radius: 12.0 },
    ]);
    let ops = ops_fast(node);
    assert!(!ops.contains(&"shadow"));
    assert!(!ops.contains(&"blur"));
    assert!(!ops.contains(&"backdrop"));
}

#[test]
fn fast_interaction_skips_subpixel_leaves_but_keeps_visible_ones() {
    let mut tiny = SceneNode::leaf("tiny", NodeKind::Rect);
    tiny.bounds = Rect::xywh(0.0, 0.0, 0.5, 0.5);
    tiny.fill = Some(Color::BLACK);
    let mut big = SceneNode::leaf("big", NodeKind::Rect);
    big.bounds = Rect::xywh(10.0, 10.0, 100.0, 100.0);
    big.fill = Some(Color::BLACK);
    let mut frame = SceneNode::leaf("f", NodeKind::Frame);
    frame.bounds = Rect::xywh(0.0, 0.0, 200.0, 200.0);
    frame.children = vec![tiny, big];

    // Fast mode: the frame + the visible leaf fill; the sub-pixel
    // leaf paints nothing.
    assert_eq!(
        ops_fast(frame.clone())
            .iter()
            .filter(|op| **op == "fill")
            .count(),
        1
    );
}

#[test]
fn subpixel_blur_skips_the_blur_save_layer() {
    // 4 px radius → sigma 2 at zoom 1 (visible), sigma 0.1 device
    // px at 5% zoom — invisible, but the save-layer still broke the
    // GPU render pass. A zoomed-out effect-dense page (3.8k blurs
    // visible at once) turned every pan frame into thousands of
    // render-pass submits.
    let node = effect_node(vec![Effect::Blur(crate::layout_scene::BlurEffect {
        radius: 4.0,
    })]);
    assert!(ops_at_zoom(&node, 1.0).contains(&"blur"));
    assert!(!ops_at_zoom(&node, 0.05).contains(&"blur"));
}

#[test]
fn subpixel_backdrop_blur_skips_the_backdrop_layer() {
    let node = effect_node(vec![Effect::BackgroundBlur { radius: 4.0 }]);
    assert!(ops_at_zoom(&node, 1.0).contains(&"backdrop"));
    assert!(!ops_at_zoom(&node, 0.05).contains(&"backdrop"));
}

#[test]
fn subpixel_shadow_skips_the_shadow_draw() {
    // Blur AND offset both under a third of a device pixel: the
    // shadow cannot move or soften the silhouette visibly.
    let node = effect_node(vec![Effect::DropShadow(DropShadow {
        offset_x: 2.0,
        offset_y: 2.0,
        blur: 4.0,
        color: Color::BLACK,
        inner: false,
    })]);
    assert!(ops_at_zoom(&node, 1.0).contains(&"shadow"));
    assert!(!ops_at_zoom(&node, 0.05).contains(&"shadow"));
}

#[test]
fn visible_offset_keeps_the_shadow_even_with_tiny_blur() {
    // A hard-edged shadow displaced 40 doc px is still a visible
    // 2 px fringe at 5% zoom — only fully sub-pixel shadows skip.
    let node = effect_node(vec![Effect::DropShadow(DropShadow {
        offset_x: 40.0,
        offset_y: 0.0,
        blur: 0.0,
        color: Color::BLACK,
        inner: false,
    })]);
    assert!(ops_at_zoom(&node, 0.05).contains(&"shadow"));
}
