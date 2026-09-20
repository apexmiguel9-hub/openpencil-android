//! Web twin of the native chat-input caret / scroll regressions
//! (`op-host-native/src/widget_host/chat_input_caret_tests.rs`).
//!
//! The two hosts share the flow but wire their own key tables, so the
//! guarantees are pinned on both sides.

use super::WidgetHost;
use op_editor_core::{EditorState, NodeId, PenNodeExt};
use op_editor_ui::widgets::AIChatPlaceholder;

const VIEWPORT: (f32, f32) = (1400.0, 900.0);
/// Seven authored rows — more than the input's row cap, so it scrolls.
const LONG_PROMPT: &str = "line one of the prompt\nline two of the prompt\nline three of the prompt\nline four of the prompt\nline five of the prompt\nline six of the prompt\nline seven of the prompt";

const TWO_RECTS: &str = r#"{"version":"1.0.0","children":[
    {"type":"rectangle","id":"n1","name":"One","x":40,"y":40,"width":80,"height":40},
    {"type":"rectangle","id":"n2","name":"Two","x":200,"y":40,"width":80,"height":40}
]}"#;

fn host_with_focused_chat(text: &str) -> WidgetHost {
    let document = jian_ops_schema::load_str(TWO_RECTS)
        .expect("fixture JSON parses")
        .value;
    let mut host = WidgetHost::new();
    host.editor_state = EditorState::from_document(document);
    host.last_viewport_w = VIEWPORT.0;
    host.last_viewport_h = VIEWPORT.1;
    host.editor_state.set_single_selection(NodeId::new("n1"));
    host.editor_state.chat.focused = true;
    host.editor_state.chat.set_input_text(text);
    host
}

fn selected_node_x(host: &WidgetHost) -> f64 {
    host.editor_state
        .selected_node()
        .expect("selected node")
        .base()
        .x
        .expect("fixture authors an explicit x")
}

fn input_point(host: &WidgetHost) -> op_editor_ui::Point2D {
    let chat_rect = host
        .ai_chat_rect(VIEWPORT.0, VIEWPORT.1)
        .expect("chat panel placed");
    let input = AIChatPlaceholder::from_editor_at(&host.editor_state, host.now_ms)
        .input_text_rect(chat_rect);
    op_editor_ui::Point2D::new(
        input.origin.x + input.size.x / 2.0,
        input.origin.y + input.size.y / 2.0,
    )
}

#[test]
fn horizontal_arrows_move_the_chat_caret_and_never_nudge_the_selection() {
    let mut host = host_with_focused_chat("abcd");
    let node_x = selected_node_x(&host);

    assert!(host.apply_chat_input_caret(false, false));
    assert!(host.apply_chat_input_caret(false, false));
    assert_eq!(host.editor_state.chat.input.caret(), 2);

    assert!(
        !host.apply_nudge(1.0, 0.0),
        "a focused chat input must make nudge decline the key"
    );
    assert_eq!(selected_node_x(&host), node_x);
}

#[test]
fn vertical_arrows_walk_visual_rows_of_the_chat_input() {
    let mut host = host_with_focused_chat("abcdefgh\nijklmnop\nqrstuvwx");
    let text = host.editor_state.chat.input.text().to_owned();
    let middle_row = text.find("ijkl").expect("fixture");
    let last_row = text.find("qrst").expect("fixture");
    host.editor_state.chat.input.set_caret(middle_row + 3, 0);

    assert!(host.apply_chat_input_vertical_caret(false, false));
    assert_eq!(host.editor_state.chat.input.caret(), 3, "row 0, column 3");

    assert!(host.apply_chat_input_vertical_caret(true, false));
    assert!(host.apply_chat_input_vertical_caret(true, false));
    assert_eq!(host.editor_state.chat.input.caret(), last_row + 3);
}

#[test]
fn arrows_leave_the_caret_alone_while_composing() {
    let mut host = host_with_focused_chat("abcd");
    host.editor_state.chat.input.set_caret(2, 0);
    host.editor_state.chat.input.set_composition("ni", 2, 0);

    assert!(host.apply_chat_input_caret(false, false));
    assert!(host.apply_chat_input_vertical_caret(true, false));

    assert_eq!(host.editor_state.chat.input.caret(), 2);
    assert!(host.editor_state.chat.input.composition().is_some());
}

#[test]
fn wheel_over_the_chat_input_scrolls_it_and_leaves_the_canvas_alone() {
    let mut host = host_with_focused_chat(LONG_PROMPT);
    let point = input_point(&host);
    let viewport_before = host.editor_state.viewport;
    let chat_rect = host
        .ai_chat_rect(VIEWPORT.0, VIEWPORT.1)
        .expect("chat panel placed");
    let (start, max) = AIChatPlaceholder::from_editor_at(&host.editor_state, host.now_ms)
        .input_scroll_state(chat_rect);
    assert!(max > 0.0, "fixture must overflow the input box");

    assert!(host.apply_wheel(point.x, point.y, 60.0, VIEWPORT.0, VIEWPORT.1));

    let (after, _) = AIChatPlaceholder::from_editor_at(&host.editor_state, host.now_ms)
        .input_scroll_state(chat_rect);
    assert!(
        after < start,
        "the input should have scrolled ({after} vs {start})"
    );
    assert_eq!(host.editor_state.viewport, viewport_before);
}
