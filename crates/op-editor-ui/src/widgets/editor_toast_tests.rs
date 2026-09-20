//! Placement, hit-test, paint and scheduling guards for the toast banner.
//!
//! The state machine itself (show / expire / dismiss / supersede) is tested in
//! `op_editor_core::editor_toast`; these cover the widget-layer half: where it
//! lands relative to the chrome it must not cover, that a press resolves where
//! the cross is drawn, and that the expiry reaches the animation scheduler.

use op_editor_core::editor_toast::{EditorToastLevel, EDITOR_TOAST_LIFETIME_MS};
use op_editor_core::editor_ui_state::Locale;
use op_editor_core::EditorState;

use super::editor_toast::{EditorToast, EditorToastHit, TOAST_HEIGHT};
use super::editor_toast_flow as flow;
use super::test_capture_backend::CaptureBackend;
use super::PaintCx;
use crate::{Point2D, Rect};

const VIEWPORT: (f32, f32) = (1440.0, 900.0);
const KEY: &str = "collab.status.localEditPreserved";

fn state_with_toast(now_ms: u64) -> EditorState {
    let mut state = EditorState::new();
    state.editor_ui.locale = Locale::EnUs;
    state
        .editor_ui
        .show_toast(KEY, Vec::new(), EditorToastLevel::Warn, now_ms);
    state
}

/// Resolve the banner's rect the way a host's paint pass does.
fn painted_rect(state: &EditorState, now_ms: u64) -> Option<Rect> {
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    flow::toast_rect(&mut cx, state, VIEWPORT.0, VIEWPORT.1, now_ms).map(|(_, rect)| rect)
}

#[test]
fn nothing_paints_without_a_toast() {
    let state = EditorState::new();
    assert!(painted_rect(&state, 0).is_none());
}

#[test]
fn an_expired_toast_paints_nothing() {
    let state = state_with_toast(0);
    assert!(painted_rect(&state, 0).is_some(), "visible when fresh");
    assert!(
        painted_rect(&state, EDITOR_TOAST_LIFETIME_MS).is_none(),
        "the banner must disappear on its own, without an input event"
    );
}

#[test]
fn the_banner_sits_at_the_top_of_the_canvas_clear_of_the_tool_column() {
    // The placement rationale is in the widget's module docs: the bottom band
    // is claimed by the Toolbar, the minimized chat dock, the StatusBar and
    // the diagnostics card, so the banner lives at the top instead.
    let state = state_with_toast(0);
    let rect = painted_rect(&state, 0).expect("a banner");
    let canvas = super::host_canvas_geometry::canvas_rect(&state, VIEWPORT.0, VIEWPORT.1);

    assert!(
        rect.origin.y >= canvas.origin.y,
        "the banner must not ride up over the top bar"
    );
    assert!(
        rect.origin.y + TOAST_HEIGHT < canvas.origin.y + canvas.size.y / 2.0,
        "it belongs in the top half, not floating mid-canvas"
    );
    assert!(
        rect.origin.x > canvas.origin.x,
        "the vertical Toolbar column keeps its own clearance"
    );
    assert!(
        rect.origin.x + rect.size.x <= canvas.origin.x + canvas.size.x,
        "and the banner stays inside the canvas's right edge"
    );
}

#[test]
fn a_canvas_too_narrow_for_the_banner_paints_none_rather_than_a_clipped_one() {
    // Same rule the align toolbar follows: a clipped surface would carry
    // hit-test geometry that disagrees with what the user can see.
    let state = state_with_toast(0);
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    assert!(
        flow::toast_rect(&mut cx, &state, 120.0, 900.0, 0).is_none(),
        "a canvas that cannot hold the banner must show nothing"
    );
}

#[test]
fn the_dismiss_cross_hit_tests_where_it_is_drawn() {
    let rect = Rect::xywh(100.0, 60.0, 300.0, TOAST_HEIGHT);
    let dismiss = EditorToast::dismiss_rect(rect);

    assert!(
        rect.contains(Point2D::new(
            dismiss.origin.x + dismiss.size.x / 2.0,
            dismiss.origin.y + dismiss.size.y / 2.0,
        )),
        "the cross must live inside the banner it closes"
    );
    assert_eq!(
        EditorToast::hit_test(
            rect,
            Point2D::new(
                dismiss.origin.x + dismiss.size.x / 2.0,
                dismiss.origin.y + dismiss.size.y / 2.0,
            )
        ),
        EditorToastHit::Dismiss
    );
    // The message half of the banner is consumed but is not the cross.
    assert_eq!(
        EditorToast::hit_test(rect, Point2D::new(rect.origin.x + 8.0, rect.origin.y + 8.0)),
        EditorToastHit::Inside
    );
    // Just outside is not the banner's press at all — it is non-modal.
    assert_eq!(
        EditorToast::hit_test(rect, Point2D::new(rect.origin.x - 1.0, rect.origin.y + 8.0)),
        EditorToastHit::Outside
    );
}

