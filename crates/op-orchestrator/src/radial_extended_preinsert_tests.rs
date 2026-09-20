use jian_ops_schema::node::PenNode;
use serde_json::{json, Value};

use crate::orchestration_self_check::{
    auto_fix_fixable_issues, check_generated_nodes, SelfCheckReport,
};

fn has_radial_issue(report: &SelfCheckReport) -> bool {
    report
        .issues
        .iter()
        .any(|issue| issue.code == "radial-stack-not-concentric")
}

fn find_id<'a>(value: &'a Value, id: &str) -> Option<&'a Value> {
    if let Some(values) = value.as_array() {
        return values.iter().find_map(|value| find_id(value, id));
    }
    if value.get("id").and_then(Value::as_str) == Some(id) {
        return Some(value);
    }
    value
        .get("children")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find_map(|child| find_id(child, id))
}

#[test]
fn annotated_sun_arc_is_salvaged_before_insert_and_is_idempotent() {
    let mut nodes: Vec<PenNode> = serde_json::from_value(json!([{
        "type":"frame","id":"sun-arc","name":"Sunrise & Sunset Arc",
        "width":"fill_container","height":"fit_content","layout":"none",
        "children":[
            {"type":"ellipse","id":"track","name":"Sun Arc Track",
             "x":12,"y":18,"width":280,"height":112,"innerRadius":0.82,
             "startAngle":180,"sweepAngle":180,
             "fill":[{"type":"solid","color":"#334155"}]},
            {"type":"ellipse","id":"progress","name":"Sun Arc Progress",
             "x":12,"y":18,"width":280,"height":112,"innerRadius":0.82,
             "startAngle":180,"sweepAngle":132,
             "fill":[{"type":"solid","color":"#FACC15"}]},
            {"type":"text","id":"sunrise","name":"Sunrise Label",
             "x":0,"y":126,"width":72,"height":18,"content":"6:18 AM"},
            {"type":"icon_font","id":"sun","name":"Sun Marker",
             "x":142,"y":8,"width":18,"height":18,"iconFontName":"sun"},
            {"type":"text","id":"sunset","name":"Sunset Label",
             "x":232,"y":126,"width":72,"height":18,"content":"8:42 PM"}
        ]
    }]))
    .expect("valid sun arc");

    let before = check_generated_nodes(&nodes, 375.0);
    assert!(has_radial_issue(&before), "precondition: {before:?}");
    assert!(auto_fix_fixable_issues(&mut nodes, 375.0));
    let after = check_generated_nodes(&nodes, 375.0);
    assert!(!after.has_fatal(), "repaired sun arc: {after:?}");

    let repaired = serde_json::to_value(&nodes).expect("serialize");
    let outer = find_id(&repaired, "sun-arc").expect("outer");
    assert_eq!(outer["width"], json!("fill_container"));
    assert_eq!(outer["height"], json!("fit_content"));
    let stack = find_id(&repaired, "sun-arc__radial-stack").expect("stack");
    assert_eq!(stack["layout"], json!("none"));
    assert_eq!(stack["width"], json!(280.0));
    assert_eq!(stack["height"], json!(112.0));
    for id in ["sunrise", "sun", "sunset"] {
        assert!(find_id(outer, id).is_some(), "{id} must survive");
    }
    let outer_order: Vec<&str> = outer["children"]
        .as_array()
        .expect("outer children")
        .iter()
        .filter_map(|child| child["id"].as_str())
        .collect();
    assert_eq!(
        outer_order,
        ["sunrise", "sun", "sunset", "sun-arc__radial-stack"],
        "annotations must remain in front of the arc stack"
    );

    let once = serde_json::to_value(&nodes).expect("serialize once");
    assert!(!auto_fix_fixable_issues(&mut nodes, 375.0));
    assert_eq!(
        serde_json::to_value(&nodes).expect("serialize twice"),
        once,
        "second auto-fix must be a byte-structure no-op"
    );
}

