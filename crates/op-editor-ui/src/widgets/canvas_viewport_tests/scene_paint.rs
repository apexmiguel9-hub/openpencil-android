//! Base scene paint tests — grid/background, primitive emission, clip isolation, reveal gating and container recursion.
//!
//! Split out of `canvas_viewport_tests.rs` to keep every file under
//! the repository's 800-line cap. Shared fixtures (`RecordingBackend`,
//! scene builders, transform-replay helpers) stay in that spine.

use super::*;

#[test]
fn from_sample_scene_paints_expected_primitives() {
    let state = sample_state();
    let scene = sample_scene();
    let mut viewport = CanvasViewport::from_editor(&state, &scene);
    // Select the Frame so the overlay stroke paints.
    viewport.selected = "n1".into();
    viewport.selected_set = vec!["n1".into()];
    let mut backend = RecordingBackend::default();
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        viewport.paint(&mut cx, Rect::xywh(0.0, 0.0, 800.0, 600.0));
    }
    // >=3 fills (canvas bg, frame fill, button rect), >=2 strokes
    // (frame outline + selection overlay), and the two scene text runs
    // still draw even when overlay labels add their own text.
    assert!(
        backend.rects >= 3,
        "expected ≥3 fills, got {}",
        backend.rects
    );
    assert!(
        backend.strokes >= 2,
        "expected ≥2 strokes (frame + selection overlay), got {}",
        backend.strokes
    );
    assert!(backend.texts.iter().any(|text| text == "Title"));
    assert!(backend.texts.iter().any(|text| text == "Button"));
}

#[test]
fn empty_scene_paints_canvas_background_and_grid_only() {
    let state = sample_state();
    let scene = LayoutScene::default();
    let viewport = CanvasViewport::from_editor(&state, &scene);
    let mut backend = RecordingBackend::default();
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        viewport.paint(&mut cx, Rect::xywh(0.0, 0.0, 100.0, 100.0));
    }
    // Infinite-canvas: bg + grid dots, no document-side strokes
    // / text.
    assert!(backend.rects >= 1, "canvas bg + grid dots");
    assert_eq!(backend.strokes, 0);
    assert_eq!(backend.text, 0);
}

#[test]
fn authored_page_background_fills_canvas_and_suppresses_grid() {
    let mut state = sample_state();
    state.doc.pages = Some(vec![PenPage {
        id: "page-1".into(),
        name: "Page 1".into(),
        children: Vec::new(),
        background_color: Some("#d7e4f380".into()),
        state: None,
        lifecycle: None,
    }]);
    let scene = LayoutScene::default();
    let viewport = CanvasViewport::from_editor(&state, &scene);
    let expected = Color {
        r: 215.0 / 255.0,
        g: 228.0 / 255.0,
        b: 243.0 / 255.0,
        a: 128.0 / 255.0,
    };
    assert_eq!(viewport.canvas_background, expected);
    assert!(!viewport.show_grid);

    let mut backend = RecordingBackend::default();
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        viewport.paint(&mut cx, Rect::xywh(0.0, 0.0, 100.0, 100.0));
    }
    assert_eq!(backend.fill_colors.first(), Some(&expected));
    assert_eq!(backend.dots, 0);
}

#[test]
fn grid_dot_count_matches_painted_dot_batch() {
    let state = sample_state();
    let scene = LayoutScene::default();
    let rect = Rect::xywh(0.0, 0.0, 320.0, 240.0);
    let paint_dots = |viewport: &CanvasViewport<'_>| -> usize {
        let mut backend = RecordingBackend::default();
        {
            let mut cx = PaintCx {
                backend: &mut backend,
            };
            viewport.paint(&mut cx, rect);
        }
        backend.dots
    };

    let viewport = CanvasViewport::from_editor(&state, &scene);
    assert_eq!(
        paint_dots(&viewport),
        crate::widgets::canvas_viewport_grid::grid_dot_count(rect, &viewport.viewport),
        "grid allocation capacity should match the dot batch exactly"
    );

    let mut viewport = CanvasViewport::from_editor(&state, &scene);
    viewport.viewport.pan_x = 17.0;
    viewport.viewport.pan_y = -23.0;
    viewport.viewport.zoom = 0.37;
    assert_eq!(
        paint_dots(&viewport),
        crate::widgets::canvas_viewport_grid::grid_dot_count(rect, &viewport.viewport),
        "panned and zoomed grid count should still match the painted batch"
    );
}

#[test]
fn empty_reveals_use_plain_node_paint_path() {
    let empty = HashMap::new();
    assert!(
        reveal_schedule_for_paint(&empty, 1_000).is_none(),
        "idle canvas paint should not give every node an empty reveal lookup"
    );

    let active = HashMap::from([("n1".to_string(), 1_000)]);
    let schedule = reveal_schedule_for_paint(&active, 1_250).expect("active reveal schedule");
    assert_eq!(schedule.now_ms, 1_250);
    assert!(std::ptr::eq(schedule.starts, &active));
}

