//! Floating AI-chat panel: header actions, transcript text selection,
//! resize edges, and the streaming-disabled input guards.
//!
//! Split out of `input_tests.rs` to keep every file under the repo's
//! 800-line cap.

use super::*;

#[test]
fn ai_chat_maximize_click_expands_panel_geometry() {
    let mut host = WidgetHostNative::new();
    let before = host
        .ai_chat_rect(1200.0, 800.0)
        .expect("chat panel visible");
    let x = before.origin.x + before.size.x - 16.0 - 50.0 + 9.0;
    let y = before.origin.y + 17.0;

    assert!(host.apply_click(x, y, 1200.0, 800.0));

    let after = host
        .ai_chat_rect(1200.0, 800.0)
        .expect("maximized chat panel visible");
    assert!(host.editor_state().chat.maximized);
    assert!(after.size.x > before.size.x);
    assert!(after.size.y > before.size.y);
}

#[test]
fn ai_chat_new_chat_click_clears_transcript_and_queues_abort() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut()
        .chat
        .messages
        .push(op_editor_core::ChatMessage::assistant("old"));
    host.editor_state_mut().chat.set_input_text("draft");
    let rect = host
        .ai_chat_rect(1200.0, 800.0)
        .expect("chat panel visible");
    let x = rect.origin.x + rect.size.x - 16.0 - 22.0 + 9.0;
    let y = rect.origin.y + 17.0;

    assert!(host.apply_click(x, y, 1200.0, 800.0));

    assert!(host.editor_state().chat.messages.is_empty());
    assert!(host.editor_state().chat.input.text().is_empty());
    assert!(host.editor_state().chat.pending_new_chat);
}

#[test]
fn dragging_user_transcript_text_selects_and_copy_queues_text() {
    let prompt = "生成一个设计精良的美食应用移动端首页";
    let mut host = WidgetHostNative::new();
    host.editor_state_mut()
        .chat
        .messages
        .push(op_editor_core::ChatMessage::user(prompt));
    let viewport_w = 1200.0;
    let viewport_h = 800.0;
    let rect = host
        .ai_chat_rect(viewport_w, viewport_h)
        .expect("chat panel visible");
    // At the 360px default panel width this 18-char CJK prompt wraps to two
    // bubble lines (line 0 = first 17 chars, line 1 = the final "页"), so a
    // whole-prompt drag has to run from line 0 down into line 1. A horizontal
    // drag on line 0 alone can only reach the end of line 0, dropping "页".
    let start_x = rect.origin.x + 96.0;
    let start_y = rect.origin.y + 74.0; // line 0
    let end_x = rect.origin.x + 338.0;
    let end_y = rect.origin.y + 88.0; // line 1 (the wrapped final glyph)

    assert!(host.apply_press(start_x, start_y, viewport_w, viewport_h));
    assert!(host.apply_cursor_move(end_x, end_y));
    assert!(host.apply_release_with_viewport(viewport_w, viewport_h));

    assert_eq!(
        host.editor_state().chat.selected_transcript_text(),
        Some(prompt)
    );
    assert!(host.apply_copy());
    assert_eq!(
        host.editor_state().chat.pending_copy_text.as_deref(),
        Some(prompt)
    );
}

#[test]
fn user_transcript_text_uses_text_cursor() {
    let prompt = "生成一个设计精良的美食应用移动端首页";
    let mut host = WidgetHostNative::new();
    host.editor_state_mut()
        .chat
        .messages
        .push(op_editor_core::ChatMessage::user(prompt));
    let viewport_w = 1200.0;
    let viewport_h = 800.0;
    let rect = host
        .ai_chat_rect(viewport_w, viewport_h)
        .expect("chat panel visible");
    let x = rect.origin.x + 96.0;
    let y = rect.origin.y + 74.0;

    // `cursor_hint` now reads the LAST BUILT transcript layout (zero hashes on
    // that pass) rather than re-fingerprinting the live transcript. Prime that
    // build the way a paint / cursor-move would — a press over the transcript
    // resolves + stores the canonical — then the hint reads the stored build and
    // flips to the text cursor. This exercises the native cursor_hint end-to-end
    // (event ordering: build stored, then hint reads it).
    assert!(host.apply_press(x, y, viewport_w, viewport_h));
    assert!(host.apply_release_with_viewport(viewport_w, viewport_h));

    assert_eq!(
        host.cursor_hint(x, y, viewport_w, viewport_h),
        CursorHint::Text
    );
}

#[test]
fn chat_input_text_uses_text_cursor() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut().chat.set_input_text("abcdef");
    let viewport_w = 1200.0;
    let viewport_h = 800.0;
    let rect = host
        .ai_chat_rect(viewport_w, viewport_h)
        .expect("chat panel visible");
    let x = rect.origin.x + 32.0;
    let y = rect.origin.y + textarea_center_y_for_test();

    assert_eq!(
        host.cursor_hint(x, y, viewport_w, viewport_h),
        CursorHint::Text
    );
}

#[test]
fn ai_chat_east_edge_shows_resize_cursor() {
    let host = WidgetHostNative::new();
    let rect = host
        .ai_chat_rect(1200.0, 800.0)
        .expect("chat panel visible");
    let x = rect.origin.x + rect.size.x - 2.0;
    let y = rect.origin.y + rect.size.y / 2.0;

    assert_eq!(host.cursor_hint(x, y, 1200.0, 800.0), CursorHint::ResizeEw);
}

