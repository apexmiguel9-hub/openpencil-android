//! Nav-surface recolor / injection plus authored-geometry preservation,
//! overbold text repair and nested appended roots.

use super::*;

#[test]
fn cleanup_injects_missing_bottom_nav_surface_on_light_mobile_root() {
    let mut sink = VecDocSink::new();
    let tree: PenNode = serde_json::from_value(json!({
        "type": "frame",
        "id": "root",
        "name": "Brooklyn Food Delivery",
        "x": 80,
        "y": 40,
        "width": 390,
        "height": 844,
        "layout": "vertical",
        "fill": [{ "type": "solid", "color": "#FFF8F0" }],
        "children": [
            {
                "type": "frame",
                "id": "content",
                "name": "Content",
                "width": "fill_container",
                "height": 760,
                "children": []
            },
            {
                "type": "frame",
                "id": "bottom-nav",
                "name": "Bottom Navigation",
                "role": "bottom-tab-bar",
                "width": "fill_container",
                "height": 84,
                "children": []
            }
        ]
    }))
    .expect("mobile root json");
    sink.state.apply(EditorCommand::InsertSubtree {
        nodes: vec![tree],
        parent_id: NodeId::NONE,
        page_id: None,
    });
    let root_id = sink.state.active_children()[0].id_str().to_string();
    sink.applied.clear();

    run_cleanup_passes(&mut sink, &plan(), &[&root_id]);

    assert!(
        sink.applied
            .iter()
            .any(|c| matches!(c, EditorCommand::SetNodeFillHex { hex, .. } if hex == "#FFF8F0")),
        "cleanup should inject the root light surface on missing-fill bottom navs"
    );
    assert!(
        sink.applied
            .iter()
            .all(|c| !matches!(c, EditorCommand::AddNodeEffect { .. })),
        "bottom nav cleanup should not add a shadow band"
    );
}

#[test]
fn cleanup_leaves_top_navbar_transparent_on_light_mobile_root() {
    // The top header is transparent on mobile (TS references). A previous
    // version of `is_nav_surface` matched `role:"navbar"`, so this pass re-filled
    // the header with the root surface hex + a downward shadow — the "mysterious
    // background + rounded border" the user flagged. The bottom nav still gets a
    // surface; the top header must be left untouched.
    let mut sink = VecDocSink::new();
    let tree: PenNode = serde_json::from_value(json!({
        "type": "frame",
        "id": "root",
        "name": "Brooklyn Food Delivery",
        "x": 80, "y": 40, "width": 390, "height": 844,
        "layout": "vertical",
        "fill": [{ "type": "solid", "color": "#FFF8F0" }],
        "children": [
            {
                "type": "frame", "id": "header", "name": "Header", "role": "navbar",
                "width": "fill_container", "height": 56, "children": []
            },
            {
                "type": "frame", "id": "content", "name": "Content",
                "width": "fill_container", "height": 704, "children": []
            },
            {
                "type": "frame", "id": "bottom-nav", "name": "Bottom Navigation",
                "role": "bottom-tab-bar", "width": "fill_container", "height": 84, "children": []
            }
        ]
    }))
    .expect("mobile root json");
    sink.state.apply(EditorCommand::InsertSubtree {
        nodes: vec![tree],
        parent_id: NodeId::NONE,
        page_id: None,
    });
    let root_id = sink.state.active_children()[0].id_str().to_string();
    sink.applied.clear();

    run_cleanup_passes(&mut sink, &plan(), &[&root_id]);

    // TODO(reconcile w/ Kayshen e3ed2f1e "normalize mobile bottom tabs"): the
    // bottom-nav upward shadow (offsetY = -4) assertion is deferred — his cleanup
    // change to mobile bottom-tab handling supersedes our inject_nav_surface
    // shadow path. Re-enable once we align on whether the bottom nav keeps a
    // surface shadow under his normalization.
    // The top header is NOT repaired → no downward (offsetY = +4) header shadow.
    assert!(
        !sink.applied.iter().any(|c| matches!(
            c,
            EditorCommand::SetEffectParam { field: EffectField::OffsetY, value, .. }
                if (*value - 4.0).abs() < f32::EPSILON
        )),
        "top navbar must stay transparent — no surface shadow re-boxing it"
    );
}

