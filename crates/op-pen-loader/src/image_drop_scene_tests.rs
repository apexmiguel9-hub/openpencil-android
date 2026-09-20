//! An image dropped onto a frame must reach the PAINT scene, not just the
//! document. `op-editor-core` proves the fill is written and survives a
//! save/load; this proves the loader turns that same fill into an image layer
//! the canvas actually draws, in cover (`Fill`) mode.

use jian_scene::layout_scene::{SceneFillLayer, SceneImageFit, SceneNode};
use op_editor_core::{EditorState, NodeId};

use crate::editor_state_to_layout_scene;

const SRC: &str = "data:image/png;base64,AAAA";

fn find<'a>(node: &'a SceneNode, id: &str) -> Option<&'a SceneNode> {
    if node.id == id {
        return Some(node);
    }
    node.children.iter().find_map(|child| find(child, id))
}

#[test]
fn a_dropped_fill_reaches_the_scene_as_a_cover_image_layer() {
    let doc = jian_ops_schema::load_str(
        r##"{
      "version":"1.0.0",
      "children":[{
        "type":"frame","id":"shot-slot","name":"Screenshot slot",
        "x":0,"y":0,"width":400,"height":300,
        "children":[
          {"type":"text","id":"hint","content":"Drop a screenshot","x":20,"y":20,
           "width":200,"fontSize":14}
        ]
      }]
    }"##,
    )
    .expect("fixture parses")
    .value;
    let mut state = EditorState::from_document(doc);
    let target = NodeId::new("shot-slot");
    assert!(state.apply_image_drop(&target, SRC, Some([1200.0, 800.0])));

    // Save + reload before building the scene: this is the path a user hits
    // after closing and reopening the document.
    let json = serde_json::to_string(&state.doc).expect("serialize document");
    let reloaded = crate::payload::load_canonical(&json).expect("op-pen-loader reads it back");
    let state = EditorState::from_document(reloaded.value);

    let scene = editor_state_to_layout_scene(&state);
    let page = scene.active_page().expect("active page");
    let node = page
        .children
        .iter()
        .find_map(|child| find(child, "shot-slot"))
        .expect("target frame in the scene");

    let layer = node
        .fill_layers
        .first()
        .expect("dropped fill became a scene layer");
    let SceneFillLayer::Image { src, fit, .. } = layer else {
        panic!("expected an image fill layer, got {layer:?}");
    };
    assert_eq!(&**src, SRC);
    assert_eq!(*fit, SceneImageFit::Fill);
    assert!(
        !node.children.is_empty(),
        "the frame keeps its children — the image fills BEHIND them"
    );
}
