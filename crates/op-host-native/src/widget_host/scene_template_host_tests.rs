//! Native-host routing for the Scene Template Center.

use super::WidgetHostNative;
use op_editor_ui::widgets::SceneTemplatePanel;
use op_editor_ui::Point2D;

const VIEWPORT_W: f32 = 1_200.0;
const VIEWPORT_H: f32 = 800.0;

fn open_host() -> WidgetHostNative {
    let mut host = WidgetHostNative::new();
    host.last_viewport_w = VIEWPORT_W;
    host.last_viewport_h = VIEWPORT_H;
    host.editor_state_mut()
        .editor_ui
        .open_scene_template_center(1);
    host
}

/// Wheel AND trackpad must both scroll the grid, never the canvas.
///
/// Wheel deltas route through `apply_wheel_inner` while trackpad pans route
/// through `apply_pan_gesture` — two separate ladders. The panel was wired
/// into only the first, so a two-finger scroll over the card grid moved the
/// canvas underneath while the grid sat still (reported 2026-08-02). A panel
/// present in one ladder and absent from the other looks correct in every
/// mouse test.
#[test]
fn scene_template_wheel_and_trackpad_scroll_without_moving_viewport() {
    let mut host = open_host();
    let panel_rect = host
        .scene_template_panel_rect(VIEWPORT_W, VIEWPORT_H)
        .expect("scene template rect");
    let point = Point2D::new(
        panel_rect.origin.x + panel_rect.size.x / 2.0,
        panel_rect.origin.y + panel_rect.size.y / 2.0,
    );
    assert!(
        SceneTemplatePanel::for_editor(host.editor_state())
            .expect("open scene template centre")
            .max_scroll(panel_rect)
            > 0.0,
        "the shipped catalogue must overflow the two-column grid"
    );
    let viewport = host.editor_state().viewport;

    assert!(host.apply_wheel(point.x, point.y, -120.0, VIEWPORT_W, VIEWPORT_H));
    let wheel_offset = host
        .editor_state()
        .editor_ui
        .scene_template_center
        .scroll
        .offset;
    assert!(wheel_offset > 0.0, "wheel must scroll the grid");
    assert_eq!(
        host.editor_state().viewport,
        viewport,
        "the canvas must not move under a wheel over the panel"
    );

    assert!(host.apply_pan_gesture(point.x, point.y, 35.0, -80.0, VIEWPORT_W, VIEWPORT_H));
    assert!(
        host.editor_state()
            .editor_ui
            .scene_template_center
            .scroll
            .offset
            > wheel_offset,
        "a trackpad vertical delta must keep scrolling the grid"
    );
    assert_eq!(
        host.editor_state().viewport,
        viewport,
        "the canvas must not move under a trackpad pan over the panel"
    );
}

/// Outside the panel is the gallery's scrim, not the canvas.
///
/// The gallery fills the canvas region and dims the rest of the window, so
/// both ladders must stop at the scrim: the grid does not scroll (the pointer
/// is not over it) and neither does the canvas (nobody can see it move).
#[test]
fn scrolling_on_the_scrim_moves_neither_the_grid_nor_the_canvas() {
    let mut host = open_host();
    let panel_rect = host
        .scene_template_panel_rect(VIEWPORT_W, VIEWPORT_H)
        .expect("scene template rect");
    let outside = Point2D::new(panel_rect.origin.x - 20.0, panel_rect.origin.y - 20.0);
    let before = host.editor_state().viewport;

    host.apply_pan_gesture(outside.x, outside.y, 0.0, -60.0, VIEWPORT_W, VIEWPORT_H);
    host.apply_wheel(outside.x, outside.y, -120.0, VIEWPORT_W, VIEWPORT_H);
    assert_eq!(
        host.editor_state().viewport,
        before,
        "the scrim must not hand the canvas a scroll the user cannot see"
    );
    assert_eq!(
        host.editor_state()
            .editor_ui
            .scene_template_center
            .scroll
            .offset,
        0.0,
        "the grid must not scroll from a pointer outside it"
    );
}

/// A press on the scrim dismisses, and does not reach the chrome underneath.
///
/// The gallery hit-tests above the top bar and the rails, so this is the only
/// thing standing between a half-visible dimmed button and a click that fires
/// it — which is exactly what a scrim is supposed to prevent.
#[test]
fn pressing_the_scrim_closes_the_gallery_without_reaching_the_chrome() {
    let mut host = open_host();
    let panel_rect = host
        .scene_template_panel_rect(VIEWPORT_W, VIEWPORT_H)
        .expect("scene template rect");
    let selection_before = host.editor_state().selection.clone();
    let scrim = Point2D::new(panel_rect.origin.x - 12.0, panel_rect.origin.y - 12.0);

    assert!(host.apply_press(scrim.x, scrim.y, VIEWPORT_W, VIEWPORT_H));
    assert!(!host.editor_state().editor_ui.scene_template_center.open);
    assert_eq!(
        host.editor_state().selection,
        selection_before,
        "the press must not have fallen through to a canvas selection"
    );
}
