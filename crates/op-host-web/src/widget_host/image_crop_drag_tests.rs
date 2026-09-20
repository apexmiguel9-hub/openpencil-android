//! Web canvas regression coverage for Figma-style image-fill crop dragging.

use super::WidgetHost;
use op_editor_core::{own_bounds, primary_image_fill_transform, walkers::find_node, NodeId, Tool};
use op_editor_ui::Point2D;

const VIEWPORT_W: f32 = 1200.0;
const VIEWPORT_H: f32 = 800.0;

const CROP_RECT: &str = r#"{
  "version":"1.0.0",
  "children":[{
    "type":"rectangle",
    "id":"photo",
    "name":"Photo",
    "x":100,
    "y":100,
    "width":120,
    "height":80,
    "fill":[{
      "type":"image",
      "url":"asset.png",
      "mode":"crop",
      "originalSize":{"width":240,"height":80},
      "transform":{
        "m00":0.5,
        "m01":0.0,
        "m02":0.25,
        "m10":0.0,
        "m11":1.0,
        "m12":0.0
      }
    }]
  }]
}"#;

const ROTATED_NESTED_CROP: &str = r#"{
  "version":"1.0.0",
  "children":[{
    "type":"frame","id":"parent","x":200,"y":100,"width":300,"height":300,
    "rotation":90,"children":[{
      "type":"rectangle","id":"photo","x":50,"y":50,"width":100,"height":100,
      "fill":[{"type":"image","url":"asset.png","mode":"crop",
        "originalSize":{"width":100,"height":200},
        "transform":{"m00":1.0,"m01":0.0,"m02":0.0,
                     "m10":0.0,"m11":0.5,"m12":0.25}}]
    }]
  }]
}"#;

const DEEP_CROP_HIERARCHY: &str = r#"{
  "version":"1.0.0",
  "children":[{
    "type":"frame","id":"outer","x":100,"y":100,"width":500,"height":400,
    "children":[{
      "type":"frame","id":"middle","x":20,"y":20,"width":400,"height":300,
      "children":[
        {"type":"rectangle","id":"deep-photo","x":20,"y":20,"width":120,"height":80,
         "fill":[{"type":"image","url":"asset.png","mode":"crop",
           "originalSize":{"width":240,"height":80},
           "transform":{"m00":0.5,"m01":0.0,"m02":0.25,
                        "m10":0.0,"m11":1.0,"m12":0.0}}]},
        {"type":"frame","id":"crop-with-child","x":180,"y":20,"width":120,"height":80,
         "fill":[{"type":"image","url":"asset.png","mode":"crop",
           "originalSize":{"width":240,"height":80},
           "transform":{"m00":0.5,"m01":0.0,"m02":0.25,
                        "m10":0.0,"m11":1.0,"m12":0.0}}],
         "children":[
           {"type":"rectangle","id":"crop-child","x":10,"y":10,"width":40,"height":40}
         ]}
      ]
    }]
  }]
}"#;

fn seed() -> WidgetHost {
    let doc = jian_ops_schema::load_str(CROP_RECT)
        .expect("crop fixture parses")
        .value;
    let mut host = WidgetHost::new();
    host.editor_state = op_editor_core::EditorState::from_document(doc);
    host.editor_state.tool = Tool::Select;
    host.editor_state_dirty = true;
    host.last_viewport_w = VIEWPORT_W;
    host.last_viewport_h = VIEWPORT_H;
    host
}

fn seed_nested() -> WidgetHost {
    let doc = jian_ops_schema::load_str(ROTATED_NESTED_CROP)
        .expect("nested crop fixture parses")
        .value;
    let mut host = WidgetHost::new();
    host.editor_state = op_editor_core::EditorState::from_document(doc);
    host.editor_state.tool = Tool::Select;
    host.editor_state_dirty = true;
    host
}

