use super::*;
use serde_json::json;

#[test]
fn layout_none_repair_centres_against_resolved_child_size() {
    let ring = json!({
        "type":"frame","id":"ring","width":"fill_container","height":120,"layout":"none",
        "children":[
            {"type":"frame","id":"center","width":98,"height":43},
            {"type":"ellipse","id":"progress","width":120,"height":120,
             "innerRadius":0.86,"sweepAngle":264},
            {"type":"ellipse","id":"track","width":120,"height":120,"innerRadius":0.86}
        ]
    });
    let rects = HashMap::from([
        ("ring".to_string(), Rect { w: 287.0, h: 120.0 }),
        ("center".to_string(), Rect { w: 98.0, h: 47.0 }),
    ]);

    let commands = radial_stack_repair(&ring, &rects).expect("radial repair");
    let center_update = commands.iter().find_map(|command| match command {
        EditorCommand::UpdateNode {
            node_id,
            x,
            y,
            width,
            height,
            ..
        } if node_id.as_str() == "center" => Some((*x, *y, *width, *height)),
        _ => None,
    });

    assert_eq!(
        center_update,
        Some((Some(95), Some(37), None, None)),
        "position must use the final 47px layout height without rewriting the authored size"
    );
}

#[test]
fn late_repair_reorders_unnamed_partial_pairs_to_canonical_painter_order() {
    for progress_sweep in [200, 185] {
        let ring_id = format!("ring-{progress_sweep}");
        let ring: jian_ops_schema::node::PenNode = serde_json::from_value(json!({
            "type":"frame","id":ring_id,"width":64,"height":64,"layout":"none",
            "children":[
                {"type":"ellipse","id":format!("large-{progress_sweep}"),
                 "x":0,"y":0,"width":64,"height":64,"innerRadius":0.72,
                 "startAngle":135,"sweepAngle":270},
                {"type":"ellipse","id":format!("small-{progress_sweep}"),
                 "x":0,"y":0,"width":64,"height":64,"innerRadius":0.72,
                 "startAngle":135,"sweepAngle":progress_sweep},
                {"type":"frame","id":format!("centre-{progress_sweep}"),
                 "x":0,"y":0,"width":64,"height":64,"layout":"horizontal"}
            ]
        }))
        .expect("valid unnamed partial ring");
        let mut state = EditorState::new();
        state.active_children_mut().clear();
        state.active_children_mut().push(ring);
        let mut sink = crate::loop_finalize::StateDocSink { state: &mut state };

        assert!(repair_radial_stacks(&mut sink, &ring_id));

        let repaired = serde_json::to_value(&sink.state.active_children()[0])
            .expect("serialize repaired ring");
        let order: Vec<&str> = repaired["children"]
            .as_array()
            .expect("ring children")
            .iter()
            .filter_map(|child| child.get("id").and_then(Value::as_str))
            .collect();
        assert_eq!(
            order,
            [
                format!("centre-{progress_sweep}"),
                format!("small-{progress_sweep}"),
                format!("large-{progress_sweep}"),
            ]
        );
    }
}
