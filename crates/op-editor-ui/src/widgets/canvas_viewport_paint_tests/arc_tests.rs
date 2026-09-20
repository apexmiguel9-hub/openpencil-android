//! Arc / pie / donut tessellation tests for `canvas_viewport_paint.rs`.
//!
//! Split out of `canvas_viewport_paint_tests.rs` to keep every file
//! under the repository's 800-line cap.

use crate::widgets::canvas_viewport_paint::arc_polygon;
use crate::Rect;

#[test]
fn pie_polygon_starts_at_centre() {
    let poly = arc_polygon(Rect::xywh(0.0, 0.0, 100.0, 100.0), 0.0, 90.0, 0.0);
    assert_eq!(poly[0].x, 50.0);
    assert_eq!(poly[0].y, 50.0);
    assert!((poly[1].x - 100.0).abs() < 0.01);
    assert!((poly[1].y - 50.0).abs() < 0.01);
}

#[test]
fn donut_polygon_has_outer_and_inner_rings() {
    let poly = arc_polygon(Rect::xywh(0.0, 0.0, 100.0, 100.0), 0.0, 360.0, 0.5);
    assert_eq!(poly.len(), 2 * (90 + 1));
    let last = poly[poly.len() - 1];
    let dist = ((last.x - 50.0).powi(2) + (last.y - 50.0).powi(2)).sqrt();
    assert!((dist - 25.0).abs() < 0.5, "inner radius ~25, got {dist}");
}

#[test]
fn quarter_sweep_end_point_at_90_degrees() {
    let poly = arc_polygon(Rect::xywh(0.0, 0.0, 100.0, 100.0), 0.0, 90.0, 0.0);
    let last = poly[poly.len() - 1];
    assert!((last.x - 50.0).abs() < 0.01);
    assert!((last.y - 100.0).abs() < 0.01);
}
