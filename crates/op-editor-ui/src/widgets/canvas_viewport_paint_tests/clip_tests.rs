//! `clipContent` child clipping, mask and composite-layer tests for `canvas_viewport_paint.rs`.
//!
//! Split out of `canvas_viewport_paint_tests.rs` to keep every file
//! under the repository's 800-line cap.

use crate::layout_scene::{MaskType, NodeKind, SceneNode, SceneStroke, SceneStrokeAlign};
use crate::widgets::canvas_viewport_paint::{
    paint_node_with_options, paint_scene_nodes_with_options_hiding,
};
use crate::widgets::PaintCx;
use crate::{Color, ImageBlendMode, Point2D, Rect, RenderBackend, TextLayout};

/// Records the paint-op sequence so the test can assert the clip
/// brackets the children (and only the children).
#[derive(Default)]
struct ClipCaptureBackend {
    ops: Vec<String>,
    content_layer_bounds: Vec<Rect>,
    image_decode_checks: Vec<u64>,
}

impl RenderBackend for ClipCaptureBackend {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, rect: Rect, _: Color) {
        self.ops
            .push(format!("fill({},{})", rect.origin.x, rect.origin.y));
    }
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32) {
        self.ops.push("stroke".into());
    }
    fn draw_text(&mut self, _: &TextLayout, _: Point2D) {}
    fn clip_rect(&mut self, rect: Rect) {
        self.ops.push(format!(
            "clip({},{},{},{})",
            rect.origin.x, rect.origin.y, rect.size.x, rect.size.y
        ));
    }
    fn clip_round_rect(&mut self, rect: Rect, radius: f32) {
        self.ops.push(format!(
            "clip_rr({},{},{},{},r={radius})",
            rect.origin.x, rect.origin.y, rect.size.x, rect.size.y
        ));
    }
    fn clip_svg_path_in_rect(&mut self, d: &str, rect: Rect, even_odd: bool) {
        self.ops.push(format!(
            "clip_path({d},{},{},{},{},evenodd={even_odd})",
            rect.origin.x, rect.origin.y, rect.size.x, rect.size.y
        ));
    }
    fn save(&mut self) {
        self.ops.push("save".into());
    }
    fn push_composite_layer(&mut self, bounds: Rect, _: f32, _: ImageBlendMode) {
        self.content_layer_bounds.push(bounds);
        self.ops.push("content_layer".into());
    }
    fn supports_pixel_masks(&self) -> bool {
        true
    }
    fn push_mask_source_layer(&mut self, luminance: bool) {
        self.ops.push(format!("mask_layer(luma={luminance})"));
    }
    fn restore(&mut self) {
        self.ops.push("restore".into());
    }
    fn translate(&mut self, _: Point2D) {}
    fn stroke_line(&mut self, _: Point2D, _: Point2D, _: Color, _: f32) {}
    fn fill_round_rect(&mut self, rect: Rect, _: f32, _: Color) {
        self.ops
            .push(format!("fill({},{})", rect.origin.x, rect.origin.y));
    }
    fn stroke_round_rect(&mut self, _: Rect, _: f32, _: Color, _: f32) {
        self.ops.push("stroke".into());
    }
    fn stroke_svg_path(&mut self, _: &str, _: Point2D, _: f32, _: Color, _: f32) {}
    fn image_decoded(&mut self, id: u64, _: &[u8], _: u32) -> bool {
        self.image_decode_checks.push(id);
        true
    }
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
}

fn paint_node(cx: &mut PaintCx<'_>, node: &SceneNode, cull: Rect) {
    let _ = paint_node_with_options(
        cx,
        node,
        Point2D::ZERO,
        1.0,
        None,
        cull,
        None,
        None,
        None,
        None,
    );
}

fn frame_with_child(clip: bool, corner_radius: f32) -> SceneNode {
    let mut child = SceneNode::leaf("c", NodeKind::Rect);
    child.bounds = Rect::xywh(10.0, 10.0, 500.0, 20.0);
    child.fill = Some(Color::RED);
    let mut frame = SceneNode::leaf("f", NodeKind::Frame);
    frame.bounds = Rect::xywh(0.0, 0.0, 100.0, 100.0);
    frame.fill = Some(Color::WHITE);
    frame.clip_content = clip;
    frame.corner_radius = corner_radius;
    frame.children = vec![child];
    frame
}

