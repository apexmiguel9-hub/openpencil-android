// Tests for Viewer::rebuild_scene / scene().
const DOC: &str = r#"{"version":"1.0","pages":[{"id":"p1","name":"P","children":[{"type":"rectangle","id":"r1","x":0,"y":0,"width":10,"height":10}]}]}"#;

#[test]
fn rebuild_scene_yields_one_page() {
    let mut v = super::Viewer::placeholder();
    v.load(DOC).unwrap();
    v.rebuild_scene();
    let scene = v.scene().expect("scene built");
    assert_eq!(scene.pages.len(), 1);
}

#[test]
fn figma_metadata_selects_authored_geometry_scene_builder() {
    let mut v = super::Viewer::placeholder();
    v.load(
        r#"{
          "version":"1.0",
          "editorMeta":{"preserveAuthoredGeometry":true},
          "pages":[{"id":"p","name":"P","children":[{
            "type":"frame","id":"root","width":240,"height":80,
            "layout":"horizontal","children":[
              {"type":"rectangle","id":"overlay","x":96,"y":17,"width":30,"height":20}
            ]
          }]}]
        }"#,
    )
    .expect("parse");

    let overlay = v.scene().expect("scene built").pages[0]
        .find("overlay")
        .expect("overlay node");
    assert_eq!(
        (overlay.bounds.origin.x, overlay.bounds.origin.y),
        (96.0, 17.0)
    );
}
