use op_editor_core::EditorState;

use super::{cursor_hover_flow, host_overlay_geometry, scroll_flow, PromptCenterPanel};
use crate::widgets::TOP_BAR_HEIGHT;
use crate::{Point2D, Rect};

fn open_state() -> EditorState {
    let mut state = EditorState::new();
    state.editor_ui.open_prompt_center(1);
    state
}

#[test]
fn hover_owns_cards_and_padding_and_clears_after_exit() {
    let mut state = open_state();
    let rect = Rect::xywh(100.0, 80.0, 720.0, 520.0);
    let card = PromptCenterPanel::for_editor(&state)
        .unwrap()
        .card_rects(rect)[0]
        .1;

    let (owns_card, card_changed) = cursor_hover_flow::prompt_center_hover(
        &mut state,
        rect,
        Point2D::new(card.origin.x + 8.0, card.origin.y + 8.0),
    );
    assert!(owns_card);
    assert!(card_changed);
    assert_eq!(state.editor_ui.prompt_center.hover, Some(0));

    let (owns_padding, padding_changed) = cursor_hover_flow::prompt_center_hover(
        &mut state,
        rect,
        Point2D::new(rect.origin.x + 4.0, rect.origin.y + 250.0),
    );
    assert!(owns_padding);
    assert!(padding_changed);
    assert_eq!(state.editor_ui.prompt_center.hover, None);

    state.editor_ui.prompt_center.hover = Some(0);
    let (owns_outside, outside_changed) = cursor_hover_flow::prompt_center_hover(
        &mut state,
        rect,
        Point2D::new(rect.origin.x - 1.0, rect.origin.y),
    );
    assert!(!owns_outside);
    assert!(outside_changed);
    assert_eq!(state.editor_ui.prompt_center.hover, None);
}

#[test]
fn scroll_owns_entire_panel_and_clamps_the_card_grid() {
    let mut state = open_state();
    let rect = Rect::xywh(100.0, 80.0, 720.0, 520.0);
    let header = Point2D::new(rect.origin.x + 20.0, rect.origin.y + 20.0);
    state.editor_ui.prompt_center.hover = Some(0);

    assert_eq!(
        scroll_flow::scroll_prompt_center(&mut state, Some(rect), header, -80.0),
        Some(true)
    );
    assert_eq!(state.editor_ui.prompt_center.scroll.offset, 80.0);
    assert_eq!(state.editor_ui.prompt_center.hover, None);

    let maximum = PromptCenterPanel::for_editor(&state)
        .unwrap()
        .max_scroll(rect);
    assert_eq!(
        scroll_flow::scroll_prompt_center(&mut state, Some(rect), header, -100_000.0),
        Some(true)
    );
    assert_eq!(state.editor_ui.prompt_center.scroll.offset, maximum);
    assert_eq!(
        scroll_flow::scroll_prompt_center(
            &mut state,
            Some(rect),
            Point2D::new(rect.origin.x - 1.0, rect.origin.y),
            -20.0,
        ),
        None
    );

    state.editor_ui.close_prompt_center();
    assert_eq!(
        scroll_flow::scroll_prompt_center(&mut state, Some(rect), header, -20.0),
        None
    );
}

#[test]
fn small_viewport_keeps_the_entire_panel_and_close_action_visible() {
    let state = open_state();
    let viewport_w = 640.0;
    let viewport_h = 400.0;
    let rect =
        host_overlay_geometry::prompt_center_panel_rect(&state, viewport_w, viewport_h).unwrap();

    assert!(rect.origin.x >= 0.0);
    assert!(rect.origin.y >= 0.0);
    assert!(rect.origin.x + rect.size.x <= viewport_w);
    assert!(rect.origin.y + rect.size.y <= viewport_h);
    // The panel scales with the window rather than filling it, but on a
    // window this short the height floor is what it gets — clamped to the
    // space below the top bar so nothing hangs off the bottom edge.
    assert!(rect.size.x < viewport_w && rect.size.x > viewport_w / 2.0);
    assert!(rect.size.y <= viewport_h - TOP_BAR_HEIGHT);

    let close = PromptCenterPanel::close_rect(rect);
    let close_center = Point2D::new(
        close.origin.x + close.size.x / 2.0,
        close.origin.y + close.size.y / 2.0,
    );
    assert!(rect.contains(close_center));
}