#[test]
fn unselected_scene_skips_overlay_stroke() {
    let _guard = crate::agent_indicator_test_support::lock();
    op_editor_core::agent_indicators::clear();
    let state = sample_state();
    let scene = sample_scene();
    // No selection — only the frame's own stroke paints.
    let viewport = CanvasViewport::from_editor(&state, &scene);
    let mut backend = RecordingBackend::default();
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        viewport.paint(&mut cx, Rect::xywh(0.0, 0.0, 800.0, 600.0));
    }
    assert_eq!(backend.strokes, 1, "no selection => only the frame stroke");
}

#[test]
fn access_node_advertises_canvas_role() {
    let state = sample_state();
    let scene = sample_scene();
    let viewport = CanvasViewport::from_editor(&state, &scene);
    let node = viewport.access_node();
    assert_eq!(node.role(), accesskit::Role::Canvas);
    assert_eq!(node.label(), Some("Canvas"));
}

#[test]
fn paint_is_clip_isolated_save_clip_then_restore() {
    let state = sample_state();
    let scene = sample_scene();
    let viewport = CanvasViewport::from_editor(&state, &scene);
    let mut backend = RecordingBackend::default();
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        viewport.paint(&mut cx, Rect::xywh(0.0, 0.0, 800.0, 600.0));
    }
    // First three ops: Save, Clip, Fill (the canvas bg).
    assert_eq!(
        &backend.ops[..3],
        &[Op::Save, Op::Clip, Op::Fill],
        "canvas paint must open with Save → Clip → bg Fill"
    );
    assert_eq!(
        backend.ops.last(),
        Some(&Op::Restore),
        "canvas paint must close with Restore"
    );
    let saves = backend.ops.iter().filter(|o| **o == Op::Save).count();
    let restores = backend.ops.iter().filter(|o| **o == Op::Restore).count();
    assert_eq!(saves, restores, "balanced save/restore");
    // One outer canvas Save/Clip wraps the whole paint; on top of it
    // each Text node opens its own save/translate/scale/restore for
    // the viewport transform (flip/rotate nodes do the same). This
    // fixture paints two Text nodes, so 1 canvas + 2 text = 3 saves.
    assert_eq!(saves, 3);
}

#[test]
fn paint_with_zero_size_rect_skips_entirely() {
    let state = sample_state();
    let scene = sample_scene();
    let viewport = CanvasViewport::from_editor(&state, &scene);
    let mut backend = RecordingBackend::default();
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        viewport.paint(&mut cx, Rect::xywh(0.0, 0.0, 0.0, 0.0));
    }
    assert!(backend.ops.is_empty(), "zero-size rect must paint nothing");
}

#[test]
fn group_kind_recurses_without_own_paint() {
    let _guard = crate::agent_indicator_test_support::lock();
    op_editor_core::agent_indicators::clear();
    let state = sample_state();
    let inner = leaf(
        "n2",
        NodeKind::Rect,
        Rect::xywh(0.0, 0.0, 50.0, 50.0),
        Some(Color::RED),
    );
    let mut group = SceneNode::leaf("n3", NodeKind::Group);
    group.bounds = Rect::xywh(10.0, 10.0, 80.0, 80.0);
    group.fill = Some(Color::BLUE); // fill on group should be ignored
    group.children = vec![inner];
    let scene = LayoutScene {
        pages: vec![ScenePage {
            id: "n1".into(),
            name: "p".into(),
            children: vec![group],
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
    // canvas bg (1) + grid dots (variable) + leaf rect fill (1)
    // — group fill skipped.
    assert!(backend.rects >= 2, "canvas bg + at least the leaf");
}

#[test]
fn selection_overlay_waits_for_future_reveal_nodes() {
    let _guard = crate::agent_indicator_test_support::lock();
    let epoch = op_editor_core::agent_indicators::begin();
    op_editor_core::agent_indicators::add_reveal(epoch, "n2", 1_200);

    let child = leaf(
        "n2",
        NodeKind::Rect,
        Rect::xywh(10.0, 10.0, 50.0, 30.0),
        Some(Color::RED),
    );
    let mut frame = SceneNode::leaf("n1", NodeKind::Frame);
    frame.bounds = Rect::xywh(0.0, 0.0, 100.0, 100.0);
    frame.children = vec![child];
    let scene = LayoutScene {
        pages: vec![ScenePage {
            id: "p".into(),
            name: "p".into(),
            children: vec![frame],
        }],
        active_page_index: 0,
    };
    let mut state = sample_state();
    state.set_single_selection(op_editor_core::NodeId::new("n2"));
    let mut viewport = CanvasViewport::from_editor(&state, &scene);

    let mut pending_backend = RecordingBackend::default();
    // 900 < reveal-250: before the sparkle cursor's entry window, so no cursor strokes either.
    viewport.now_ms = 900;
    {
        let mut cx = PaintCx {
            backend: &mut pending_backend,
        };
        viewport.paint(&mut cx, Rect::xywh(0.0, 0.0, 200.0, 200.0));
    }
    assert_eq!(pending_backend.strokes, 0);

    let mut started_backend = RecordingBackend::default();
    viewport.now_ms = 1_200;
    {
        let mut cx = PaintCx {
            backend: &mut started_backend,
        };
        viewport.paint(&mut cx, Rect::xywh(0.0, 0.0, 200.0, 200.0));
    }
    assert!(
        started_backend.strokes > 0,
        "selection overlay should paint once the node starts revealing"
    );
    op_editor_core::agent_indicators::end_if_epoch(epoch);
}
