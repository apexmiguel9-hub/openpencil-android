//! Flex layout resolution, unbounded-group bounds, multi-root canvas
//! offsets and multi-page scene structure.

use super::*;

#[test]
fn flex_layout_resolves_child_bounds_not_authored_coords() {
    // A vertical flex frame: a `fill_container`-width child must come
    // out stretched to the root's 375 px width — NOT the authored
    // `0` width the schema collapses flex tokens to. This proves the
    // scene carries layout-RESOLVED geometry.
    let src = r##"{
      "version":"1.0.0",
      "pages":[{
        "id":"p1","name":"Page 1",
        "children":[{
          "type":"frame","id":"root","width":375,"height":812,
          "layout":"vertical","gap":16,
          "children":[
            {"type":"rectangle","id":"r1","width":"fill_container","height":40,
             "fill":[{"type":"solid","color":"#000000"}]}
          ]
        }]
      }],
      "children":[]
    }"##;
    let scene = editor_state_to_layout_scene(&state_from(src));
    assert_eq!(scene.pages.len(), 1);
    let root = &scene.pages[0].children[0];
    assert_eq!(root.id, "root");
    let child = &root.children[0];
    assert_eq!(child.id, "r1");
    // Flex stretched the child to the root width.
    assert_eq!(
        child.bounds.size.x, 375.0,
        "fill_container stretched via taffy"
    );
    assert_eq!(child.bounds.size.y, 40.0);
}

#[test]
fn layout_scene_precomputes_unbounded_group_bounds() {
    let src = r##"{
      "version":"1.0.0",
      "pages":[{
        "id":"p1","name":"Page 1",
        "children":[{
          "type":"group","id":"g",
          "children":[
            {"type":"rectangle","id":"a","x":10,"y":20,"width":30,"height":40},
            {"type":"rectangle","id":"b","x":80,"y":5,"width":20,"height":25}
          ]
        }]
      }],
      "children":[]
    }"##;
    let scene = editor_state_to_layout_scene(&state_from(src));
    let group = &scene.pages[0].children[0];

    assert_eq!(group.bounds, op_editor_core::render_backend::Rect::ZERO);
    assert_eq!(
        group.aggregate_bounds_cache,
        op_editor_core::render_backend::Rect::xywh(10.0, 5.0, 90.0, 55.0),
        "loader should cache unbounded aggregate bounds once during scene build"
    );
}

#[test]
fn multi_root_designs_keep_authored_canvas_offset() {
    // Two side-by-side designs at distinct canvas coords — each
    // root's resolved bounds must reflect its authored `(x, y)`,
    // not collapse to origin.
    let src = r##"{
      "version":"1.0.0","pages":[{"id":"p","name":"P","children":[
        {"type":"frame","id":"a","x":100,"y":50,"width":200,"height":100,
         "fill":[{"type":"solid","color":"#FF0000"}]},
        {"type":"frame","id":"b","x":-500,"y":2000,"width":200,"height":100,
         "fill":[{"type":"solid","color":"#00FF00"}]}
      ]}],"children":[]
    }"##;
    let scene = editor_state_to_layout_scene(&state_from(src));
    let kids = &scene.pages[0].children;
    assert_eq!(
        (kids[0].bounds.origin.x, kids[0].bounds.origin.y),
        (100.0, 50.0)
    );
    assert_eq!(
        (kids[1].bounds.origin.x, kids[1].bounds.origin.y),
        (-500.0, 2000.0)
    );
}

#[test]
#[ignore = "opt-in local fixture (mutable Desktop file, v2.8 loaded best-effort): currently \
            trips a real main-axis overflow (tab section bottom 2456 > root bottom 2418) in \
            the jian growth path being reworked around jian 57068a6; run with --ignored"]
fn pencil_demo_monochrome_tab_bar_children_stay_inside_artboard() {
    let path = "/Users/kayshen/Desktop/pencil-demo.op";
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };
    let src = std::str::from_utf8(&bytes).unwrap();
    let parsed = crate::payload::load_canonical(src).expect("canonical load");
    let scene = editor_state_to_layout_scene(&EditorState::from_document(parsed.value));
    let page = scene.active_page().expect("active page");
    let root = page
        .find("xkt7Z")
        .expect("Habit Tracker - Monochrome Type root");
    let tab_section = root.find("rlIWP").expect("bottom tab section");
    let tab_bar = root.find("aJihn").expect("bottom tab bar");

    let root_bottom = root.bounds.origin.y + root.bounds.size.y;
    let tab_bar_bottom = tab_bar.bounds.origin.y + tab_bar.bounds.size.y;
    let visual_bottom = max_descendant_bottom(root);
    let root_children = root
        .children
        .iter()
        .map(|child| {
            format!(
                "{}:[{},{},{},{}]",
                child.id,
                child.bounds.origin.x,
                child.bounds.origin.y,
                child.bounds.size.x,
                child.bounds.size.y
            )
        })
        .collect::<Vec<_>>()
        .join(", ");

    assert!(
        visual_bottom <= root_bottom + 0.5,
        "monochrome artboard clips bottom tab content: root bottom={root_bottom}, \
         tab section={:?}, tab bar={:?}, tab bar bottom={tab_bar_bottom}, \
         visual bottom={visual_bottom}, root children={root_children}",
        tab_section.bounds,
        tab_bar.bounds
    );
}

#[test]
fn multi_page_document_produces_expected_page_structure() {
    let src = r##"{
      "version":"1.0.0","pages":[
        {"id":"home","name":"Home","children":[
          {"type":"rectangle","id":"h1","width":80,"height":40}
        ]},
        {"id":"about","name":"About","children":[
          {"type":"rectangle","id":"a1","width":60,"height":30},
          {"type":"rectangle","id":"a2","width":60,"height":30}
        ]}
      ],"children":[]
    }"##;
    let scene = editor_state_to_layout_scene(&state_from(src));
    assert_eq!(scene.pages.len(), 2);
    assert_eq!(scene.pages[0].id, "home");
    assert_eq!(scene.pages[0].name, "Home");
    assert_eq!(scene.pages[0].children.len(), 1);
    assert_eq!(scene.pages[1].id, "about");
    assert_eq!(scene.pages[1].name, "About");
    assert_eq!(scene.pages[1].children.len(), 2);
    // The builder always opens on page 0 (the loader resets the
    // active page index).
    assert_eq!(scene.active_page_index, 0);
    assert_eq!(scene.active_page().map(|p| p.id.as_str()), Some("home"));
}