#[test]
fn cleanup_recolors_white_bottom_nav_to_tinted_mobile_root_surface() {
    let mut sink = VecDocSink::new();
    let tree: PenNode = serde_json::from_value(json!({
        "type": "frame",
        "id": "root",
        "name": "Brooklyn Food Delivery",
        "x": 80,
        "y": 40,
        "width": 390,
        "height": 844,
        "layout": "vertical",
        "fill": [{ "type": "solid", "color": "#FFF8F0" }],
        "children": [
            {
                "type": "frame",
                "id": "content",
                "name": "Content",
                "width": "fill_container",
                "height": 760,
                "children": []
            },
            {
                "type": "frame",
                "id": "bottom-nav",
                "name": "Bottom Navigation",
                "role": "bottom-tab-bar",
                "width": "fill_container",
                "height": 84,
                "fill": [{ "type": "solid", "color": "#FFFFFF" }],
                "children": []
            }
        ]
    }))
    .expect("mobile root json");
    sink.state.apply(EditorCommand::InsertSubtree {
        nodes: vec![tree],
        parent_id: NodeId::NONE,
        page_id: None,
    });
    let root_id = sink.state.active_children()[0].id_str().to_string();
    sink.applied.clear();

    run_cleanup_passes(&mut sink, &plan(), &[&root_id]);

    assert!(
        sink.applied
            .iter()
            .any(|c| matches!(c, EditorCommand::SetNodeFillHex { hex, .. } if hex == "#FFF8F0")),
        "cream mobile roots should not keep a pure-white bottom nav band"
    );
}

#[test]
fn cleanup_preserves_authored_mobile_section_padding_and_width() {
    let mut sink = VecDocSink::new();
    let tree: PenNode = serde_json::from_value(json!({
        "type": "frame",
        "id": "root",
        "name": "Brooklyn Food Delivery",
        "x": 80,
        "y": 40,
        "width": 390,
        "height": 844,
        "layout": "vertical",
        "fill": [{ "type": "solid", "color": "#FFF8F0" }],
        "children": [
            {
                "type": "frame",
                "id": "popular-section",
                "name": "Popular Restaurants",
                "width": "fill_container",
                "height": "fit_content",
                "layout": "vertical",
                "children": [
                    {
                        "type": "frame",
                        "id": "restaurant-card",
                        "name": "Restaurant Card",
                        "width": 390,
                        "height": 120,
                        "children": []
                    }
                ]
            }
        ]
    }))
    .expect("mobile root json");
    sink.state.apply(EditorCommand::InsertSubtree {
        nodes: vec![tree],
        parent_id: NodeId::NONE,
        page_id: None,
    });
    let root_id = sink.state.active_children()[0].id_str().to_string();
    sink.applied.clear();

    run_cleanup_passes(&mut sink, &plan(), &[&root_id]);

    assert!(
        !sink.applied.iter().any(|command| matches!(
            command,
            EditorCommand::SetNodeLayoutProp { node_id, property, .. }
                if (node_id.as_str() == "popular-section" && property == "padding")
                    || (node_id.as_str() == "restaurant-card" && property == "width")
        )),
        "mobile padding and full-bleed width are design intent"
    );
}

#[test]
fn cleanup_preserves_authored_absolute_mobile_position() {
    let mut sink = VecDocSink::new();
    let tree: PenNode = serde_json::from_value(json!({
        "type": "frame",
        "id": "root",
        "name": "Brooklyn Food Delivery",
        "x": 80,
        "y": 40,
        "width": 390,
        "height": 844,
        "layout": "vertical",
        "fill": [{ "type": "solid", "color": "#FFF8F0" }],
        "children": [
            {
                "type": "frame",
                "id": "promo-section",
                "name": "Promo Banner",
                "width": "fill_container",
                "height": 140,
                "layout": "none",
                "padding": [0, 24, 0, 24],
                "children": [
                    {
                        "type": "frame",
                        "id": "promo-icon-tile",
                        "name": "Promo Icon Tile",
                        "x": 330,
                        "y": 40,
                        "width": 56,
                        "height": 56,
                        "fill": [{ "type": "solid", "color": "#FF6B00" }],
                        "children": []
                    }
                ]
            }
        ]
    }))
    .expect("mobile root json");
    sink.state.apply(EditorCommand::InsertSubtree {
        nodes: vec![tree],
        parent_id: NodeId::NONE,
        page_id: None,
    });
    let root_id = sink.state.active_children()[0].id_str().to_string();
    sink.applied.clear();

    run_cleanup_passes(&mut sink, &plan(), &[&root_id]);

    assert!(
        !sink.applied.iter().any(|command| matches!(
            command,
            EditorCommand::UpdateNode { node_id, x: Some(_), .. }
                if node_id.as_str() == "promo-icon-tile"
        )),
        "an absolute decoration is not repositioned from a guessed content inset"
    );
}