fn seed_deep_hierarchy(selected: &str) -> WidgetHost {
    let doc = jian_ops_schema::load_str(DEEP_CROP_HIERARCHY)
        .expect("deep crop fixture parses")
        .value;
    let mut host = WidgetHost::new();
    host.editor_state = op_editor_core::EditorState::from_document(doc);
    host.editor_state.tool = Tool::Select;
    host.editor_state
        .set_single_selection(NodeId::new(selected));
    host.editor_state_dirty = true;
    host.last_viewport_w = VIEWPORT_W;
    host.last_viewport_h = VIEWPORT_H;
    host
}

fn screen_at(host: &WidgetHost, doc_x: f32, doc_y: f32) -> Point2D {
    let (canvas_x, canvas_y, _, _) = host.canvas_region(VIEWPORT_W, VIEWPORT_H);
    Point2D::new(canvas_x + doc_x, canvas_y + doc_y)
}

fn photo_bounds(host: &WidgetHost) -> op_editor_core::DocRect {
    own_bounds(
        find_node(host.editor_state.active_children(), &NodeId::new("photo"))
            .expect("photo remains in the document"),
    )
}

fn photo_transform(host: &WidgetHost) -> [f32; 6] {
    primary_image_fill_transform(
        find_node(host.editor_state.active_children(), &NodeId::new("photo"))
            .expect("photo remains in the document"),
    )
    .expect("crop keeps an explicit image transform")
}

#[test]
fn crop_edit_drag_changes_only_transform_and_is_one_undo_step() {
    let mut host = seed();
    host.editor_state.set_single_selection(NodeId::new("photo"));
    assert!(host.enter_selected_image_crop_edit());

    let bounds_before = photo_bounds(&host);
    let transform_before = photo_transform(&host);
    let history_before = host.editor_state.history.past.len();
    let press = screen_at(&host, 160.0, 140.0);
    let move_to = screen_at(&host, 184.0, 140.0);

    assert!(host.apply_press(press.x, press.y, VIEWPORT_W, VIEWPORT_H));
    assert!(host.image_crop_drag.is_some());
    assert!(host.node_drag.is_none());
    assert!(
        host.cursor_move_requires_immediate_frame(),
        "an active crop drag must repaint on every pointer move"
    );

    assert!(host.apply_cursor_move(move_to.x, move_to.y));
    assert_ne!(photo_transform(&host), transform_before);
    assert_eq!(
        photo_bounds(&host),
        bounds_before,
        "panning the bitmap must not move or resize its containing node"
    );

    assert!(host.apply_release_with_viewport(VIEWPORT_W, VIEWPORT_H));
    assert!(host.image_crop_drag.is_none());
    assert_eq!(
        host.editor_state.history.past.len(),
        history_before + 1,
        "the whole crop gesture is one history entry"
    );

    assert!(host.apply_undo());
    assert_eq!(photo_transform(&host), transform_before);
    assert_eq!(photo_bounds(&host), bounds_before);
}

#[test]
fn crop_node_without_edit_mode_starts_an_ordinary_node_drag() {
    let mut host = seed();
    host.editor_state.set_single_selection(NodeId::new("photo"));
    let bounds_before = photo_bounds(&host);
    let transform_before = photo_transform(&host);
    let press = screen_at(&host, 160.0, 140.0);
    let move_to = screen_at(&host, 184.0, 152.0);

    assert!(host.apply_press(press.x, press.y, VIEWPORT_W, VIEWPORT_H));
    assert!(host.node_drag.is_some());
    assert!(host.image_crop_drag.is_none());

    assert!(host.apply_cursor_move(move_to.x, move_to.y));
    let bounds_after = photo_bounds(&host);
    assert_ne!(bounds_after.x, bounds_before.x);
    assert_ne!(bounds_after.y, bounds_before.y);
    assert_eq!(bounds_after.w, bounds_before.w);
    assert_eq!(bounds_after.h, bounds_before.h);
    assert_eq!(
        photo_transform(&host),
        transform_before,
        "ordinary node drag must leave the image crop untouched"
    );
    assert!(host.apply_release_with_viewport(VIEWPORT_W, VIEWPORT_H));
}