fn capture(node: &SceneNode, cull: Rect) -> ClipCaptureBackend {
    let mut backend = ClipCaptureBackend::default();
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        paint_node(&mut cx, node, cull);
    }
    backend
}

fn paint(node: &SceneNode) -> Vec<String> {
    capture(node, Rect::xywh(0.0, 0.0, 4000.0, 4000.0)).ops
}

fn add_center_stroke(node: &mut SceneNode) {
    node.stroke = Some(SceneStroke {
        color: Color::BLACK,
        width: 2.0,
        sides: None,
        align: SceneStrokeAlign::Center,
    });
}

#[test]
fn clipped_round_container_stroke_overlays_children_once() {
    for kind in [NodeKind::Frame, NodeKind::Rect] {
        let mut container = frame_with_child(true, 9999.0);
        container.kind = kind;
        add_center_stroke(&mut container);

        let ops = paint(&container);
        assert_eq!(
            ops,
            vec![
                "fill(0,0)".to_string(),
                "save".to_string(),
                "clip_rr(0,0,100,100,r=50)".to_string(),
                "fill(10,10)".to_string(),
                "restore".to_string(),
                "stroke".to_string(),
            ]
        );
        assert_eq!(
            ops.iter().filter(|op| op.as_str() == "stroke").count(),
            1,
            "container stroke must be painted exactly once"
        );
    }
}

#[test]
fn leaf_frame_keeps_fill_then_single_stroke() {
    let mut frame = SceneNode::leaf("leaf", NodeKind::Frame);
    frame.bounds = Rect::xywh(0.0, 0.0, 100.0, 100.0);
    frame.fill = Some(Color::WHITE);
    add_center_stroke(&mut frame);

    assert_eq!(
        paint(&frame),
        vec!["fill(0,0)".to_string(), "stroke".to_string()]
    );
}

#[test]
fn clip_content_frame_brackets_children_with_sharp_clip() {
    let ops = paint(&frame_with_child(true, 0.0));
    // Own fill paints UN-clipped, then save → clip → child → restore.
    assert_eq!(
        ops,
        vec![
            "fill(0,0)".to_string(),
            "save".to_string(),
            "clip(0,0,100,100)".to_string(),
            "fill(10,10)".to_string(),
            "restore".to_string(),
        ]
    );
}

#[test]
fn clip_content_uses_rounded_clip_clamped_to_half_height() {
    // Authored radius 80 clamps to h/2 = 50 (TS flattener rule).
    let ops = paint(&frame_with_child(true, 80.0));
    assert!(
        ops.contains(&"clip_rr(0,0,100,100,r=50)".to_string()),
        "{ops:?}"
    );
}

#[test]
fn frame_without_clip_content_paints_children_unclipped() {
    let ops = paint(&frame_with_child(false, 0.0));
    assert_eq!(
        ops,
        vec!["fill(0,0)".to_string(), "fill(10,10)".to_string()]
    );
}

#[test]
fn offscreen_image_container_skips_its_complete_subtree() {
    let mut child = filled_rect("child", 1_020.0);
    child.bounds.origin.y = 1_020.0;
    let mut frame = SceneNode::leaf("image-frame", NodeKind::Frame);
    frame.bounds = Rect::xywh(1_000.0, 1_000.0, 100.0, 100.0);
    frame.image_src = Some("data:image/png;base64,QUJD".into());
    frame.image_src_id = 42;
    frame.children = vec![child];

    let capture = capture(&frame, Rect::xywh(0.0, 0.0, 100.0, 100.0));

    assert!(
        capture.ops.is_empty(),
        "an offscreen container must not paint itself or visit descendants"
    );
    assert!(
        capture.image_decode_checks.is_empty(),
        "an offscreen image-filled container must not enter the decode path"
    );
}

#[test]
fn open_offscreen_container_keeps_visible_overflow_descendant() {
    let mut frame = SceneNode::leaf("open-frame", NodeKind::Frame);
    frame.bounds = Rect::xywh(1_000.0, 1_000.0, 100.0, 100.0);
    frame.children = vec![filled_rect("visible-overflow", 20.0)];

    let capture = capture(&frame, Rect::xywh(0.0, 0.0, 100.0, 100.0));

    assert!(
        capture.ops.contains(&"fill(20,0)".to_string()),
        "an unclipped descendant that reaches the viewport must still paint"
    );
}

