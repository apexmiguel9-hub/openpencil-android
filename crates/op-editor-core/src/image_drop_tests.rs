//! Drop-target resolution + application tests for [`super`].

use crate::fills::first_image_fill_summary;
use crate::pen_node_ext::PenNodeExt;
use crate::walkers::find_node;
use crate::{EditorState, ImageFillMode, NodeId};
use jian_ops_schema::node::PenNode;

const SRC: &str = "data:image/png;base64,AAAA";

fn state_from(src: &str) -> EditorState {
    let doc = jian_ops_schema::load_str(src)
        .expect("fixture parses")
        .value;
    let mut state = EditorState::from_document(doc);
    state.clear_selection();
    state
}

/// A placeholder box in a template is a frame wrapping a hint label and an
/// icon wrapper. The pointer lands on the label, but the drop must fill the
/// BOX — resolved structurally (text paints no droppable area), never by name.
fn placeholder_state() -> EditorState {
    state_from(
        r##"{
      "version":"1.0.0",
      "children":[{
        "type":"frame","id":"shot-slot","name":"Screenshot slot",
        "x":0,"y":0,"width":400,"height":300,
        "children":[
          {"type":"text","id":"hint","content":"Drop a screenshot","x":20,"y":20,
           "width":200,"fontSize":14},
          {"type":"frame","id":"icon-wrap","x":20,"y":60,"width":48,"height":48,
           "children":[]}
        ]
      }]
    }"##,
    )
}

fn history_depth(state: &EditorState) -> usize {
    state.history.past.len()
}

#[test]
fn drop_over_a_label_fills_the_nearest_area_ancestor() {
    let state = placeholder_state();
    let target = state
        .resolve_image_drop_target(&["shot-slot".to_string(), "hint".to_string()])
        .expect("text hit walks up to the frame");
    assert_eq!(target, NodeId::new("shot-slot"));
}

#[test]
fn drop_over_a_nested_frame_fills_that_frame_not_its_parent() {
    let state = placeholder_state();
    let target = state
        .resolve_image_drop_target(&["shot-slot".to_string(), "icon-wrap".to_string()])
        .expect("deepest droppable node wins");
    assert_eq!(target, NodeId::new("icon-wrap"));
}

#[test]
fn drop_over_empty_space_has_no_target() {
    let state = placeholder_state();
    assert_eq!(state.resolve_image_drop_target(&[]), None);
}

#[test]
fn a_locked_box_is_skipped_in_favour_of_an_editable_ancestor() {
    let mut state = placeholder_state();
    let node =
        crate::walkers::find_node_mut(state.active_children_mut(), &NodeId::new("icon-wrap"))
            .expect("inner frame exists");
    node.base_mut().locked = Some(true);

    let target = state
        .resolve_image_drop_target(&["shot-slot".to_string(), "icon-wrap".to_string()])
        .expect("locked node yields to its editable parent");
    assert_eq!(target, NodeId::new("shot-slot"));
}

#[test]
fn applied_drop_writes_a_cover_image_fill_and_selects_the_target() {
    let mut state = placeholder_state();
    let target = NodeId::new("shot-slot");
    assert!(state.apply_image_drop(&target, SRC, Some([1200.0, 800.0])));

    let node = find_node(state.active_children(), &target).expect("target still present");
    let summary = first_image_fill_summary(node).expect("image fill written");
    assert_eq!(summary.image_url.as_deref(), Some(SRC));
    assert_eq!(summary.mode, ImageFillMode::Fill);
    assert_eq!(summary.original_size, Some([1200.0, 800.0]));
    assert_eq!(state.selection.anchor, target);
}

/// One gesture must be one undo step — not "fill" plus a stray commit that
/// the user has to press Cmd+Z twice to unwind.
#[test]
fn one_drop_is_one_undo_step() {
    let mut state = placeholder_state();
    let before = history_depth(&state);
    let target = NodeId::new("shot-slot");
    assert!(state.apply_image_drop(&target, SRC, None));
    assert_eq!(history_depth(&state), before + 1);

    state.undo();
    let node = find_node(state.active_children(), &target).expect("target restored");
    assert!(
        first_image_fill_summary(node).is_none(),
        "undo must remove the dropped fill"
    );
}

#[test]
fn dropping_onto_an_image_node_replaces_its_source() {
    let mut state = state_from(
        r##"{
      "version":"1.0.0",
      "children":[{"type":"image","id":"photo","src":"assets/old.png",
                   "x":0,"y":0,"width":100,"height":100}]
    }"##,
    );
    let target = NodeId::new("photo");
    assert!(state.apply_image_drop(&target, SRC, None));

    let PenNode::Image(image) =
        find_node(state.active_children(), &target).expect("image still present")
    else {
        panic!("expected an image node");
    };
    assert_eq!(image.src.as_str(), SRC);
}

/// Text is the case the structural rule exists for: it HAS a fill list, but
/// that fill paints glyphs, so an image there would be invisible nonsense.
#[test]
fn text_and_missing_nodes_refuse_the_drop_without_touching_history() {
    let mut state = placeholder_state();
    let before = history_depth(&state);
    assert!(!state.apply_image_drop(&NodeId::new("hint"), SRC, None));
    assert!(!state.apply_image_drop(&NodeId::new("does-not-exist"), SRC, None));
    assert_eq!(history_depth(&state), before);
}

#[test]
fn a_drop_on_empty_canvas_inserts_the_image_at_the_drop_point() {
    let mut state = placeholder_state();
    let id = state
        .insert_image_node_at_doc_point_sized("Shot", SRC, 600, 400, (1000.0, 500.0))
        .expect("image node inserted");

    let PenNode::Image(image) = find_node(state.active_children(), &id).expect("inserted node")
    else {
        panic!("expected an image node");
    };
    // 600×400 capped to a 300 px long side → 300×200, centred on the point.
    assert_eq!(image.base.x, Some(1000.0 - 150.0));
    assert_eq!(image.base.y, Some(500.0 - 100.0));
    assert_eq!(numeric_size(image.width.as_ref()), 300.0);
    assert_eq!(numeric_size(image.height.as_ref()), 200.0);
}

fn numeric_size(value: Option<&jian_ops_schema::sizing::SizingBehavior>) -> f64 {
    match value {
        Some(jian_ops_schema::sizing::SizingBehavior::Number(n)) => *n,
        other => panic!("expected a numeric size, got {other:?}"),
    }
}

/// The drop writes plain canonical schema, so a saved document must load back
/// with the fill intact — no host-only shape the parser drops on the way in.
/// Guards the "it looked right until you reopened the file" failure.
#[test]
fn a_dropped_fill_survives_a_document_roundtrip() {
    let mut state = placeholder_state();
    let target = NodeId::new("shot-slot");
    assert!(state.apply_image_drop(&target, SRC, Some([1200.0, 800.0])));

    let json = serde_json::to_string(&state.doc).expect("serialize document");
    let reloaded = jian_ops_schema::load_str(&json)
        .expect("saved document reloads")
        .value;
    let reloaded = EditorState::from_document(reloaded);

    let node = find_node(reloaded.active_children(), &target).expect("target survived");
    let summary = first_image_fill_summary(node).expect("image fill survived");
    assert_eq!(summary.image_url.as_deref(), Some(SRC));
    assert_eq!(summary.mode, ImageFillMode::Fill);
    assert_eq!(summary.original_size, Some([1200.0, 800.0]));
}