#[test]
fn fluid_ring_with_direct_value_and_unit_is_grouped_and_centred() {
    let mut nodes: Vec<PenNode> = serde_json::from_value(json!([{
        "type":"frame","id":"ring","name":"Progress Ring",
        "width":"fill_container","height":"fit_content","layout":"horizontal",
        "children":[
            {"type":"ellipse","id":"track","name":"Ring Track",
             "width":120,"height":120,"innerRadius":0.8,
             "fill":[{"type":"solid","color":"#334155"}]},
            {"type":"ellipse","id":"progress","name":"Ring Progress",
             "width":120,"height":120,"innerRadius":0.8,
             "startAngle":-90,"sweepAngle":240,
             "fill":[{"type":"solid","color":"#22C55E"}]},
            {"type":"text","id":"value","width":64,"height":24,"content":"8,432"},
            {"type":"text","id":"unit","width":40,"height":18,"content":"steps"}
        ]
    }]))
    .expect("valid ring");

    assert!(has_radial_issue(&check_generated_nodes(&nodes, 375.0)));
    assert!(auto_fix_fixable_issues(&mut nodes, 375.0));
    let after = check_generated_nodes(&nodes, 375.0);
    assert!(!after.has_fatal(), "repaired ring: {after:?}");

    let repaired = serde_json::to_value(&nodes).expect("serialize");
    let ring = find_id(&repaired, "ring").expect("ring");
    assert_eq!(ring["layout"], json!("none"));
    assert_eq!(ring["width"], json!(120.0));
    assert_eq!(ring["height"], json!(120.0));
    let order: Vec<&str> = ring["children"]
        .as_array()
        .expect("children")
        .iter()
        .filter_map(|child| child["id"].as_str())
        .collect();
    assert_eq!(
        order,
        ["ring__radial-centre", "progress", "track"],
        "centre must paint above progress and track"
    );
}

#[test]
fn independent_gauges_and_oversized_centre_remain_fatal() {
    let cases = [
        json!({
            "type":"frame","id":"gauges","name":"Independent Gauges",
            "width":"fill_container","height":"fit_content","layout":"horizontal",
            "children":[
                {"type":"ellipse","id":"left","name":"Gauge Progress",
                 "x":0,"y":0,"width":44,"height":44,"innerRadius":0.8,
                 "startAngle":-90,"sweepAngle":180},
                {"type":"ellipse","id":"right","name":"Gauge Track",
                 "x":72,"y":0,"width":44,"height":44,"innerRadius":0.8},
                {"type":"text","id":"left-label","x":0,"y":48,
                 "width":44,"height":16,"content":"UV"},
                {"type":"text","id":"right-label","x":72,"y":48,
                 "width":44,"height":16,"content":"AQI"}
            ]
        }),
        json!({
            "type":"frame","id":"single-label-gauges","name":"Independent Gauges",
            "width":"fill_container","height":"fit_content","layout":"horizontal",
            "children":[
                {"type":"ellipse","id":"left","name":"Gauge Progress",
                 "x":0,"y":0,"width":44,"height":44,"innerRadius":0.8,
                 "startAngle":-90,"sweepAngle":180},
                {"type":"ellipse","id":"right","name":"Gauge Track",
                 "x":72,"y":0,"width":44,"height":44,"innerRadius":0.8},
                {"type":"text","id":"label","width":44,"height":16,"content":"UV / AQI"}
            ]
        }),
        json!({
            "type":"frame","id":"ring","name":"Progress Ring",
            "width":56,"height":56,"layout":"horizontal","children":[
                {"type":"ellipse","id":"track","name":"Ring Track",
                 "width":56,"height":56,"innerRadius":0.8},
                {"type":"ellipse","id":"progress","name":"Ring Progress",
                 "width":56,"height":56,"innerRadius":0.8,
                 "startAngle":-90,"sweepAngle":240},
                {"type":"frame","id":"centre","name":"Ring Center","width":160,"height":24}
            ]
        }),
    ];

    for (index, value) in cases.into_iter().enumerate() {
        let mut nodes: Vec<PenNode> =
            serde_json::from_value(json!([value])).expect("valid unsafe fixture");
        let before = serde_json::to_value(&nodes).expect("before");
        assert!(has_radial_issue(&check_generated_nodes(&nodes, 375.0)));
        assert!(
            !auto_fix_fixable_issues(&mut nodes, 375.0),
            "case {index} must not be guessed"
        );
        assert_eq!(serde_json::to_value(&nodes).expect("after"), before);
        assert!(has_radial_issue(&check_generated_nodes(&nodes, 375.0)));
    }
}
