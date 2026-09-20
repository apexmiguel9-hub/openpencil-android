//! Transform-replay tests — rotation / flip chains applied to hover, selection, path and arc overlays.
//!
//! Split out of `canvas_viewport_tests.rs` to keep every file under
//! the repository's 800-line cap. Shared fixtures (`RecordingBackend`,
//! scene builders, transform-replay helpers) stay in that spine.

use super::*;

#[test]
fn hover_outline_rotates_with_rotated_frame() {
    let _guard = crate::agent_indicator_test_support::lock();
    op_editor_core::agent_indicators::clear();
    let mut scene = sample_scene();
    let rotation = 0.5_f32;
    scene.pages[0].children[0].rotation = rotation;
    let state = EditorState::new();
    let mut viewport = CanvasViewport::from_editor(&state, &scene);
    viewport.hovered = Some("n1".into());
    let mut backend = RecordingBackend::default();
    let rect = Rect::xywh(0.0, 0.0, 800.0, 600.0);
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        viewport.paint(&mut cx, rect);
    }

    let rotations = active_rotations_at_first_stroke(&backend, HOVER_OUTLINE_COLOR)
        .expect("hover outline should paint dashed strokes");
    assert_eq!(
        rotations.len(),
        1,
        "hover outline should paint under the hovered frame's rotation"
    );
    // Frame n1 spans (40, 40, 320, 200) → doc-space center (200, 140).
    let vp = &viewport.viewport;
    let pivot = Point2D::new(
        rect.origin.x + vp.pan_x + 200.0 * vp.zoom,
        rect.origin.y + vp.pan_y + 140.0 * vp.zoom,
    );
    assert!((rotations[0].0 - rotation).abs() < 1e-4);
    assert!(
        (rotations[0].1.x - pivot.x).abs() < 0.5 && (rotations[0].1.y - pivot.y).abs() < 0.5,
        "hover outline must rotate about the frame's own center; got {:?}, want {:?}",
        rotations[0].1,
        pivot
    );
}

#[test]
fn hover_outline_on_child_applies_ancestor_rotation() {
    let _guard = crate::agent_indicator_test_support::lock();
    op_editor_core::agent_indicators::clear();
    let mut scene = sample_scene();
    let rotation = 0.5_f32;
    scene.pages[0].children[0].rotation = rotation;
    let state = EditorState::new();
    let mut viewport = CanvasViewport::from_editor(&state, &scene);
    // n2 is an unrotated child of the rotated frame n1.
    viewport.hovered = Some("n2".into());
    let mut backend = RecordingBackend::default();
    let rect = Rect::xywh(0.0, 0.0, 800.0, 600.0);
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        viewport.paint(&mut cx, rect);
    }

    let rotations = active_rotations_at_first_stroke(&backend, HOVER_OUTLINE_COLOR)
        .expect("hover outline should paint dashed strokes");
    assert_eq!(
        rotations.len(),
        1,
        "child hover outline should inherit the rotated ancestor's transform"
    );
    // The pivot is the rotated PARENT's center, not the child's.
    let vp = &viewport.viewport;
    let pivot = Point2D::new(
        rect.origin.x + vp.pan_x + 200.0 * vp.zoom,
        rect.origin.y + vp.pan_y + 140.0 * vp.zoom,
    );
    assert!((rotations[0].0 - rotation).abs() < 1e-4);
    assert!(
        (rotations[0].1.x - pivot.x).abs() < 0.5 && (rotations[0].1.y - pivot.y).abs() < 0.5,
        "child hover outline must rotate about the ancestor's pivot; got {:?}, want {:?}",
        rotations[0].1,
        pivot
    );
}

