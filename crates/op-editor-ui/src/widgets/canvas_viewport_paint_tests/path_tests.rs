//! Path flattening / world-point projection tests for `canvas_viewport_paint.rs`.
//!
//! Split out of `canvas_viewport_paint_tests.rs` to keep every file
//! under the repository's 800-line cap.

use crate::layout_scene::{NodeKind, SceneAnchor, SceneNode, ScenePointType};
use crate::widgets::canvas_viewport_paint::{flatten_path, world_path_points, WorldPathPoints};
use crate::{Point2D, Rect};
use jian_scene::path_geometry::{flatten_path_points, PathPoints};

fn anchor(x: f32, y: f32, hout: Option<Point2D>) -> SceneAnchor {
    SceneAnchor {
        pos: Point2D::new(x, y),
        handle_in: None,
        handle_out: hout,
        point_type: ScenePointType::Corner,
    }
}

#[test]
fn handle_free_path_falls_back_to_points() {
    let mut n = SceneNode::leaf("p", NodeKind::Path);
    n.points = vec![Point2D::new(0.0, 0.0), Point2D::new(10.0, 0.0)];
    n.path_anchors = vec![anchor(0.0, 0.0, None), anchor(10.0, 0.0, None)];
    assert_eq!(flatten_path(&n), n.points);
}

#[test]
fn handle_free_open_path_borrows_points_without_allocating() {
    let mut n = SceneNode::leaf("p", NodeKind::Path);
    n.points = vec![Point2D::new(0.0, 0.0), Point2D::new(10.0, 0.0)];
    n.path_anchors = vec![anchor(0.0, 0.0, None), anchor(10.0, 0.0, None)];

    let points = flatten_path_points(&n);

    assert!(matches!(points, PathPoints::Borrowed(_)));
    assert_eq!(points.as_slice(), n.points.as_slice());
}

#[test]
fn small_filled_path_world_points_use_stack_buffer() {
    let points = [
        Point2D::new(0.0, 0.0),
        Point2D::new(10.0, 0.0),
        Point2D::new(10.0, 10.0),
    ];

    let world = world_path_points(&points, Point2D::new(5.0, 7.0), 2.0);

    assert!(matches!(world, WorldPathPoints::Stack { .. }));
    assert_eq!(
        world.as_slice(),
        &[
            Point2D::new(5.0, 7.0),
            Point2D::new(25.0, 7.0),
            Point2D::new(25.0, 27.0),
        ]
    );
}

#[test]
fn curved_segment_tessellates_into_many_points() {
    let mut n = SceneNode::leaf("p", NodeKind::Path);
    n.points = vec![Point2D::new(0.0, 0.0), Point2D::new(100.0, 0.0)];
    n.path_anchors = vec![
        anchor(0.0, 0.0, Some(Point2D::new(0.0, 50.0))),
        anchor(100.0, 0.0, None),
    ];
    let poly = flatten_path(&n);
    assert_eq!(poly.len(), 17);
    assert_eq!(poly[0], Point2D::new(0.0, 0.0));
    assert_eq!(poly[poly.len() - 1], Point2D::new(100.0, 0.0));
    assert!(poly[8].y > 1.0, "curve bows toward the handle");
}

#[test]
fn bounds_kept_so_helper_is_pure() {
    let mut n = SceneNode::leaf("p", NodeKind::Path);
    n.bounds = Rect::xywh(1.0, 2.0, 3.0, 4.0);
    let _ = flatten_path(&n);
    assert_eq!(n.bounds, Rect::xywh(1.0, 2.0, 3.0, 4.0));
}
