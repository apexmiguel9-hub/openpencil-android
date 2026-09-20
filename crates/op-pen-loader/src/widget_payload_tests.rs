use crate::pen_document_to_payload;

fn assert_child_payload(src: &str, radius: f32, kind: &str) {
    let parsed = jian_ops_schema::load_str(src).expect("canonical load");
    let loaded = pen_document_to_payload(&parsed.value);
    let node = &loaded.payload.pages[0].children[0];

    assert_eq!(node.corner_radius, radius);
    let widget = node.widget.as_ref().expect("widget payload");
    assert_eq!(widget.kind, kind);
    assert!(widget.corner_radius_authored);
}

#[test]
fn text_input_payload_carries_corner_radius() {
    assert_child_payload(
        r##"{
          "version":"1.0.0",
          "pages":[{"id":"p","name":"P","children":[{
            "type":"text_input","id":"search","width":160,"height":36,
            "placeholder":"Search","cornerRadius":8,
            "fill":[{"type":"solid","color":"#F8FAFC"}],
            "stroke":{"fill":[{"type":"solid","color":"#CBD5E1"}],"thickness":1}
          }]}],
          "children":[]
        }"##,
        8.0,
        "text_input",
    );
}

#[test]
fn number_input_payload_carries_corner_radius() {
    assert_child_payload(
        r##"{
          "version":"1.0.0",
          "pages":[{"id":"p","name":"P","children":[{
            "type":"number_input","id":"amount","width":120,"height":36,
            "placeholder":"0","cornerRadius":6,
            "fill":[{"type":"solid","color":"#F8FAFC"}],
            "stroke":{"fill":[{"type":"solid","color":"#CBD5E1"}],"thickness":1}
          }]}],
          "children":[]
        }"##,
        6.0,
        "number_input",
    );
}

#[test]
fn widget_payload_distinguishes_absent_from_explicit_zero_radius() {
    let src = r##"{
      "version":"1.0.0",
      "pages":[{"id":"p","name":"P","children":[
        {"type":"switch","id":"absent","width":40,"height":20},
        {"type":"switch","id":"square","width":40,"height":20,"cornerRadius":0}
      ]}],
      "children":[]
    }"##;
    let parsed = jian_ops_schema::load_str(src).expect("canonical load");
    let loaded = pen_document_to_payload(&parsed.value);
    let children = &loaded.payload.pages[0].children;

    assert!(!children[0].widget.as_ref().unwrap().corner_radius_authored);
    assert!(children[1].widget.as_ref().unwrap().corner_radius_authored);
    assert_eq!(
        (children[0].corner_radius, children[1].corner_radius),
        (0.0, 0.0)
    );
}

#[test]
fn progress_payload_carries_indeterminate() {
    let src = r##"{
      "version":"1.0.0",
      "pages":[{"id":"p","name":"P","children":[
        {"type":"progress","id":"loading","width":160,"height":8,
         "value":80,"max":100,"indeterminate":true}
      ]}],
      "children":[]
    }"##;
    let parsed = jian_ops_schema::load_str(src).expect("canonical load");
    let loaded = pen_document_to_payload(&parsed.value);
    let widget = loaded.payload.pages[0].children[0]
        .widget
        .as_ref()
        .expect("progress widget payload");

    assert!(widget.indeterminate);
    assert_eq!(widget.value_num, Some(80.0));
}