#[test]
fn double_clicking_selected_crop_enters_crop_edit_mode() {
    let mut host = seed();
    let point = screen_at(&host, 160.0, 140.0);

    host.set_now_ms(1_000);
    assert!(host.apply_press(point.x, point.y, VIEWPORT_W, VIEWPORT_H));
    assert_eq!(host.editor_state.selection.anchor, NodeId::new("photo"));
    assert!(host.apply_release_with_viewport(VIEWPORT_W, VIEWPORT_H));

    host.set_now_ms(1_200);
    assert!(host.apply_press(point.x, point.y, VIEWPORT_W, VIEWPORT_H));
    assert_eq!(
        host.editor_state.editor_ui.image_crop_editing,
        Some(NodeId::new("photo"))
    );
    assert!(host.node_drag.is_none());
    assert!(
        host.image_crop_drag.is_none(),
        "the double click enters edit mode; the following press starts panning"
    );
}

#[test]
fn nested_crop_uses_editing_id_and_inverts_ancestor_rotation() {
    let mut host = seed_nested();
    host.editor_state.set_single_selection(NodeId::new("photo"));
    assert!(host.enter_selected_image_crop_edit());
    let before = photo_transform(&host);

    assert!(host.apply_canvas_node_press(
        vec![NodeId::new("parent"), NodeId::new("photo")],
        0.0,
        0.0,
        false,
        VIEWPORT_H,
    ));
    assert!(host.image_crop_drag.is_some());
    assert!(host.node_drag.is_none());
    assert_eq!(
        host.apply_image_crop_drag_cursor_move(20.0, 0.0),
        Some(true)
    );

    let after = photo_transform(&host);
    assert_eq!(after[2], before[2]);
    assert!(after[5] > before[5]);
}

#[test]
fn layer_selected_deep_crop_leaf_enters_edit_after_two_canvas_presses() {
    let mut host = seed_deep_hierarchy("deep-photo");
    let path = vec![
        NodeId::new("outer"),
        NodeId::new("middle"),
        NodeId::new("deep-photo"),
    ];

    host.set_now_ms(1_000);
    assert!(host.apply_canvas_node_press(path.clone(), 0.0, 0.0, false, VIEWPORT_H));
    assert_eq!(
        host.editor_state.selection.anchor,
        NodeId::new("deep-photo"),
        "the first press preserves the exact Layer-panel crop selection"
    );
    assert!(host.apply_release_with_viewport(VIEWPORT_W, VIEWPORT_H));

    host.set_now_ms(1_200);
    assert!(host.apply_canvas_node_press(path, 0.0, 0.0, false, VIEWPORT_H));
    assert_eq!(
        host.editor_state.editor_ui.image_crop_editing,
        Some(NodeId::new("deep-photo"))
    );
    assert!(host.node_drag.is_none());
}

#[test]
fn selected_crop_with_deeper_child_keeps_one_level_drill_behavior() {
    let mut host = seed_deep_hierarchy("crop-with-child");
    let path = vec![
        NodeId::new("outer"),
        NodeId::new("middle"),
        NodeId::new("crop-with-child"),
        NodeId::new("crop-child"),
    ];

    host.set_now_ms(1_000);
    assert!(host.apply_canvas_node_press(path.clone(), 0.0, 0.0, false, VIEWPORT_H));
    assert!(host.apply_release_with_viewport(VIEWPORT_W, VIEWPORT_H));
    host.set_now_ms(1_200);
    assert!(host.apply_canvas_node_press(path, 0.0, 0.0, false, VIEWPORT_H));

    assert_eq!(host.editor_state.editor_ui.image_crop_editing, None);
    assert_eq!(
        host.editor_state.selection.anchor,
        NodeId::new("crop-with-child"),
        "double press drills one level instead of editing the ancestor crop"
    );
    assert_eq!(
        host.editor_state.editor_ui.entered_container,
        Some(NodeId::new("middle"))
    );
}
