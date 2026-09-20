use super::widget_children_to_paint;
use crate::layout_scene::{
    LayoutScene, NodeKind, SceneNode, ScenePage, SceneWidget, SceneWidgetOption,
};
use crate::{Point2D, Rect};

fn tabs_node(value: Option<&str>) -> SceneNode {
    let mut node = SceneNode::leaf("tabs", NodeKind::Frame);
    node.widget = Some(SceneWidget {
        kind: "tabs".into(),
        value_str: value.map(str::to_owned),
        options: vec![
            SceneWidgetOption {
                value: "overview".into(),
                label: "Overview".into(),
            },
            SceneWidgetOption {
                value: "details".into(),
                label: "Details".into(),
            },
        ],
        ..Default::default()
    });
    node.children = vec![
        SceneNode::leaf("overview-panel", NodeKind::Frame),
        SceneNode::leaf("details-panel", NodeKind::Frame),
    ];
    node
}

#[test]
fn tabs_paint_only_the_authored_active_panel() {
    let node = tabs_node(Some("details"));
    let visible = widget_children_to_paint(&node);
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].id, "details-panel");
}

#[test]
fn tabs_live_value_switches_panel_and_invalid_value_falls_back_first() {
    let mut node = tabs_node(Some("missing"));
    assert_eq!(widget_children_to_paint(&node)[0].id, "overview-panel");

    node.widget.as_mut().unwrap().value_str = Some("details".into());
    assert_eq!(widget_children_to_paint(&node)[0].id, "details-panel");
}

fn bounded(id: &str, kind: NodeKind, bounds: Rect) -> SceneNode {
    let mut node = SceneNode::leaf(id, kind);
    node.bounds = bounds;
    node
}

/// The laid-out shape jian produces for a tabs frame: both panels occupy the
/// same single grid cell below the bar, each holding its own content.
fn laid_out_tabs_scene(value: Option<&str>) -> LayoutScene {
    let panel_rect = Rect::xywh(0.0, 40.0, 200.0, 160.0);
    let mut node = tabs_node(value);
    node.bounds = Rect::xywh(0.0, 0.0, 200.0, 200.0);
    node.children = vec![
        bounded("overview-panel", NodeKind::Frame, panel_rect),
        bounded("details-panel", NodeKind::Frame, panel_rect),
    ];
    node.children[0].children = vec![bounded(
        "overview-card",
        NodeKind::Rect,
        Rect::xywh(16.0, 56.0, 168.0, 64.0),
    )];
    node.children[1].children = vec![bounded(
        "details-card",
        NodeKind::Rect,
        Rect::xywh(16.0, 56.0, 168.0, 64.0),
    )];
    LayoutScene {
        pages: vec![ScenePage {
            id: "p".into(),
            name: "P".into(),
            children: vec![node],
        }],
        active_page_index: 0,
    }
}

/// Ids the painter would draw, walking with the painter's own child rule.
fn painted_ids(node: &SceneNode, out: &mut Vec<String>) {
    out.push(node.id.clone());
    for child in widget_children_to_paint(node) {
        painted_ids(child, out);
    }
}

/// The invariant behind the tabs fix: paint and hit-test must agree on which
/// nodes exist. Anything `node_path_at_doc_point` returns has to be a node the
/// painter drew — otherwise a click selects an invisible panel. This is a
/// reconciliation test, not a single-point regression: it probes the whole
/// tabs rect for every active value, including the fall-back ones.
#[test]
fn painted_subtree_and_canvas_hit_test_agree_for_every_active_tab() {
    for value in [None, Some("nope"), Some("overview"), Some("details")] {
        let scene = laid_out_tabs_scene(value);
        let root = &scene.pages[0].children[0];
        let mut painted = Vec::new();
        painted_ids(root, &mut painted);

        let mut hit_count = 0;
        for step_x in 0..20 {
            for step_y in 0..20 {
                let point = Point2D::new(5.0 + step_x as f32 * 10.0, 5.0 + step_y as f32 * 10.0);
                let Some(path) = scene.node_path_at_doc_point(point, 1.0) else {
                    continue;
                };
                hit_count += 1;
                for id in &path {
                    assert!(
                        painted.contains(id),
                        "hit-test reached {id:?} at {point:?} but the painter \
                         never drew it (active value {value:?}; painted \
                         {painted:?})"
                    );
                }
            }
        }
        assert!(
            hit_count > 0,
            "probe grid must actually hit the tabs frame, else the invariant \
             above is vacuous"
        );
    }
}