#[test]
fn cleanup_preserves_blank_gray_mobile_surface_color() {
    let mut sink = VecDocSink::new();
    let tree: PenNode = serde_json::from_value(json!({
        "type": "frame",
        "id": "root",
        "name": "Brooklyn Food Delivery",
        "x": 80,
        "y": 40,
        "width": 390,
        "height": 844,
        "layout": "vertical",
        "fill": [{ "type": "solid", "color": "#FFF8F0" }],
        "children": [
            {
                "type": "frame",
                "id": "restaurant-section",
                "name": "Restaurant Cards",
                "width": "fill_container",
                "height": 240,
                "padding": [0, 24, 0, 24],
                "children": [
                    {
                        "type": "rectangle",
                        "id": "tile",
                        "name": "Tile",
                        "width": 72,
                        "height": 72,
                        "fill": [{ "type": "solid", "color": "#E5E7EB" }]
                    }
                ]
            }
        ]
    }))
    .expect("mobile root json");
    sink.state.apply(EditorCommand::InsertSubtree {
        nodes: vec![tree],
        parent_id: NodeId::NONE,
        page_id: None,
    });
    let root_id = sink.state.active_children()[0].id_str().to_string();
    sink.applied.clear();

    run_cleanup_passes(&mut sink, &plan(), &[&root_id]);

    assert!(
        !sink.applied.iter().any(|command| matches!(
            command,
            EditorCommand::SetNodeFillHex { node_id, .. } if node_id.as_str() == "tile"
        )),
        "a neutral surface is not recolored from its dimensions"
    );
}

#[test]
fn cleanup_preserves_authored_mobile_tile_shapes() {
    let mut sink = VecDocSink::new();
    let tree: PenNode = serde_json::from_value(json!({
        "type": "frame",
        "id": "root",
        "name": "Brooklyn Food Delivery",
        "x": 80,
        "y": 40,
        "width": 390,
        "height": 844,
        "layout": "vertical",
        "fill": [{ "type": "solid", "color": "#FFF8F0" }],
        "children": [
            {
                "type": "frame",
                "id": "content-section",
                "name": "Content",
                "width": "fill_container",
                "height": 260,
                "padding": [0, 24, 0, 24],
                "children": [
                    {
                        "type": "frame",
                        "id": "filter-button",
                        "name": "Filter Button",
                        "width": 52,
                        "height": 72,
                        "fill": [{ "type": "solid", "color": "#FF6B00" }],
                        "children": [
                            {
                                "type": "icon_font",
                                "id": "filter-icon",
                                "name": "Sliders",
                                "iconFontName": "sliders-horizontal",
                                "width": 24,
                                "height": 24
                            }
                        ]
                    },
                    {
                        "type": "rectangle",
                        "id": "restaurant-media",
                        "name": "Tile",
                        "width": 82,
                        "height": 112,
                        "fill": [{ "type": "solid", "color": "#E5E7EB" }]
                    }
                ]
            }
        ]
    }))
    .expect("mobile root json");
    sink.state.apply(EditorCommand::InsertSubtree {
        nodes: vec![tree],
        parent_id: NodeId::NONE,
        page_id: None,
    });
    let root_id = sink.state.active_children()[0].id_str().to_string();
    sink.applied.clear();

    run_cleanup_passes(&mut sink, &plan(), &[&root_id]);

    assert!(
        !sink.applied.iter().any(|command| matches!(
            command,
            EditorCommand::UpdateNode { node_id, width: Some(_), height: Some(_), .. }
                if matches!(node_id.as_str(), "filter-button" | "restaurant-media")
        )),
        "generic mobile geometry does not imply square controls or media"
    );
}

