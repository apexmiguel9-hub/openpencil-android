//! Host wiring for the TopBar hover tooltip: hovering a chrome button
//! through the real cursor path starts the dwell clock, and the dwell's
//! expiry both schedules a wake and is still recognisable as owing a
//! paint once that wake advances the clock past it.

use super::*;
use op_editor_core::TopBarButton;
use op_editor_ui::widgets::top_bar_tooltip::TOOLTIP_DWELL_MS;
use op_editor_ui::widgets::{TopBar, TOP_BAR_HEIGHT};
use op_editor_ui::{Point2D, Rect};

const VIEWPORT_W: f32 = 1400.0;
const VIEWPORT_H: f32 = 900.0;

fn hosted() -> WidgetHostNative {
    let mut host = WidgetHostNative::new();
    host.last_viewport_w = VIEWPORT_W;
    host.last_viewport_h = VIEWPORT_H;
    host
}

/// Centre of a chrome button, in window coordinates.
fn button_centre(host: &WidgetHostNative, button: TopBarButton) -> Point2D {
    let bar = TopBar::for_editor_ui(&host.editor_state().editor_ui);
    let rect = bar
        .button_rect(
            Rect {
                origin: Point2D::ZERO,
                size: Point2D::new(VIEWPORT_W, TOP_BAR_HEIGHT),
            },
            button,
        )
        .expect("button paints in this build");
    Point2D::new(
        rect.origin.x + rect.size.x / 2.0,
        rect.origin.y + rect.size.y / 2.0,
    )
}

#[test]
fn hovering_a_chrome_button_starts_the_dwell_clock() {
    let mut host = hosted();
    host.set_now_ms(1_000);
    let point = button_centre(&host, TopBarButton::OpenAssetCenter);
    host.apply_cursor_move(point.x, point.y);

    let ui = &host.editor_state().editor_ui;
    assert_eq!(
        ui.topbar_button_hover,
        Some(TopBarButton::OpenAssetCenter),
        "the asset-center button should be hovered at its own centre"
    );
    assert_eq!(ui.topbar_hover_since_ms, Some(1_000));
    assert!(
        !host.top_bar_tooltip_showing(),
        "nothing should show the instant the cursor lands"
    );
}

#[test]
fn a_dwelling_tooltip_asks_the_scheduler_to_wake_for_it() {
    let mut host = hosted();
    host.set_now_ms(1_000);
    let point = button_centre(&host, TopBarButton::OpenExportMenu);
    host.apply_cursor_move(point.x, point.y);

    assert_eq!(
        host.next_animation_deadline_ms(),
        Some(1_000 + TOOLTIP_DWELL_MS),
        "the dwell's expiry has to reach the wake scheduler"
    );
}

/// The bug this pair exists to prevent: the runner advances the clock
/// BEFORE deciding whether the wake needs a paint. By then the dwell
/// deadline has retired, so a runner asking only about deadlines would
/// discard the very wake it scheduled. `top_bar_tooltip_showing` is what
/// keeps that wake.
#[test]
fn the_dwell_wake_is_still_recognisable_after_the_clock_passes_it() {
    let mut host = hosted();
    host.set_now_ms(1_000);
    let point = button_centre(&host, TopBarButton::OpenExportMenu);
    host.apply_cursor_move(point.x, point.y);

    host.set_now_ms(1_000 + TOOLTIP_DWELL_MS);
    assert_eq!(
        host.next_animation_deadline_ms(),
        None,
        "the deadline retires the moment it is reached"
    );
    assert!(
        host.top_bar_tooltip_showing(),
        "…so the tooltip itself must be what keeps the wake alive"
    );
}

#[test]
fn leaving_the_bar_takes_the_tooltip_with_it() {
    let mut host = hosted();
    host.set_now_ms(1_000);
    let point = button_centre(&host, TopBarButton::OpenExportMenu);
    host.apply_cursor_move(point.x, point.y);
    host.set_now_ms(1_000 + TOOLTIP_DWELL_MS);
    assert!(host.top_bar_tooltip_showing());

    // Down into the canvas.
    host.apply_cursor_move(VIEWPORT_W / 2.0, TOP_BAR_HEIGHT + 300.0);
    assert_eq!(host.editor_state().editor_ui.topbar_button_hover, None);
    assert!(!host.top_bar_tooltip_showing());
    assert_eq!(host.next_animation_deadline_ms(), None);
}

#[test]
fn sliding_along_the_bar_swaps_labels_without_waiting_again() {
    let mut host = hosted();
    host.set_now_ms(1_000);
    let export = button_centre(&host, TopBarButton::OpenExportMenu);
    host.apply_cursor_move(export.x, export.y);
    host.set_now_ms(1_000 + TOOLTIP_DWELL_MS);
    assert!(host.top_bar_tooltip_showing());

    let theme = button_centre(&host, TopBarButton::ToggleTheme);
    host.apply_cursor_move(theme.x, theme.y);
    assert_eq!(
        host.editor_state().editor_ui.topbar_button_hover,
        Some(TopBarButton::ToggleTheme)
    );
    assert!(
        host.top_bar_tooltip_showing(),
        "a settled tooltip must follow the cursor along the bar, not restart its wait"
    );
}
