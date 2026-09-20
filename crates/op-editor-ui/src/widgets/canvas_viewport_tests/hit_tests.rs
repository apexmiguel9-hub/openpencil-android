//! Input hit-test tests — selection handles, rotation annulus and arc-handle placement.
//!
//! Split out of `canvas_viewport_tests.rs` to keep every file under
//! the repository's 800-line cap. Shared fixtures (`RecordingBackend`,
//! scene builders, transform-replay helpers) stay in that spine.

use super::*;

/// `selection_handle_at_point` + `rotation_corner_at_point` are
/// gated to single-select — multi-select paints an outline-only
/// overlay, so the hit-test must return `None` to match.
#[test]
fn selection_overlay_hit_tests_gate_to_single_select() {
    let scene = sample_scene();
    let canvas_rect = Rect::xywh(0.0, 0.0, 800.0, 600.0);
    // Frame "n1" bounds = (40, 40, 320, 200); at zoom 1, pan 0 the
    // top-left handle sits at the canvas-rect-relative origin.
    let handle_point = Point2D::new(40.0, 40.0);

    // Multi-select → both hit-tests return None.
    let mut multi = sample_state();
    multi.selection.set = vec![
        op_editor_core::NodeId::new("n1"),
        op_editor_core::NodeId::new("n2"),
    ];
    multi.selection.anchor = op_editor_core::NodeId::new("n1");
    assert!(
        selection_handle_at_point(canvas_rect, &scene, &multi, handle_point).is_none(),
        "multi-select must not expose handle hit-tests"
    );
    assert!(
        rotation_corner_at_point(canvas_rect, &scene, &multi, handle_point).is_none(),
        "multi-select must not expose rotation hit-tests"
    );

    // Single-select → the top-left handle is interactive again.
    let mut single = sample_state();
    single.set_single_selection(op_editor_core::NodeId::new("n1"));
    assert_eq!(
        selection_handle_at_point(canvas_rect, &scene, &single, handle_point),
        Some(SelectionHandle::TopLeft),
    );
}

/// The rotation ring is the annulus just OUTSIDE each corner —
/// a point beyond the 6 px handle slop but within 16 px hits it.
#[test]
fn rotation_corner_hit_tests_the_outer_annulus() {
    let scene = sample_scene();
    let canvas_rect = Rect::xywh(0.0, 0.0, 800.0, 600.0);
    let mut single = sample_state();
    single.set_single_selection(op_editor_core::NodeId::new("n1"));
    // 10 px diagonally outside the top-left corner (40, 40).
    let rot_point = Point2D::new(40.0 - 7.0, 40.0 - 7.0);
    assert_eq!(
        rotation_corner_at_point(canvas_rect, &scene, &single, rot_point),
        Some(SelectionHandle::TopLeft),
    );
}

#[test]
fn arc_handle_positions_places_three_handles() {
    use crate::widgets::canvas_viewport::{arc_handle_positions, ArcHandle};
    // 100×100 ellipse at origin → centre (50, 50), radii 50.
    let mut node = SceneNode::leaf("e1", NodeKind::Ellipse);
    node.bounds = Rect::xywh(0.0, 0.0, 100.0, 100.0);
    node.arc_start_angle = Some(0.0);
    node.arc_sweep_angle = Some(90.0);
    node.arc_inner_radius = Some(0.5);
    let handles = arc_handle_positions(&node).expect("ellipse yields handles");
    // Start handle at 0° → +X perimeter (100, 50).
    assert_eq!(handles[0].0, ArcHandle::Start);
    assert!((handles[0].1.x - 100.0).abs() < 0.01);
    assert!((handles[0].1.y - 50.0).abs() < 0.01);
    // Sweep handle at 90° → +Y perimeter (50, 100).
    assert_eq!(handles[1].0, ArcHandle::Sweep);
    assert!((handles[1].1.x - 50.0).abs() < 0.01);
    assert!((handles[1].1.y - 100.0).abs() < 0.01);
    // Inner handle at start angle, half radius → (75, 50).
    assert_eq!(handles[2].0, ArcHandle::Inner);
    assert!((handles[2].1.x - 75.0).abs() < 0.01);
}

#[test]
fn arc_handle_positions_none_for_non_ellipse() {
    let mut node = SceneNode::leaf("r1", NodeKind::Rect);
    node.bounds = Rect::xywh(0.0, 0.0, 100.0, 100.0);
    assert!(crate::widgets::canvas_viewport::arc_handle_positions(&node).is_none());
}
