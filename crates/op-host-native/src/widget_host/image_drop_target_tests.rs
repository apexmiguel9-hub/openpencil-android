//! Drop-point → drop-target resolution tests.
//!
//! Geometry discipline mirrors the other canvas-press suites: viewport
//! 1440×900 and fixtures at doc x ≥ 400 so every probe lands clear of the
//! floating toolbar and chat panel.

use super::WidgetHostNative;
use op_editor_core::NodeId;

const VIEWPORT_W: f32 = 1440.0;
const VIEWPORT_H: f32 = 900.0;

/// A screenshot placeholder: a frame with a hint label and an icon box,
/// exactly the shape the template ships.
const PLACEHOLDER: &str = r#"{"version":"1.0.0","children":[
  {"type":"frame","id":"slot","name":"Screenshot slot","x":400,"y":200,
   "width":400,"height":300,"children":[
     {"type":"text","id":"hint","content":"Drop a screenshot","x":20,"y":40,
      "width":200,"fontSize":16}
   ]}
]}"#;

fn seed(json: &str) -> WidgetHostNative {
    let mut host = WidgetHostNative::new();
    let doc = jian_ops_schema::load_str(json)
        .expect("fixture JSON parses")
        .value;
    *host.editor_state_mut() = op_editor_core::EditorState::from_document(doc);
    host.mark_paint_dirty_for_test();
    host
}

/// Screen point for a doc point (zoom 1, pan 0 in a fresh host).
fn screen_at(host: &WidgetHostNative, doc_x: f32, doc_y: f32) -> (f32, f32) {
    let (cx0, cy0, _cw, _ch) = host.canvas_region(VIEWPORT_W, VIEWPORT_H);
    (cx0 + doc_x, cy0 + doc_y)
}

#[test]
fn a_drop_over_the_hint_label_targets_the_placeholder_frame() {
    let mut host = seed(PLACEHOLDER);
    let (x, y) = screen_at(&host, 430.0, 250.0);
    assert_eq!(
        host.image_drop_target_at(x, y, VIEWPORT_W, VIEWPORT_H),
        Some(NodeId::new("slot"))
    );
}

#[test]
fn a_drop_over_bare_canvas_has_no_target_but_still_maps_to_a_doc_point() {
    let mut host = seed(PLACEHOLDER);
    let (x, y) = screen_at(&host, 1000.0, 700.0);
    assert_eq!(
        host.image_drop_target_at(x, y, VIEWPORT_W, VIEWPORT_H),
        None
    );
    let point = host
        .canvas_doc_point(x, y, VIEWPORT_W, VIEWPORT_H)
        .expect("bare canvas still resolves a doc point");
    assert_eq!(point, (1000.0, 700.0));
}

/// The canvas origin moves with the sidebar, so a point over the rail is not
/// a canvas point at all — dropping there must not fill anything.
#[test]
fn a_drop_over_the_left_rail_is_not_a_canvas_drop() {
    let mut host = seed(PLACEHOLDER);
    host.editor_state_mut().editor_ui.sidebar_open = true;
    host.mark_paint_dirty_for_test();
    assert_eq!(
        host.image_drop_target_at(10.0, 400.0, VIEWPORT_W, VIEWPORT_H),
        None
    );
    assert_eq!(
        host.canvas_doc_point(10.0, 400.0, VIEWPORT_W, VIEWPORT_H),
        None
    );
}

/// Preview mode runs the design as an app; a file dropped on it must not
/// silently edit the document behind the running screen.
#[test]
fn preview_mode_refuses_drop_targets() {
    let mut host = seed(PLACEHOLDER);
    host.editor_state_mut().editor_ui.preview.mode = true;
    let (x, y) = screen_at(&host, 430.0, 250.0);
    assert_eq!(
        host.image_drop_target_at(x, y, VIEWPORT_W, VIEWPORT_H),
        None
    );
}

#[test]
fn the_target_ring_rect_follows_pan_and_zoom() {
    let mut host = seed(PLACEHOLDER);
    host.editor_state_mut().viewport.zoom = 2.0;
    host.editor_state_mut().viewport.pan_x = 30.0;
    host.editor_state_mut().viewport.pan_y = -40.0;
    host.mark_paint_dirty_for_test();
    let _ = host.layout_scene();

    let (cx0, cy0, _cw, _ch) = host.canvas_region(VIEWPORT_W, VIEWPORT_H);
    let rect = host
        .node_screen_rect(&NodeId::new("slot"), VIEWPORT_W, VIEWPORT_H)
        .expect("frame is in the scene");
    assert_eq!(rect.origin.x, cx0 + 30.0 + 400.0 * 2.0);
    assert_eq!(rect.origin.y, cy0 - 40.0 + 200.0 * 2.0);
    assert_eq!(rect.size.x, 800.0);
    assert_eq!(rect.size.y, 600.0);
}

#[test]
fn applying_a_drop_dirties_the_host_and_can_be_undone_in_one_step() {
    let mut host = seed(PLACEHOLDER);
    let target = NodeId::new("slot");
    assert!(host.apply_image_drop(&target, "data:image/png;base64,AAAA", Some([800.0, 600.0])));

    let filled = op_editor_core::walkers::find_node(host.editor_state().active_children(), &target)
        .and_then(op_editor_core::fills::first_image_fill_summary)
        .expect("image fill written");
    assert_eq!(
        filled.image_url.as_deref(),
        Some("data:image/png;base64,AAAA")
    );

    host.editor_state_mut().undo();
    let after = op_editor_core::walkers::find_node(host.editor_state().active_children(), &target)
        .and_then(op_editor_core::fills::first_image_fill_summary);
    assert!(after.is_none(), "one drop is one undo step");
}

#[test]
fn a_bare_canvas_drop_inserts_the_image_centred_on_the_drop_point() {
    let mut host = seed(PLACEHOLDER);
    let id = host
        .insert_dropped_image(
            "Shot",
            "data:image/png;base64,AAAA",
            Some((600, 400)),
            (1000.0, 700.0),
        )
        .expect("image node inserted");

    let node = op_editor_core::walkers::find_node(host.editor_state().active_children(), &id)
        .expect("inserted node");
    let jian_ops_schema::node::PenNode::Image(image) = node else {
        panic!("expected an image node");
    };
    assert_eq!(image.base.x, Some(850.0));
    assert_eq!(image.base.y, Some(600.0));
}