#[test]
fn clip_content_group_clips_children_too() {
    let mut group = frame_with_child(true, 0.0);
    group.kind = NodeKind::Group;
    group.fill = None;
    let ops = paint(&group);
    assert_eq!(
        ops,
        vec![
            "save".to_string(),
            "clip(0,0,100,100)".to_string(),
            "fill(10,10)".to_string(),
            "restore".to_string(),
        ]
    );
}

fn filled_rect(id: &str, x: f32) -> SceneNode {
    let mut node = SceneNode::leaf(id, NodeKind::Rect);
    node.bounds = Rect::xywh(x, 0.0, 10.0, 10.0);
    node.fill = Some(Color::RED);
    node
}

fn path_mask(id: &str, x: f32, d: &str) -> SceneNode {
    let mut node = SceneNode::leaf(id, NodeKind::Path);
    node.bounds = Rect::xywh(x, 0.0, 10.0, 10.0);
    node.svg_path = Some(d.to_string());
    node.fill = Some(Color::WHITE);
    node.is_mask = true;
    node
}

#[test]
fn opaque_path_mask_clips_only_front_siblings_and_is_not_painted() {
    let mut frame = SceneNode::leaf("frame", NodeKind::Frame);
    frame.bounds = Rect::xywh(0.0, 0.0, 100.0, 100.0);
    // Scene children are topmost-first. The reverse painter must draw the
    // background, install the mask, then draw the foreground inside it.
    frame.children = vec![
        filled_rect("front", 10.0),
        path_mask("mask", 0.0, "M0 0 L10 0 L10 10 Z"),
        filled_rect("back", 20.0),
    ];

    assert_eq!(
        paint(&frame),
        vec![
            "fill(20,0)".to_string(),
            "save".to_string(),
            "clip_path(M0 0 L10 0 L10 10 Z,0,0,10,10,evenodd=false)".to_string(),
            "fill(10,0)".to_string(),
            "restore".to_string(),
        ]
    );
}

#[test]
fn next_path_mask_starts_a_fresh_sibling_clip_run() {
    let mut frame = SceneNode::leaf("frame", NodeKind::Frame);
    frame.bounds = Rect::xywh(0.0, 0.0, 100.0, 100.0);
    frame.children = vec![
        filled_rect("front", 10.0),
        path_mask("front-mask", 0.0, "M0 0 L10 0 L10 10 Z"),
        filled_rect("middle", 20.0),
        path_mask("back-mask", 40.0, "M0 0 L10 0 L10 10 Z"),
        filled_rect("back", 30.0),
    ];

    assert_eq!(
        paint(&frame),
        vec![
            "fill(30,0)".to_string(),
            "save".to_string(),
            "clip_path(M0 0 L10 0 L10 10 Z,40,0,10,10,evenodd=false)".to_string(),
            "fill(20,0)".to_string(),
            "restore".to_string(),
            "save".to_string(),
            "clip_path(M0 0 L10 0 L10 10 Z,0,0,10,10,evenodd=false)".to_string(),
            "fill(10,0)".to_string(),
            "restore".to_string(),
        ]
    );
}

fn pixel_mask(id: &str, kind: NodeKind, mask_type: MaskType, alpha: f32) -> SceneNode {
    let mut node = SceneNode::leaf(id, kind);
    node.bounds = Rect::xywh(0.0, 0.0, 10.0, 10.0);
    node.fill = Some(Color::WHITE.with_alpha(alpha));
    node.mask_type = Some(mask_type);
    node.is_mask = true;
    node
}

#[test]
fn translucent_alpha_mask_uses_two_layers_and_is_deferred() {
    let mut frame = SceneNode::leaf("frame", NodeKind::Frame);
    frame.bounds = Rect::xywh(0.0, 0.0, 100.0, 100.0);
    frame.children = vec![
        filled_rect("front", 10.0),
        pixel_mask("mask", NodeKind::Rect, MaskType::Alpha, 0.5),
        filled_rect("back", 20.0),
    ];
    assert_eq!(
        paint(&frame),
        vec![
            "fill(20,0)",
            "content_layer",
            "fill(10,0)",
            "mask_layer(luma=false)",
            "fill(0,0)",
            "restore",
            "restore",
        ]
    );
}