#[test]
fn ai_chat_east_edge_drag_resizes_panel_width() {
    let mut host = WidgetHostNative::new();
    let before = host
        .ai_chat_rect(1200.0, 800.0)
        .expect("chat panel visible");
    let x = before.origin.x + before.size.x - 2.0;
    let y = before.origin.y + before.size.y / 2.0;

    assert!(host.apply_press(x, y, 1200.0, 800.0));
    assert!(host.apply_cursor_move(x + 72.0, y));

    let after = host
        .ai_chat_rect(1200.0, 800.0)
        .expect("chat panel visible after resize");
    assert!(
        after.size.x > before.size.x + 60.0,
        "dragging the east edge should grow chat width; before={before:?}, after={after:?}"
    );
    assert_eq!(after.origin.x, before.origin.x);
}

#[test]
fn ai_chat_stop_click_keeps_transcript_and_queues_abort() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut()
        .chat
        .messages
        .push(op_editor_core::ChatMessage::user("make a dashboard"));
    host.editor_state_mut()
        .chat
        .messages
        .push(op_editor_core::ChatMessage::assistant_streaming());
    let rect = host
        .ai_chat_rect(1200.0, 800.0)
        .expect("chat panel visible");
    let x = rect.origin.x + rect.size.x - 16.0 - 20.0;
    let y = rect.origin.y + toolbar_center_y_for_test();

    assert!(host.apply_click(x, y, 1200.0, 800.0));

    assert_eq!(host.editor_state().chat.messages.len(), 2);
    assert!(!host.editor_state().chat.messages[1].streaming);
    assert!(host.editor_state().chat.pending_stop_chat);
}

#[test]
fn ai_chat_agent_team_click_sets_team_size_via_parallel_agents_picker() {
    // #32: the ⚡ footer chip no longer cycles the team size on click — it
    // opens the Parallel Agents picker, and clicking a row (1x–6x) sets
    // `agent_team_size`. Drive that two-step flow through the real hit-test
    // routing (probe with `hit_test`, then click the resolved points).
    use op_editor_ui::widgets::AIChatHit;
    let mut host = WidgetHostNative::new();
    host.editor_state_mut()
        .chat
        .available_models
        .push(op_editor_core::chat::ModelEntry::new(
            op_editor_core::chat::AgentProvider::CodexCli,
            "gpt-5",
            "GPT-5",
        ));
    let viewport_w = 1200.0;
    let viewport_h = 800.0;
    host.last_viewport_w = viewport_w;
    host.last_viewport_h = viewport_h;
    let rect = host.ai_chat_rect(viewport_w, viewport_h).unwrap();

    // Locate the ⚡ speed chip along the footer toolbar row and click it to
    // open the picker.
    let panel = op_editor_ui::widgets::AIChatPlaceholder::from_editor(host.editor_state());
    let toolbar_y = rect.origin.y + toolbar_center_y_for_test();
    let mut speed_point = None;
    let mut sx = rect.origin.x;
    while sx < rect.origin.x + rect.size.x {
        let p = op_editor_ui::Point2D::new(sx, toolbar_y);
        if panel.hit_test(rect, p) == Some(AIChatHit::ToggleParallelAgentsPicker) {
            speed_point = Some(p);
            break;
        }
        sx += 1.0;
    }
    let speed_point = speed_point.expect("footer speed chip should be hittable");
    assert!(host.apply_click(speed_point.x, speed_point.y, viewport_w, viewport_h));
    assert!(host.editor_state().editor_ui.parallel_agents_picker_open);

    // With the picker open, find and click the "2x" row.
    let panel = op_editor_ui::widgets::AIChatPlaceholder::from_editor(host.editor_state());
    let mut row_point = None;
    let mut ry = rect.origin.y;
    'scan: while ry < rect.origin.y + rect.size.y {
        let mut rx = rect.origin.x;
        while rx < rect.origin.x + rect.size.x {
            let p = op_editor_ui::Point2D::new(rx, ry);
            if panel.hit_test(rect, p) == Some(AIChatHit::SetParallelAgents(2)) {
                row_point = Some(p);
                break 'scan;
            }
            rx += 2.0;
        }
        ry += 2.0;
    }
    let row_point = row_point.expect("parallel agents picker row 2 should be hittable");
    assert!(host.apply_click(row_point.x, row_point.y, viewport_w, viewport_h));
    assert_eq!(host.editor_state().chat.agent_team_size, 2);
    assert!(!host.editor_state().editor_ui.parallel_agents_picker_open);
}

#[test]
fn ai_chat_streaming_textarea_click_does_not_focus_disabled_input() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut()
        .chat
        .messages
        .push(op_editor_core::ChatMessage::assistant_streaming());
    let rect = host
        .ai_chat_rect(1200.0, 800.0)
        .expect("chat panel visible");
    let x = rect.origin.x + 120.0;
    let y = rect.origin.y + textarea_center_y_for_test();

    assert!(host.apply_click(x, y, 1200.0, 800.0));

    assert!(!host.editor_state().chat.focused);
}

#[test]
fn ai_chat_streaming_attachment_click_does_not_open_picker() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut()
        .chat
        .messages
        .push(op_editor_core::ChatMessage::assistant_streaming());
    let rect = host
        .ai_chat_rect(1200.0, 800.0)
        .expect("chat panel visible");
    let x = rect.origin.x + op_editor_ui::widgets::AI_CHAT_WIDTH - 16.0 - 52.0;
    let y = rect.origin.y + toolbar_center_y_for_test();

    assert!(host.apply_click(x, y, 1200.0, 800.0));

    assert!(!host.editor_state().chat.pending_attachment_pick);
}
