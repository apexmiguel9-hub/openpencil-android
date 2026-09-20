//! Traversal-cost tests — selection / pen / frame-label lookups must reuse the single paint walk instead of deep-searching.
//!
//! Split out of `canvas_viewport_tests.rs` to keep every file under
//! the repository's 800-line cap. Shared fixtures (`RecordingBackend`,
//! scene builders, transform-replay helpers) stay in that spine.

use super::*;

#[test]
fn single_selected_canvas_paint_reuses_paint_traversal_lookup() {
    let _guard = crate::agent_indicator_test_support::lock();
    op_editor_core::agent_indicators::clear();

    let mut selected = SceneNode::leaf("selected-ellipse", NodeKind::Ellipse);
    selected.bounds = Rect::xywh(24.0, 24.0, 40.0, 40.0);
    selected.arc_start_angle = Some(0.0);
    selected.arc_sweep_angle = Some(180.0);
    let mut node = selected;
    let depth = 7;
    for i in (0..depth).rev() {
        let mut frame = SceneNode::leaf(format!("wrap-{i}"), NodeKind::Frame);
        frame.bounds = Rect::xywh(0.0, 0.0, 120.0, 120.0);
        frame.children = vec![node];
        node = frame;
    }
    let scene = LayoutScene {
        pages: vec![ScenePage {
            id: "p".into(),
            name: "p".into(),
            children: vec![node],
        }],
        active_page_index: 0,
    };
    let mut state = EditorState::new();
    state.viewport.zoom = 1.0;
    state.viewport.pan_x = 0.0;
    state.viewport.pan_y = 0.0;
    state.set_single_selection(op_editor_core::NodeId::new("selected-ellipse"));
    state.tool = op_editor_core::Tool::Select;
    let viewport = CanvasViewport::from_editor(&state, &scene);

    crate::layout_scene::reset_find_visit_count();
    let mut backend = RecordingBackend::default();
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        viewport.paint(&mut cx, Rect::xywh(0.0, 0.0, 300.0, 300.0));
    }

    assert_eq!(
        crate::layout_scene::find_visit_count(),
        0,
        "single-selected paint should use the scene node already seen during paint traversal"
    );
}

#[test]
fn pen_preview_canvas_paint_reuses_paint_traversal_lookup() {
    let _guard = crate::agent_indicator_test_support::lock();
    op_editor_core::agent_indicators::clear();

    let anchor = |x, y| SceneAnchor {
        pos: Point2D::new(x, y),
        handle_in: None,
        handle_out: None,
        point_type: ScenePointType::Corner,
    };
    let mut path = SceneNode::leaf("editing-path", NodeKind::Path);
    path.bounds = Rect::xywh(24.0, 24.0, 80.0, 40.0);
    path.points = vec![Point2D::new(24.0, 24.0), Point2D::new(104.0, 64.0)];
    path.path_anchors = vec![anchor(24.0, 24.0), anchor(104.0, 64.0)];
    let mut node = path;
    for i in (0..7).rev() {
        let mut frame = SceneNode::leaf(format!("wrap-{i}"), NodeKind::Frame);
        frame.bounds = Rect::xywh(0.0, 0.0, 140.0, 100.0);
        frame.children = vec![node];
        node = frame;
    }
    let scene = LayoutScene {
        pages: vec![ScenePage {
            id: "p".into(),
            name: "p".into(),
            children: vec![node],
        }],
        active_page_index: 0,
    };
    let state = EditorState::new();
    let mut viewport = CanvasViewport::from_editor(&state, &scene);
    viewport.pen_in_progress = Some("editing-path".into());
    viewport.pen_cursor_doc = Some(Point2D::new(120.0, 80.0));

    crate::layout_scene::reset_find_visit_count();
    let mut backend = RecordingBackend::default();
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        viewport.paint(&mut cx, Rect::xywh(0.0, 0.0, 300.0, 300.0));
    }

    assert_eq!(
        crate::layout_scene::find_visit_count(),
        0,
        "pen preview paint should use the scene node already seen during paint traversal"
    );
    assert!(backend.strokes > 0, "pen preview should still paint");
}

