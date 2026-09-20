use std::collections::BTreeMap;

use crate::{pen_document_to_layout_scene, pen_document_to_layout_scene_with_geometry_mode};

fn document() -> jian_ops_schema::PenDocument {
    jian_ops_schema::load_str(
        r#"{
          "version":"1.0.0",
          "pages":[{"id":"p","name":"P","children":[{
            "type":"frame","id":"root","width":240,"height":80,
            "layout":"horizontal","children":[
              {"type":"rectangle","id":"a","x":72,"y":11,"width":30,"height":20},
              {"type":"rectangle","id":"b","x":144,"y":13,"width":30,"height":20}
            ]
          }]}]
        }"#,
    )
    .expect("fixture parses")
    .value
}

#[test]
fn geometry_mode_preserves_figma_parent_local_coordinates() {
    let doc = document();
    let scene = pen_document_to_layout_scene_with_geometry_mode(&doc, &BTreeMap::new(), 0, true);
    let root = &scene.pages[0].children[0];

    assert_eq!(
        (
            root.children[0].bounds.origin.x,
            root.children[0].bounds.origin.y
        ),
        (72.0, 11.0)
    );
    assert_eq!(
        (
            root.children[1].bounds.origin.x,
            root.children[1].bounds.origin.y
        ),
        (144.0, 13.0)
    );
}

#[test]
fn legacy_entry_point_keeps_layout_geometry_mode() {
    let doc = document();
    let active_theme = BTreeMap::new();
    let legacy = pen_document_to_layout_scene(&doc, &active_theme, 0);
    let explicit = pen_document_to_layout_scene_with_geometry_mode(&doc, &active_theme, 0, false);

    assert_eq!(legacy, explicit);
    assert_ne!(
        legacy.pages[0].children[0].children[0].bounds.origin.x,
        72.0
    );
}
