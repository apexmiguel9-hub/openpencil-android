//! Per-shape section tests — the Polygon side count and the Ellipse
//! arc inputs that only paint for their own node kind.
//!
//! Split out of `property_panel_tests.rs` to keep both files under
//! the openpencil 800-line cap.

use crate::widgets::property_panel::PropertyPanel;
use crate::widgets::property_panel_sections as sections;
use crate::widgets::property_panel_test_support::{state_from, visible_for};
use crate::{Point2D, Rect};
use op_editor_core::NodeId;

#[test]
fn polygon_selection_exposes_sides_layer_input() {
    let mut state = state_from(
        r##"{ "version": "1.0.0", "children": [
              {"type":"polygon","id":"poly","name":"Hex",
               "x":40,"y":40,"width":120,"height":120,
               "polygonCount":6}
        ]}"##,
    );
    state.set_single_selection(NodeId::new("poly"));
    let panel = PropertyPanel::for_selection(&state).expect("polygon panel");

    assert_eq!(panel.snapshot.polygon_sides, Some(6));

    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(280.0, 1200.0),
    };
    let sides_rect = sections::editable_input_rects(
        rect,
        visible_for(&panel),
        &panel.snapshot.fills,
        &panel.snapshot.effects,
    )
    .into_iter()
    .find(|(focus, _)| *focus == op_editor_core::PropertyFocus::PolygonSides)
    .map(|(_, r)| r)
    .expect("polygon side input rect");
    let center = Point2D::new(
        sides_rect.origin.x + sides_rect.size.x / 2.0,
        sides_rect.origin.y + sides_rect.size.y / 2.0,
    );
    assert_eq!(
        panel.hit_test(rect, center),
        Some(op_editor_core::PropertyFocus::PolygonSides)
    );
}

#[test]
fn ellipse_selection_exposes_arc_layer_inputs() {
    let mut state = state_from(
        r##"{ "version": "1.0.0", "children": [
              {"type":"ellipse","id":"ell","name":"Arc",
               "x":40,"y":40,"width":120,"height":100,
               "startAngle":30,"sweepAngle":270,"innerRadius":0.25}
        ]}"##,
    );
    state.set_single_selection(NodeId::new("ell"));
    let panel = PropertyPanel::for_selection(&state).expect("ellipse panel");

    let arc = panel.snapshot.ellipse_arc.expect("ellipse arc snapshot");
    assert_eq!(arc.start_deg, 30.0);
    assert_eq!(arc.sweep_deg, 270.0);
    assert_eq!(arc.inner_percent, 25.0);

    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(280.0, 1200.0),
    };
    let rects = sections::editable_input_rects(
        rect,
        visible_for(&panel),
        &panel.snapshot.fills,
        &panel.snapshot.effects,
    );
    for focus in [
        op_editor_core::PropertyFocus::EllipseStart,
        op_editor_core::PropertyFocus::EllipseSweep,
        op_editor_core::PropertyFocus::EllipseInnerRadius,
    ] {
        let target = rects
            .iter()
            .find(|(f, _)| *f == focus)
            .map(|(_, r)| *r)
            .expect("ellipse arc input rect");
        let center = Point2D::new(
            target.origin.x + target.size.x / 2.0,
            target.origin.y + target.size.y / 2.0,
        );
        assert_eq!(panel.hit_test(rect, center), Some(focus));
    }
}