#[test]
fn cleanup_preserves_dark_bottom_nav_on_dark_mobile_root() {
    let mut sink = VecDocSink::new();
    let tree: PenNode = serde_json::from_value(json!({
        "type": "frame",
        "id": "root",
        "name": "Dark Delivery App",
        "x": 80,
        "y": 40,
        "width": 390,
        "height": 844,
        "layout": "vertical",
        "fill": [{ "type": "solid", "color": "#111827" }],
        "children": [
            {
                "type": "frame",
                "id": "content",
                "name": "Content",
                "width": "fill_container",
                "height": 760,
                "children": []
            },
            {
                "type": "frame",
                "id": "bottom-nav",
                "name": "Bottom Navigation",
                "role": "bottom-tab-bar",
                "width": "fill_container",
                "height": 84,
                "fill": [{ "type": "solid", "color": "#0F0F0F" }],
                "children": []
            }
        ]
    }))
    .expect("mobile root json");
    sink.state.apply(EditorCommand::InsertSubtree {
        nodes: vec![tree],
        parent_id: NodeId::NONE,
        page_id: None,
    });
    let root_id = sink.state.active_children()[0].id_str().to_string();
    sink.applied.clear();

    run_cleanup_passes(&mut sink, &plan(), &[&root_id]);

    assert!(
        sink.applied
            .iter()
            .all(|c| !matches!(c, EditorCommand::SetNodeFillHex { .. })),
        "cleanup should not force a light nav surface when the root itself is dark"
    );
}

#[test]
fn run_cleanup_passes_repairs_overbold_text_hierarchy() {
    let mut sink = VecDocSink::new();
    let tree: PenNode = serde_json::from_value(json!({
        "type": "frame",
        "id": "root",
        "name": "Mobile Root",
        "width": 390,
        "height": 844,
        "children": [
            {
                "type": "text",
                "id": "title",
                "role": "heading",
                "content": "Popular Restaurants",
                "width": 320,
                "height": 40,
                "fontSize": 30,
                "fontWeight": 800
            },
            {
                "type": "text",
                "id": "subtitle",
                "role": "body-text",
                "content": "Fresh Brooklyn favorites, delivered fast.",
                "width": 320,
                "height": 22,
                "fontSize": 16,
                "fontWeight": 800
            },
            {
                "type": "text",
                "id": "placeholder",
                "name": "Placeholder",
                "content": "Search restaurants or dishes",
                "width": 280,
                "height": 24,
                "fontSize": 17,
                "fontWeight": 800
            },
            {
                "type": "text",
                "id": "metadata",
                "role": "caption",
                "content": "20-30 min",
                "width": 100,
                "height": 18,
                "fontSize": 14,
                "fontWeight": 800
            }
        ]
    }))
    .expect("mobile root json");
    sink.state.apply(EditorCommand::InsertSubtree {
        nodes: vec![tree],
        parent_id: NodeId::NONE,
        page_id: None,
    });
    let root_id = sink.state.active_children()[0].id_str().to_string();
    sink.applied.clear();

    run_cleanup_passes(&mut sink, &plan(), &[&root_id]);

    assert!(
        sink.applied.iter().any(|c| matches!(
            c,
            EditorCommand::SetNodeFontWeight {
                font_weight: 400,
                ..
            }
        )),
        "cleanup should downgrade non-heading text when the whole screen was emitted as bold"
    );
}

#[test]
fn cleanup_finds_nested_appended_root() {
    let mut sink = VecDocSink::new();
    // `target` is a pre-existing container frame.  `appended` is a new section
    // nested under it — matching how append-mode subagent.rs inserts roots
    // under an existing target frame rather than at top level.
    let tree = frame_json(
        "target",
        json!([
            frame_json_value("old-section", json!([])),
            frame_json_value(
                "appended",
                json!([
                    frame_json_value("status-bar-1", json!([])),
                    frame_json_value("hero", json!([])),
                    frame_json_value("status-bar-2", json!([])),
                ]),
            ),
        ]),
    );
    sink.state.apply(EditorCommand::InsertSubtree {
        nodes: vec![tree],
        parent_id: NodeId::NONE,
        page_id: None,
    });
    // Resolve the real (remapped) id of the nested `appended` frame by name.
    let appended_id = find_node_id_by_name(&sink.state, "appended");
    sink.applied.clear();
    run_cleanup_passes(&mut sink, &plan(), &[&appended_id]);
    // The dup-status-bar pass ran against the NESTED root → a DeleteNode fired.
    assert!(
        sink.applied
            .iter()
            .any(|c| matches!(c, EditorCommand::DeleteNode { .. })),
        "find_root must locate a nested appended root so cleanup passes fire"
    );
}
