//! Thumbnail-cache tests: the revision key, the debounce, the per-frame
//! render budget, and the eviction of boards the deck no longer has.
//!
//! Windows-gated with the rest of the tests that paint through skia.

#![cfg(all(test, not(target_os = "windows")))]

use super::*;
use crate::backend::NativeBackend;
use op_editor_core::EditorState;
use op_editor_ui::layout_scene::ScenePage;

const THREE_BOARD_DECK: &str = r##"{
    "version": "1.0.0",
    "children": [
        { "type": "frame", "id": "slide-1", "name": "Cover", "x": 0, "y": 0,
          "width": 1920, "height": 1080,
          "fill": [{"type":"solid","color":"#ffffff"}], "children": [] },
        { "type": "frame", "id": "slide-2", "name": "Agenda", "x": 2100, "y": 0,
          "width": 1920, "height": 1080,
          "fill": [{"type":"solid","color":"#eeeeee"}], "children": [] },
        { "type": "frame", "id": "slide-3", "name": "Closing", "x": 4200, "y": 0,
          "width": 1920, "height": 1080,
          "fill": [{"type":"solid","color":"#dddddd"}], "children": [] }
    ]
}"##;

const THUMB: Point2D = Point2D { x: 194.0, y: 109.0 };

fn deck_scene() -> op_editor_ui::layout_scene::LayoutScene {
    let document = jian_ops_schema::load_str(THREE_BOARD_DECK)
        .expect("parse deck fixture")
        .value;
    let state = EditorState::from_document(document);
    op_pen_loader::editor_state_to_active_page_layout_scene(&state)
}

fn board_ids() -> Vec<String> {
    vec!["slide-1".into(), "slide-2".into(), "slide-3".into()]
}

/// Run `f` with a frame backend over a throwaway raster surface — the
/// same shape the host's paint pass hands the cache.
fn with_frame<R>(f: impl FnOnce(&mut NativeFrameBackend<'_>) -> R) -> R {
    let mut backend = NativeBackend::with_dpi(1.0);
    let mut surface =
        skia_safe::surfaces::raster_n32_premul((240, 700)).expect("raster surface allocated");
    let mut frame = NativeFrameBackend::new(&mut backend, surface.canvas());
    f(&mut frame)
}

fn wanted<'a>(
    page: &'a ScenePage,
    ids: &[String],
) -> Vec<(String, &'a op_editor_ui::layout_scene::SceneNode, Point2D)> {
    wanted_at(page, ids, THUMB)
}

/// The same request list at a chosen raster size — every row is fitted
/// into its own rect, so the size travels with the entry.
fn wanted_at<'a>(
    page: &'a ScenePage,
    ids: &[String],
    size: Point2D,
) -> Vec<(String, &'a op_editor_ui::layout_scene::SceneNode, Point2D)> {
    ids.iter()
        .filter_map(|id| page.find(id).map(|node| (id.clone(), node, size)))
        .collect()
}

#[test]
fn a_still_document_renders_and_a_moving_one_waits() {
    let mut cache = SlideThumbCache::default();
    // A brand new revision starts the debounce rather than rendering.
    assert!(!cache.tick(1, 1_000));
    assert!(!cache.tick(1, 1_000 + DEBOUNCE_MS - 1));
    assert!(cache.tick(1, 1_000 + DEBOUNCE_MS));
    // A fresh edit restarts it.
    assert!(!cache.tick(2, 2_000));
    assert!(cache.tick(2, 2_000 + DEBOUNCE_MS));
    // While waiting the host is asked to come back.
    let mut waiting = SlideThumbCache::default();
    waiting.tick(7, 500);
    assert_eq!(waiting.wake_deadline_ms(), Some(500 + DEBOUNCE_MS));
}

#[test]
fn a_frame_renders_at_most_the_budget_and_the_next_takes_over() {
    let scene = deck_scene();
    let page = scene.active_page().expect("the deck has a page").clone();
    let ids = board_ids();
    let mut cache = SlideThumbCache::default();

    with_frame(|frame| {
        let want = wanted(&page, &ids);
        assert!(cache.render_pending(frame, &want, 1));
        assert_eq!(
            cache.renders, RENDERS_PER_FRAME as u64,
            "one frame renders the budget, not the whole deck"
        );
        assert!(
            cache.has_pending(frame, &want, 1),
            "the third slide is still outstanding"
        );
        // The next frame picks up exactly what is left.
        assert!(cache.render_pending(frame, &want, 1));
        assert_eq!(cache.renders, 3);
        assert!(!cache.has_pending(frame, &want, 1));
    });

    for id in &ids {
        assert!(cache.image(id).is_some(), "{id} has a raster");
    }
}

#[test]
fn the_same_revision_never_renders_twice() {
    let scene = deck_scene();
    let page = scene.active_page().expect("page").clone();
    let ids = board_ids();
    let mut cache = SlideThumbCache::default();
    with_frame(|frame| {
        let want = wanted(&page, &ids);
        cache.render_pending(frame, &want, 4);
        cache.render_pending(frame, &want, 4);
        let after_fill = cache.renders;
        assert_eq!(after_fill, 3, "three boards, three rasters");

        // Same revision, same size: nothing to do.
        assert!(!cache.render_pending(frame, &want, 4));
        assert_eq!(cache.renders, after_fill, "a cache hit renders nothing");

        // A new revision invalidates every board.
        assert!(cache.has_pending(frame, &want, 5));
        assert!(cache.render_pending(frame, &want, 5));
        assert_eq!(cache.renders, after_fill + RENDERS_PER_FRAME as u64);
    });
}

#[test]
fn a_resized_rail_re_renders_and_a_stale_raster_still_draws() {
    let scene = deck_scene();
    let page = scene.active_page().expect("page").clone();
    let ids = vec!["slide-1".to_string()];
    let mut cache = SlideThumbCache::default();
    with_frame(|frame| {
        let want = wanted(&page, &ids);
        cache.render_pending(frame, &want, 1);
        assert_eq!(cache.renders, 1);
        // Same revision, wider rail — the raster no longer fits its box.
        let wider = wanted_at(&page, &ids, Point2D::new(THUMB.x + 40.0, THUMB.y + 22.0));
        assert!(cache.has_pending(frame, &wider, 1));
        // Meanwhile the old raster is still there to paint, which is why
        // a resize does not blink the rail back to placeholders.
        assert!(cache.image("slide-1").is_some());
        cache.render_pending(frame, &wider, 1);
        assert_eq!(cache.renders, 2);
    });
}

#[test]
fn boards_the_deck_no_longer_has_are_dropped() {
    let scene = deck_scene();
    let page = scene.active_page().expect("page").clone();
    let ids = board_ids();
    let mut cache = SlideThumbCache::default();
    with_frame(|frame| {
        let want = wanted(&page, &ids);
        cache.render_pending(frame, &want, 1);
        cache.render_pending(frame, &want, 1);
    });
    assert!(cache.image("slide-3").is_some());
    cache.retain_boards(&["slide-1".to_string()]);
    assert!(cache.image("slide-1").is_some());
    assert!(
        cache.image("slide-3").is_none(),
        "a deleted slide's raster does not outlive it"
    );
}

#[test]
fn a_zero_sized_board_renders_nothing_rather_than_panicking() {
    use op_editor_ui::layout_scene::{NodeKind, SceneNode};
    let mut node = SceneNode::leaf("empty", NodeKind::Frame);
    node.bounds = Rect::xywh(0.0, 0.0, 0.0, 0.0);
    with_frame(|frame| {
        assert!(render_board(frame, &node, THUMB).is_none());
    });
}
