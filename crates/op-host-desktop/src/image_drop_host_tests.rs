//! Drop routing tests: which arm of the gesture runs, and what lands in the
//! document. The winit event plumbing above this layer is a thin shell and is
//! covered by the manual checklist instead.

use super::{apply_image_drop, is_supported_image_drop, ImageDropOutcome};
use op_editor_core::fills::first_image_fill_summary;
use op_editor_core::walkers::find_node;
use op_editor_core::{ImageFillMode, NodeId};
use op_host_native::widget_host::WidgetHostNative;
use std::path::{Path, PathBuf};

const VIEWPORT_W: f32 = 1440.0;
const VIEWPORT_H: f32 = 900.0;

const PLACEHOLDER: &str = r#"{"version":"1.0.0","children":[
  {"type":"frame","id":"slot","name":"Screenshot slot","x":400,"y":200,
   "width":400,"height":300,"children":[
     {"type":"text","id":"hint","content":"Drop a screenshot","x":20,"y":40,
      "width":200,"fontSize":16}
   ]}
]}"#;

fn seed() -> WidgetHostNative {
    let mut host = WidgetHostNative::new();
    let doc = jian_ops_schema::load_str(PLACEHOLDER)
        .expect("fixture JSON parses")
        .value;
    *host.editor_state_mut() = op_editor_core::EditorState::from_document(doc);
    host.mark_editor_state_dirty();
    host
}

/// Screen rect of the placeholder frame, resolved through the host so the
/// test never has to reimplement the canvas-origin math.
fn slot_rect(host: &mut WidgetHostNative) -> op_editor_ui::Rect {
    let _ = host.layout_scene();
    host.node_screen_rect(&NodeId::new("slot"), VIEWPORT_W, VIEWPORT_H)
        .expect("placeholder frame is in the scene")
}

/// A point inside the placeholder.
fn point_on_slot(host: &mut WidgetHostNative) -> (f32, f32) {
    let rect = slot_rect(host);
    (
        rect.origin.x + rect.size.x / 2.0,
        rect.origin.y + rect.size.y / 2.0,
    )
}

/// A point on bare canvas: left of the placeholder, clear of the floating
/// toolbar column and of the bottom-centred chat panel.
fn point_off_slot(host: &mut WidgetHostNative) -> (f32, f32) {
    let rect = slot_rect(host);
    (rect.origin.x - 120.0, rect.origin.y + 50.0)
}

/// A real 7×5 PNG on disk — the drop path decodes it for the fill's
/// `originalSize`, so a fake byte blob would not exercise that.
fn write_png(tag: &str) -> PathBuf {
    let mut surface = skia_safe::surfaces::raster_n32_premul((7, 5)).expect("surface");
    surface.canvas().clear(skia_safe::Color::BLUE);
    let png = surface
        .image_snapshot()
        .encode(None, skia_safe::EncodedImageFormat::PNG, 100)
        .expect("encode png");
    let path = std::env::temp_dir().join(format!("op-image-drop-{tag}-{}.png", std::process::id()));
    std::fs::write(&path, png.as_bytes()).expect("write png");
    path
}

#[test]
fn only_raster_image_extensions_take_the_drop_path() {
    for name in ["shot.png", "SHOT.PNG", "a.jpg", "a.jpeg", "a.webp", "a.gif"] {
        assert!(is_supported_image_drop(Path::new(name)), "{name}");
    }
    for name in [
        "design.op",
        "design.pen",
        "export.fig",
        "page.html",
        "logo.svg",
    ] {
        assert!(!is_supported_image_drop(Path::new(name)), "{name}");
    }
}

#[test]
fn a_drop_over_a_placeholder_fills_it_with_the_decoded_bitmap() {
    let mut host = seed();
    let path = write_png("fill");
    let point = point_on_slot(&mut host);

    let outcome = apply_image_drop(&mut host, &path, Some(point), VIEWPORT_W, VIEWPORT_H);
    let _ = std::fs::remove_file(&path);

    assert_eq!(outcome, ImageDropOutcome::Filled(NodeId::new("slot")));
    let summary = find_node(host.editor_state().active_children(), &NodeId::new("slot"))
        .and_then(first_image_fill_summary)
        .expect("image fill written");
    assert!(summary
        .image_url
        .as_deref()
        .is_some_and(|url| url.starts_with("data:image/png;base64,")));
    assert_eq!(summary.mode, ImageFillMode::Fill);
    assert_eq!(summary.original_size, Some([7.0, 5.0]));
}

#[test]
fn a_drop_over_bare_canvas_inserts_a_node_instead() {
    let mut host = seed();
    let path = write_png("insert");
    let point = point_off_slot(&mut host);
    let (doc_x, doc_y) = host
        .canvas_doc_point(point.0, point.1, VIEWPORT_W, VIEWPORT_H)
        .expect("the fallback point is over the canvas");

    let outcome = apply_image_drop(&mut host, &path, Some(point), VIEWPORT_W, VIEWPORT_H);
    let _ = std::fs::remove_file(&path);

    let ImageDropOutcome::Inserted(id) = outcome else {
        panic!("expected an insert, got {outcome:?}");
    };
    let node =
        find_node(host.editor_state().active_children(), &id).expect("inserted node is in the doc");
    let jian_ops_schema::node::PenNode::Image(image) = node else {
        panic!("expected an image node");
    };
    // 7×5 is under the 300 px cap, so it keeps its natural size, centred.
    assert_eq!(image.base.x, Some(doc_x - 3.5));
    assert_eq!(image.base.y, Some(doc_y - 2.5));
}

/// Platforms without a live drag position (everything but macOS today) must
/// still accept the file — they just cannot aim it.
#[test]
fn a_drop_without_a_position_falls_back_to_the_viewport_centre() {
    let mut host = seed();
    let path = write_png("nopoint");

    let outcome = apply_image_drop(&mut host, &path, None, VIEWPORT_W, VIEWPORT_H);
    let _ = std::fs::remove_file(&path);

    assert!(matches!(outcome, ImageDropOutcome::Inserted(_)));
    assert!(
        find_node(host.editor_state().active_children(), &NodeId::new("slot"))
            .and_then(first_image_fill_summary)
            .is_none(),
        "a position-less drop must not guess a fill target"
    );
}

#[test]
fn an_unreadable_file_changes_nothing() {
    let mut host = seed();
    let before = host.editor_state().active_children().len();
    let missing = std::env::temp_dir().join("op-image-drop-does-not-exist.png");

    let outcome = apply_image_drop(&mut host, &missing, None, VIEWPORT_W, VIEWPORT_H);

    assert_eq!(outcome, ImageDropOutcome::Ignored);
    assert_eq!(host.editor_state().active_children().len(), before);
}