#[test]
fn frame_label_paint_matches_roots_linearly() {
    let _guard = crate::agent_indicator_test_support::lock();
    op_editor_core::agent_indicators::clear();

    let mut roots = Vec::new();
    let mut labels = Vec::new();
    for i in 0..32 {
        let mut frame = SceneNode::leaf(format!("frame-{i}"), NodeKind::Frame);
        frame.bounds = Rect::xywh(i as f32 * 90.0, 0.0, 80.0, 48.0);
        roots.push(frame);
        labels.push(FrameLabel::new(
            format!("frame-{i}"),
            format!("Frame {i}"),
            Color {
                r: 0.6,
                g: 0.6,
                b: 0.6,
                a: 1.0,
            },
            false,
        ));
    }
    let scene = LayoutScene {
        pages: vec![ScenePage {
            id: "p".into(),
            name: "p".into(),
            children: roots,
        }],
        active_page_index: 0,
    };
    let mut state = EditorState::new();
    state.viewport.zoom = 1.0;
    let mut viewport = CanvasViewport::from_editor(&state, &scene);
    viewport.frame_labels = labels;

    crate::widgets::canvas_frame_labels::reset_label_match_count();
    let mut backend = RecordingBackend::default();
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        viewport.paint(&mut cx, Rect::xywh(0.0, 0.0, 3_200.0, 240.0));
    }

    assert!(
        crate::widgets::canvas_frame_labels::label_match_count() <= 32,
        "frame label matching should stay linear in the number of top-level roots"
    );
}

#[test]
fn multi_selection_overlay_finds_nodes_linearly() {
    let _guard = crate::agent_indicator_test_support::lock();
    op_editor_core::agent_indicators::clear();

    let roots: Vec<_> = (0..32)
        .map(|i| {
            let mut frame = SceneNode::leaf(format!("frame-{i}"), NodeKind::Frame);
            frame.bounds = Rect::xywh(i as f32 * 12.0, 0.0, 10.0, 10.0);
            frame
        })
        .collect();
    let scene = LayoutScene {
        pages: vec![ScenePage {
            id: "p".into(),
            name: "p".into(),
            children: roots,
        }],
        active_page_index: 0,
    };
    let state = EditorState::new();
    let mut viewport = CanvasViewport::from_editor(&state, &scene);
    viewport.selected_set = (0..32).map(|i| format!("frame-{i}")).collect();
    viewport.frame_labels.clear();

    crate::layout_scene::reset_find_visit_count();
    let mut backend = RecordingBackend::default();
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        viewport.paint(&mut cx, Rect::xywh(0.0, 0.0, 800.0, 200.0));
    }

    assert!(
        crate::layout_scene::find_visit_count() <= 32,
        "multi-selection overlay should resolve selected nodes in one scene pass"
    );
}

#[test]
fn hover_outline_does_not_deep_search_after_scene_paint() {
    let _guard = crate::agent_indicator_test_support::lock();
    op_editor_core::agent_indicators::clear();

    let mut node = SceneNode::leaf("hovered-leaf", NodeKind::Rect);
    node.bounds = Rect::xywh(24.0, 24.0, 40.0, 40.0);
    let depth = 8;
    for i in (0..depth).rev() {
        let mut frame = SceneNode::leaf(format!("wrap-{i}"), NodeKind::Frame);
        frame.bounds = Rect::xywh(0.0, 0.0, 120.0, 120.0);
        frame.children = vec![node];
        node = frame;
    }
    let scene = LayoutScene {
        pages: vec![ScenePage {
            id: "p".into(),
            name: "p".into(),
            children: vec![node],
        }],
        active_page_index: 0,
    };
    let state = EditorState::new();
    let mut viewport = CanvasViewport::from_editor(&state, &scene);
    viewport.hovered = Some("hovered-leaf".into());

    crate::layout_scene::reset_find_visit_count();
    let mut backend = RecordingBackend::default();
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        viewport.paint(&mut cx, Rect::xywh(0.0, 0.0, 300.0, 300.0));
    }

    assert_eq!(
        crate::layout_scene::find_visit_count(),
        0,
        "hover outline should paint during the existing scene walk instead of deep-searching after paint"
    );
    assert!(
        backend.strokes > 0,
        "hover outline should still paint dashed strokes"
    );
}