#[test]
fn direct_child_hover_hint_replays_parent_and_child_rotation_flip_chain() {
    let _guard = crate::agent_indicator_test_support::lock();
    op_editor_core::agent_indicators::clear();
    let mut scene = hover_hierarchy_scene();
    let focus = &mut scene.pages[0].children[0];
    let rotation = 0.4_f32;
    focus.rotation = rotation;
    focus.flip_y = true;
    // Children paint in reverse order; the hidden child is skipped, so
    // direct-b supplies the first dashed hierarchy stroke.
    focus.children[1].flip_x = true;

    let mut state = EditorState::new();
    state.editor_ui.canvas_hover_node = Some(op_editor_core::NodeId::new("focus"));
    let viewport = CanvasViewport::from_editor(&state, &scene);
    let mut backend = RecordingBackend::default();
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        viewport.paint(&mut cx, Rect::xywh(0.0, 0.0, 300.0, 240.0));
    }

    let transforms = active_transforms_at_first_line_stroke(&backend, HOVER_OUTLINE_COLOR)
        .expect("a direct child should paint dashed hierarchy strokes");
    assert_eq!(
        transforms.rotations.len(),
        1,
        "the child hint should inherit the focus rotation once"
    );
    assert!((transforms.rotations[0].0 - rotation).abs() < 1e-4);
    assert_eq!(
        transforms.scales.len(),
        2,
        "the child hint should replay both the focus and child flips"
    );
    assert_eq!(transforms.scales[0].0, Point2D::new(1.0, -1.0));
    assert_eq!(transforms.scales[1].0, Point2D::new(-1.0, 1.0));
}

#[test]
fn selection_overlay_on_child_applies_ancestor_rotation() {
    let _guard = crate::agent_indicator_test_support::lock();
    op_editor_core::agent_indicators::clear();
    let mut scene = sample_scene();
    let rotation = 0.5_f32;
    scene.pages[0].children[0].rotation = rotation;
    let mut state = EditorState::new();
    state.set_single_selection(op_editor_core::NodeId::new("n2"));
    let viewport = CanvasViewport::from_editor(&state, &scene);
    let mut backend = RecordingBackend::default();
    let rect = Rect::xywh(0.0, 0.0, 800.0, 600.0);
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        viewport.paint(&mut cx, rect);
    }

    let rotations = active_rotations_at_first_stroke(&backend, viewport.theme.primary)
        .expect("selected child overlay should paint primary strokes");
    assert!(
        rotations.iter().any(|(r, _)| (*r - rotation).abs() < 1e-4),
        "selection overlay should inherit the rotated ancestor transform; got {rotations:?}"
    );
}

#[test]
fn multi_selection_overlay_on_children_applies_ancestor_rotation() {
    let _guard = crate::agent_indicator_test_support::lock();
    op_editor_core::agent_indicators::clear();
    let mut scene = sample_scene();
    let rotation = 0.5_f32;
    scene.pages[0].children[0].rotation = rotation;
    let mut state = EditorState::new();
    state.selection.set = vec![
        op_editor_core::NodeId::new("n2"),
        op_editor_core::NodeId::new("n3"),
    ];
    state.selection.anchor = op_editor_core::NodeId::new("n2");
    let viewport = CanvasViewport::from_editor(&state, &scene);
    let mut backend = RecordingBackend::default();
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        viewport.paint(&mut cx, Rect::xywh(0.0, 0.0, 800.0, 600.0));
    }

    let rotations = active_rotations_at_first_stroke(&backend, viewport.theme.primary)
        .expect("multi-selection overlay should paint primary strokes");
    assert!(
        rotations.iter().any(|(r, _)| (*r - rotation).abs() < 1e-4),
        "multi-selection overlay should inherit the rotated ancestor transform; got {rotations:?}"
    );
}

