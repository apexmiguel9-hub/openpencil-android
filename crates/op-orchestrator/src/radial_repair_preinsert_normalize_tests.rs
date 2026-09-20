use super::*;
use serde_json::json;

fn annotated_sun_arc() -> Value {
    json!({
        "type": "frame",
        "id": "sun-arc",
        "name": "Sunrise & Sunset Arc",
        "width": "fill_container",
        "height": "fit_content",
        "layout": "none",
        "children": [
            {"type":"ellipse","id":"track","name":"Sun Arc Track",
             "x":12,"y":18,"width":280,"height":112,"innerRadius":0.82,
             "startAngle":180,"sweepAngle":180},
            {"type":"ellipse","id":"progress","name":"Sun Arc Progress",
             "x":12,"y":18,"width":280,"height":112,"innerRadius":0.82,
             "startAngle":180,"sweepAngle":132},
            {"type":"text","id":"sunrise","name":"Sunrise Label",
             "x":0,"y":126,"width":72,"height":18,"content":"6:18 AM"},
            {"type":"icon_font","id":"sun","name":"Sun Marker",
             "x":142,"y":8,"width":18,"height":18,"iconFontName":"sun"},
            {"type":"text","id":"sunset","name":"Sunset Label",
             "x":232,"y":126,"width":72,"height":18,"content":"8:42 PM"}
        ]
    })
}

#[test]
fn wraps_only_the_arc_pair_and_preserves_sunrise_sunset_annotations() {
    let mut node = annotated_sun_arc();
    assert!(normalize_extended_radial_stack(&mut node));

    assert_eq!(node["width"], json!("fill_container"));
    assert_eq!(node["height"], json!("fit_content"));
    let kids = node["children"].as_array().expect("children");
    assert_eq!(kids.len(), 4);
    let stack = kids
        .iter()
        .find(|child| child["name"] == "Radial Stack")
        .expect("stack");
    assert_eq!(stack["layout"], json!("none"));
    assert_eq!(stack["x"], json!(12.0));
    assert_eq!(stack["y"], json!(18.0));
    assert_eq!(stack["width"], json!(280.0));
    assert_eq!(stack["height"], json!(112.0));
    let order: Vec<&str> = stack["children"]
        .as_array()
        .expect("stack children")
        .iter()
        .filter_map(|child| child["id"].as_str())
        .collect();
    assert_eq!(order, ["progress", "track"]);

    for id in ["sunrise", "sun", "sunset"] {
        let child = kids
            .iter()
            .find(|child| child["id"] == id)
            .unwrap_or_else(|| panic!("{id} preserved"));
        assert!(child.get("x").is_some(), "{id} position preserved");
    }
    let outer_order: Vec<&str> = kids
        .iter()
        .filter_map(|child| child["id"].as_str())
        .collect();
    assert_eq!(
        outer_order,
        ["sunrise", "sun", "sunset", "sun-arc__radial-stack"],
        "positioned annotations must paint in front of the arc stack"
    );
}

#[test]
fn infers_fluid_axes_for_a_single_centre_progress_ring() {
    let mut node = json!({
        "type":"frame","id":"ring","name":"Progress Ring",
        "width":"fill_container","height":"fit_content","layout":"horizontal",
        "children":[
            {"type":"ellipse","id":"track","name":"Ring Track",
             "width":80,"height":80,"innerRadius":0.8},
            {"type":"ellipse","id":"progress","name":"Ring Progress",
             "width":80,"height":80,"innerRadius":0.8,"startAngle":-90,"sweepAngle":240},
            {"type":"frame","id":"centre","name":"Ring Center","width":44,"height":24}
        ]
    });
    assert!(normalize_extended_radial_stack(&mut node));
    assert_eq!(node["width"], json!(80.0));
    assert_eq!(node["height"], json!(80.0));
}