#[test]
fn pressing_the_cross_dismisses_and_anything_else_falls_through() {
    let mut state = state_with_toast(0);
    let rect = painted_rect(&state, 0).expect("a banner");
    let dismiss = EditorToast::dismiss_rect(rect);

    // Outside: not consumed, and the banner stays up.
    assert!(!flow::press(
        &mut state,
        Some(rect),
        Point2D::new(rect.origin.x - 10.0, rect.origin.y),
        0
    ));
    assert!(state.editor_ui.visible_toast(0).is_some());

    // Inside but not the cross: consumed so the canvas behind is not clicked,
    // and the banner survives.
    assert!(flow::press(
        &mut state,
        Some(rect),
        Point2D::new(rect.origin.x + 4.0, rect.origin.y + 4.0),
        0
    ));
    assert!(state.editor_ui.visible_toast(0).is_some());

    // The cross: consumed and cleared.
    assert!(flow::press(
        &mut state,
        Some(rect),
        Point2D::new(
            dismiss.origin.x + dismiss.size.x / 2.0,
            dismiss.origin.y + dismiss.size.y / 2.0,
        ),
        0
    ));
    assert!(state.editor_ui.visible_toast(0).is_none());
}

#[test]
fn a_stale_rect_from_an_expired_toast_never_eats_a_press() {
    // The host caches the last painted rect. If the toast expired between that
    // paint and this press, the cached rect must not swallow a click aimed at
    // the canvas.
    let mut state = state_with_toast(0);
    let rect = painted_rect(&state, 0).expect("a banner");
    assert!(!flow::press(
        &mut state,
        Some(rect),
        Point2D::new(rect.origin.x + 4.0, rect.origin.y + 4.0),
        EDITOR_TOAST_LIFETIME_MS
    ));
}

#[test]
fn a_host_with_no_painted_rect_has_nothing_to_press() {
    let mut state = state_with_toast(0);
    assert!(!flow::press(&mut state, None, Point2D::new(10.0, 10.0), 0));
}

#[test]
fn the_expiry_reaches_the_animation_scheduler_exactly_once() {
    let state = state_with_toast(1_000);
    let ui = &state.editor_ui;

    assert_eq!(
        flow::next_deadline_ms(ui, 1_000),
        Some(1_000 + EDITOR_TOAST_LIFETIME_MS),
        "the host must be told when to wake and drop the banner"
    );
    // Once it has expired there is nothing left to wake for — a running clock
    // would repaint forever.
    assert_eq!(
        flow::next_deadline_ms(ui, 1_000 + EDITOR_TOAST_LIFETIME_MS),
        None
    );
    assert_eq!(
        flow::next_deadline_ms(&EditorState::new().editor_ui, 0),
        None
    );
}

#[test]
fn the_shared_frame_bookkeeping_folds_the_toast_deadline_in() {
    // Both hosts read their base deadline from this one function, which is how
    // native and web stay identical without either arm knowing about toasts.
    let state = state_with_toast(1_000);
    let deadline = super::host_frame_bookkeeping::base_animation_deadline_ms(&state, None, 1_000);
    assert_eq!(deadline, Some(1_000 + EDITOR_TOAST_LIFETIME_MS));
}

#[test]
fn the_banner_paints_its_localized_message_and_a_level_tinted_frame() {
    let state = state_with_toast(0);
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    let rect = flow::paint(&mut cx, &state, VIEWPORT.0, VIEWPORT.1, 0).expect("a banner");

    assert!(
        backend
            .round_fills
            .iter()
            .any(|(painted, _, _)| *painted == rect),
        "the surface must fill the rect the hit-test uses"
    );
    let message = backend
        .texts
        .first()
        .map(|(text, _)| text.clone())
        .expect("a message");
    assert!(
        message.contains("Undo"),
        "the recovery action must be named in the sentence: {message}"
    );
    assert!(
        !message.contains("collab.status"),
        "a raw key means the locale table is missing the entry: {message}"
    );
}

#[test]
fn an_unknown_key_falls_back_to_the_key_rather_than_a_blank_banner() {
    // A silent empty box is a worse bug report than a raw key.
    let mut state = EditorState::new();
    state
        .editor_ui
        .show_toast("no.such.key", Vec::new(), EditorToastLevel::Info, 0);
    let toast = EditorToast::for_editor(&state, 0).expect("a banner");
    assert_eq!(toast.message(), "no.such.key");
}