#[test]
fn path_editor_overlay_on_child_applies_ancestor_rotation() {
    let _guard = crate::agent_indicator_test_support::lock();
    op_editor_core::agent_indicators::clear();
    let mut path = SceneNode::leaf("editing-path", NodeKind::Path);
    path.bounds = Rect::xywh(60.0, 80.0, 120.0, 40.0);
    path.points = vec![Point2D::new(60.0, 80.0), Point2D::new(180.0, 120.0)];
    path.path_anchors = vec![
        SceneAnchor {
            pos: Point2D::new(60.0, 80.0),
            handle_in: None,
            handle_out: None,
            point_type: ScenePointType::Corner,
        },
        SceneAnchor {
            pos: Point2D::new(180.0, 120.0),
            handle_in: None,
            handle_out: None,
            point_type: ScenePointType::Corner,
        },
    ];
    let rotation = 0.5_f32;
    let mut frame = SceneNode::leaf("rotated-frame", NodeKind::Frame);
    frame.bounds = Rect::xywh(40.0, 40.0, 320.0, 200.0);
    frame.rotation = rotation;
    frame.children = vec![path];
    let scene = LayoutScene {
        pages: vec![ScenePage {
            id: "p".into(),
            name: "p".into(),
            children: vec![frame],
        }],
        active_page_index: 0,
    };
    let mut state = EditorState::new();
    state.set_single_selection(op_editor_core::NodeId::new("editing-path"));
    let mut viewport = CanvasViewport::from_editor(&state, &scene);
    viewport.tool = op_editor_core::Tool::Select;
    let mut backend = RecordingBackend::default();
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        viewport.paint(&mut cx, Rect::xywh(0.0, 0.0, 800.0, 600.0));
    }

    let rotations = active_rotations_at_first_oval_stroke(&backend, SELECTION_BLUE)
        .expect("path editor should paint selection-blue anchor strokes");
    assert!(
        rotations.iter().any(|(r, _)| (*r - rotation).abs() < 1e-4),
        "path editor overlay should inherit the rotated ancestor transform; got {rotations:?}"
    );
}

#[test]
fn arc_handles_on_child_apply_ancestor_rotation() {
    let _guard = crate::agent_indicator_test_support::lock();
    op_editor_core::agent_indicators::clear();
    let mut ellipse = SceneNode::leaf("arc-ellipse", NodeKind::Ellipse);
    ellipse.bounds = Rect::xywh(60.0, 80.0, 120.0, 80.0);
    ellipse.arc_start_angle = Some(0.0);
    ellipse.arc_sweep_angle = Some(90.0);
    ellipse.arc_inner_radius = Some(0.5);
    let rotation = 0.5_f32;
    let mut frame = SceneNode::leaf("rotated-frame", NodeKind::Frame);
    frame.bounds = Rect::xywh(40.0, 40.0, 320.0, 200.0);
    frame.rotation = rotation;
    frame.children = vec![ellipse];
    let scene = LayoutScene {
        pages: vec![ScenePage {
            id: "p".into(),
            name: "p".into(),
            children: vec![frame],
        }],
        active_page_index: 0,
    };
    let mut state = EditorState::new();
    state.set_single_selection(op_editor_core::NodeId::new("arc-ellipse"));
    let mut viewport = CanvasViewport::from_editor(&state, &scene);
    viewport.tool = op_editor_core::Tool::Select;
    let mut backend = RecordingBackend::default();
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        viewport.paint(&mut cx, Rect::xywh(0.0, 0.0, 800.0, 600.0));
    }

    let rotations = active_rotations_at_first_oval_stroke(&backend, viewport.theme.background)
        .expect("arc handles should paint oval strokes");
    assert!(
        rotations.iter().any(|(r, _)| (*r - rotation).abs() < 1e-4),
        "arc handles should inherit the rotated ancestor transform; got {rotations:?}"
    );
}

#[test]
fn flipped_node_applies_scale_transform() {
    let state = sample_state();
    let mut node = leaf(
        "flipped",
        NodeKind::Rect,
        Rect::xywh(20.0, 30.0, 50.0, 40.0),
        Some(Color::RED),
    );
    node.flip_x = true;
    let scene = LayoutScene {
        pages: vec![ScenePage {
            id: "p".into(),
            name: "p".into(),
            children: vec![node],
        }],
        active_page_index: 0,
    };
    let viewport = CanvasViewport::from_editor(&state, &scene);
    let mut backend = RecordingBackend::default();
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        viewport.paint(&mut cx, Rect::xywh(0.0, 0.0, 200.0, 200.0));
    }

    assert!(
        backend.ops.contains(&Op::Scale),
        "flipX/flipY must apply a canvas scale transform"
    );
}
