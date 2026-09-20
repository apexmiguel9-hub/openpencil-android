//! Text-offset resolution and the selection wash painted over user text.
//!
//! Split out of `ai_chat_transcript_tests.rs` to keep that file under the
//! 800-line cap.

use super::*;

#[test]
fn transcript_text_offset_at_resolves_user_message_text() {
    let prompt = "生成一个设计精良的美食应用移动端首页";
    let messages = [ChatMessage::user(prompt)];
    let body = body();
    let items = build_transcript(&messages, body, op_editor_core::Locale::EnUs);
    let bubble = items[0].bubble.as_ref().expect("user prompt bubble");
    // Click within the user bubble text area (past the USER_BUBBLE_PAD inset).
    let point = Point2D::new(
        bubble.rect.origin.x + USER_BUBBLE_PAD + 22.0,
        bubble.rect.origin.y + USER_BUBBLE_PAD + 2.0,
    );

    let canonical = crate::widgets::ai_chat_transcript_cache::unowned_for_tests(
        &messages,
        body,
        op_editor_core::Locale::EnUs,
    );
    let hit = transcript_text_offset_at(&messages, &canonical, body, point, 0.0)
        .expect("user message text should be selectable");

    assert_eq!(hit.message_index, 0);
    assert!(hit.offset > 0);
    assert!(hit.offset <= prompt.len());
}

#[test]
fn paint_transcript_highlights_selected_user_text() {
    let prompt = "生成一个设计精良的美食应用移动端首页";
    let messages = [ChatMessage::user(prompt)];
    let selection = op_editor_core::chat::ChatTranscriptSelection {
        message_index: 0,
        anchor: 0,
        focus: prompt.len(),
    };
    let theme = crate::Theme::dark();
    let mut backend = TranscriptPaintBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    let canonical = crate::widgets::ai_chat_transcript_cache::unowned_for_tests(
        &messages,
        body(),
        op_editor_core::Locale::EnUs,
    );
    paint_transcript_with_selection(
        &mut cx,
        &theme,
        body(),
        &messages,
        &canonical,
        0,
        None,
        Some(selection),
        0.0,
    );

    assert!(
        backend
            .round_rect_colors
            .iter()
            .any(|color| *color == crate::widgets::text_selection::selection_color(&theme)),
        "selected transcript text should paint a visible selection wash"
    );
}