#[test]
fn pixel_mask_layer_is_bounded_to_mask_and_its_front_run() {
    let mut frame = SceneNode::leaf("frame", NodeKind::Frame);
    frame.bounds = Rect::xywh(0.0, 0.0, 100.0, 100.0);
    frame.children = vec![
        filled_rect("front", 10.0),
        pixel_mask("mask", NodeKind::Rect, MaskType::Alpha, 0.5),
        filled_rect("back", 80.0),
    ];
    let cull = Rect::xywh(0.0, 0.0, 4_000.0, 4_000.0);
    let capture = capture(&frame, cull);
    assert_eq!(
        capture.content_layer_bounds,
        vec![Rect::xywh(0.0, 0.0, 21.0, 11.0)]
    );
    assert_ne!(capture.content_layer_bounds[0], cull);
}

#[test]
fn consecutive_masks_bound_each_sibling_run_independently() {
    let mut frame = SceneNode::leaf("frame", NodeKind::Frame);
    frame.bounds = Rect::xywh(0.0, 0.0, 100.0, 100.0);
    frame.children = vec![
        filled_rect("front", 10.0),
        pixel_mask("front-mask", NodeKind::Rect, MaskType::Alpha, 0.5),
        filled_rect("middle", 20.0),
        {
            let mut mask = pixel_mask("back-mask", NodeKind::Rect, MaskType::Alpha, 0.5);
            mask.bounds.origin.x = 40.0;
            mask
        },
        filled_rect("back", 80.0),
    ];
    let capture = capture(&frame, Rect::xywh(0.0, 0.0, 4_000.0, 4_000.0));
    assert_eq!(
        capture.content_layer_bounds,
        vec![
            Rect::xywh(19.0, 0.0, 32.0, 11.0),
            Rect::xywh(0.0, 0.0, 21.0, 11.0),
        ]
    );
}

#[test]
fn luminance_mask_requests_luma_before_dst_in() {
    let mut frame = SceneNode::leaf("frame", NodeKind::Frame);
    frame.bounds = Rect::xywh(0.0, 0.0, 100.0, 100.0);
    frame.children = vec![
        filled_rect("front", 10.0),
        pixel_mask("mask", NodeKind::Rect, MaskType::Luminance, 1.0),
    ];
    let ops = paint(&frame);
    assert!(
        ops.contains(&"mask_layer(luma=true)".to_string()),
        "{ops:?}"
    );
    assert!(!ops.iter().any(|op| op.starts_with("clip_path")), "{ops:?}");
}

#[test]
fn frame_mask_renders_its_subtree_as_the_mask_source() {
    let mut mask = pixel_mask("mask", NodeKind::Frame, MaskType::Alpha, 0.0);
    mask.fill = None;
    mask.children = vec![filled_rect("mask-child", 40.0)];
    let mut frame = SceneNode::leaf("frame", NodeKind::Frame);
    frame.bounds = Rect::xywh(0.0, 0.0, 100.0, 100.0);
    frame.children = vec![filled_rect("front", 10.0), mask];
    let ops = paint(&frame);
    let start = ops
        .iter()
        .position(|op| op == "mask_layer(luma=false)")
        .unwrap();
    assert_eq!(ops[start + 1], "fill(40,0)", "{ops:?}");
}

#[test]
fn page_root_siblings_use_the_mask_aware_walk() {
    let nodes = vec![
        filled_rect("front", 10.0),
        pixel_mask("mask", NodeKind::Rect, MaskType::Alpha, 0.5),
        filled_rect("back", 20.0),
    ];
    let mut backend = ClipCaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    let _ = paint_scene_nodes_with_options_hiding(
        &mut cx,
        &nodes,
        Point2D::ZERO,
        1.0,
        None,
        Rect::xywh(0.0, 0.0, 100.0, 100.0),
        None,
        None,
        None,
        None,
        None,
        0,
        None,
        None,
        None,
        false,
    );
    assert_eq!(backend.ops[0], "fill(20,0)");
    assert!(backend.ops.contains(&"content_layer".to_string()));
    assert!(backend.ops.contains(&"mask_layer(luma=false)".to_string()));
}

#[test]
fn zero_sized_alpha_mask_still_creates_an_empty_dst_in_source() {
    let mut mask = pixel_mask("zero", NodeKind::Rect, MaskType::Alpha, 1.0);
    mask.bounds.size.x = 0.0;
    let mut frame = SceneNode::leaf("frame", NodeKind::Frame);
    frame.bounds = Rect::xywh(0.0, 0.0, 100.0, 100.0);
    frame.children = vec![filled_rect("front", 10.0), mask];
    let ops = paint(&frame);
    assert!(ops.contains(&"content_layer".to_string()), "{ops:?}");
    assert!(
        ops.contains(&"mask_layer(luma=false)".to_string()),
        "{ops:?}"
    );
}