#[test]
fn groups_multiple_direct_centre_labels_into_one_measurable_layer() {
    let mut node = json!({
        "type":"frame","id":"ring","name":"Steps Ring",
        "width":"fill_container","height":"fit_content","layout":"horizontal",
        "children":[
            {"type":"ellipse","id":"track","name":"Ring Track",
             "width":120,"height":120,"innerRadius":0.8},
            {"type":"ellipse","id":"progress","name":"Ring Progress",
             "width":120,"height":120,"innerRadius":0.8,"startAngle":-90,"sweepAngle":240},
            {"type":"text","id":"value","width":64,"height":24,"content":"8,432"},
            {"type":"text","id":"unit","width":40,"height":18,"content":"steps"}
        ]
    });

    assert!(normalize_extended_radial_stack(&mut node));
    assert_eq!(node["width"], json!(120.0));
    assert_eq!(node["height"], json!(120.0));
    let kids = node["children"].as_array().expect("children");
    assert_eq!(kids.len(), 3);
    assert_eq!(kids[0]["name"], json!("Radial Centre"));
    assert_eq!(kids[0]["layout"], json!("vertical"));
    let centre_ids: Vec<&str> = kids[0]["children"]
        .as_array()
        .expect("centre children")
        .iter()
        .filter_map(|child| child["id"].as_str())
        .collect();
    assert_eq!(centre_ids, ["value", "unit"]);
}

#[test]
fn positioned_centre_labels_in_a_flow_ring_are_not_reinterpreted() {
    let mut node = json!({
        "type":"frame","id":"ring","name":"Progress Ring",
        "width":"fill_container","height":"fit_content","layout":"horizontal",
        "children":[
            {"type":"ellipse","id":"track","name":"Ring Track",
             "width":120,"height":120,"innerRadius":0.8},
            {"type":"ellipse","id":"progress","name":"Ring Progress",
             "width":120,"height":120,"innerRadius":0.8,
             "startAngle":-90,"sweepAngle":240},
            {"type":"text","id":"value","x":28,"y":38,
             "width":64,"height":24,"content":"8,432"},
            {"type":"text","id":"unit","x":40,"y":66,
             "width":40,"height":18,"content":"steps"}
        ]
    });
    let before = node.clone();

    assert!(!normalize_extended_radial_stack(&mut node));
    assert_eq!(node, before);
}

#[test]
fn separated_arc_centres_are_not_collapsed_into_one_ring() {
    let mut node = json!({
        "type":"frame","id":"gauges","name":"Independent Gauges",
        "width":"fill_container","height":"fit_content","layout":"horizontal",
        "children":[
            {"type":"ellipse","id":"left","name":"Gauge Progress",
             "x":0,"y":0,"width":44,"height":44,"innerRadius":0.8,
             "startAngle":-90,"sweepAngle":180},
            {"type":"ellipse","id":"right","name":"Gauge Track",
             "x":72,"y":0,"width":44,"height":44,"innerRadius":0.8},
            {"type":"text","id":"label","width":44,"height":16,"content":"UV / AQI"}
        ]
    });
    let before = node.clone();

    assert!(!normalize_extended_radial_stack(&mut node));
    assert_eq!(node, before);
}

#[test]
fn does_not_merge_independent_dual_gauges() {
    let mut node = json!({
        "type":"frame","id":"gauges","name":"Independent Gauges",
        "width":"fill_container","height":"fit_content","layout":"horizontal","children":[
            {"type":"ellipse","id":"left","name":"Gauge Progress",
             "x":0,"y":0,"width":44,"height":44,"innerRadius":0.8,
             "startAngle":-90,"sweepAngle":180},
            {"type":"ellipse","id":"right","name":"Gauge Track",
             "x":72,"y":0,"width":44,"height":44,"innerRadius":0.8},
            {"type":"text","id":"left-label","x":0,"y":48,"width":44,"height":16,"content":"UV"},
            {"type":"text","id":"right-label","x":72,"y":48,"width":44,"height":16,"content":"AQI"}
        ]
    });
    let before = node.clone();
    assert!(!normalize_extended_radial_stack(&mut node));
    assert_eq!(node, before);
}

#[test]
fn fixed_wrapper_with_oversized_centre_stays_unfixed() {
    let mut node = json!({
        "type":"frame","id":"ring","name":"Progress Ring",
        "width":56,"height":56,"layout":"horizontal","children":[
            {"type":"ellipse","id":"track","name":"Ring Track",
             "width":56,"height":56,"innerRadius":0.8},
            {"type":"ellipse","id":"progress","name":"Ring Progress",
             "width":56,"height":56,"innerRadius":0.8,"startAngle":-90,"sweepAngle":240},
            {"type":"frame","id":"centre","name":"Ring Center","width":160,"height":24}
        ]
    });
    let before = node.clone();
    assert!(!normalize_extended_radial_stack(&mut node));
    assert_eq!(node, before);
}
