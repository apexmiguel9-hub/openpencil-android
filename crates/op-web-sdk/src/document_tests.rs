// Tests for Viewer document parsing and page snapshot accessors.
const DOC: &str = r#"{"version":"1.0","pages":[{"id":"p1","name":"Page 1","children":[]}]}"#;

#[test]
fn load_parses_pages() {
    let mut v = super::Viewer::placeholder();
    v.load(DOC).expect("parse");
    assert_eq!(v.page_count(), 1);
    assert_eq!(v.active_page_index(), 0);
}

#[test]
fn load_restores_and_clamps_shared_editor_metadata() {
    let mut v = super::Viewer::placeholder();
    v.load(
        r#"{
          "version":"1.0",
          "editorMeta":{"activePageIndex":99,"preserveAuthoredGeometry":true},
          "pages":[
            {"id":"p1","name":"One","children":[]},
            {"id":"p2","name":"Two","children":[]}
          ]
        }"#,
    )
    .expect("parse");

    assert_eq!(v.active_page_index(), 1);
    assert!(v.preserve_authored_geometry);
    assert_eq!(v.scene().expect("scene").active_page_index, 1);
}

#[test]
fn legacy_document_opens_first_non_empty_page_with_layout_geometry() {
    let mut v = super::Viewer::placeholder();
    v.load(
        r#"{
          "version":"1.0",
          "pages":[
            {"id":"p1","name":"Empty","children":[]},
            {"id":"p2","name":"Content","children":[{"type":"rectangle","id":"r"}]}
          ]
        }"#,
    )
    .expect("parse");

    assert_eq!(v.active_page_index(), 1);
    assert!(!v.preserve_authored_geometry);
}
