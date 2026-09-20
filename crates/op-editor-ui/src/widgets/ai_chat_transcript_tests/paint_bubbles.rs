//! Painted bubbles: activity loaders, answer framing and typing placeholders.
//!
//! Split out of `ai_chat_transcript_tests.rs` to keep that file under the
//! 800-line cap.

use super::*;

#[test]
fn cli_running_activity_uses_the_shared_rotating_loader() {
    let mut message = ChatMessage::assistant_streaming();
    message.activities.push(op_editor_core::ChatActivity {
        id: "build".into(),
        title: "Building the screen".into(),
        detail: None,
        status: op_editor_core::ChatActivityStatus::Running,
        content_offset: None,
    });
    let mut backend = TranscriptPaintBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    paint_transcript(
        &mut cx,
        &crate::Theme::dark(),
        body(),
        &[message],
        250,
        op_editor_core::Locale::EnUs,
    );

    assert_eq!(backend.rotations.len(), 1);
    assert!((backend.rotations[0] - std::f32::consts::FRAC_PI_2).abs() < 0.0001);
}

#[test]
fn cli_pending_activity_uses_a_quiet_non_rotating_wait_ring() {
    let mut message = ChatMessage::assistant_streaming();
    message.activities.push(op_editor_core::ChatActivity {
        id: "queued".into(),
        title: "Queued section".into(),
        detail: None,
        status: op_editor_core::ChatActivityStatus::Pending,
        content_offset: None,
    });
    let mut backend = TranscriptPaintBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    paint_transcript(
        &mut cx,
        &crate::Theme::dark(),
        body(),
        &[message],
        250,
        op_editor_core::Locale::EnUs,
    );

    assert!(backend.rotations.is_empty());
}

#[test]
fn paint_transcript_leaves_assistant_answer_unframed() {
    let messages = [ChatMessage::assistant("assistant answer")];
    let mut backend = TranscriptPaintBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    paint_transcript(
        &mut cx,
        &crate::Theme::dark(),
        body(),
        &messages,
        0,
        op_editor_core::Locale::EnUs,
    );

    assert_eq!(backend.round_rects.len(), 0);
}

#[test]
fn paint_transcript_keeps_user_answer_bubble_background() {
    // #27 restyle: user bubble uses theme.user_bubble (medium-gray),
    // replacing the old theme.row_selected_primary (blue-tinted wash).
    let messages = [ChatMessage::user("user prompt")];
    let theme = crate::Theme::dark();
    let mut backend = TranscriptPaintBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    paint_transcript(
        &mut cx,
        &theme,
        body(),
        &messages,
        0,
        op_editor_core::Locale::EnUs,
    );

    assert_eq!(backend.round_rects.len(), 1);
    // Old: theme.row_selected_primary (blue-tinted). New: theme.user_bubble (medium-gray).
    assert_eq!(backend.round_rect_colors[0], theme.user_bubble);
    assert_ne!(backend.round_rect_colors[0], theme.primary);
}

#[test]
fn streaming_message_with_no_text_yields_a_typing_bubble() {
    let msgs = vec![ChatMessage::assistant_streaming()];
    let items = build_transcript(&msgs, body(), op_editor_core::Locale::EnUs);
    assert_eq!(items.len(), 1);
    assert!(items[0].streaming);
    let bubble = items[0].bubble.as_ref().expect("typing bubble present");
    assert!(bubble.typing, "empty in-flight message shows typing dots");
    assert!(bubble.lines.is_empty());
    assert!(
        bubble.rect.size.x < 120.0,
        "TS renders the empty streaming state as a compact w-fit pill"
    );
}

#[test]
fn paint_streaming_empty_assistant_shows_thinking_pill_label() {
    let messages = [ChatMessage::assistant_streaming()];
    let mut backend = TranscriptPaintBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    paint_transcript(
        &mut cx,
        &crate::Theme::dark(),
        body(),
        &messages,
        0,
        op_editor_core::Locale::EnUs,
    );

    assert_eq!(backend.round_rects.len(), 1);
    let (pill, radius) = backend.round_rects[0];
    assert!(pill.size.x < 120.0, "typing pill should not be full width");
    assert!(
        (radius - pill.size.y / 2.0).abs() < 1e-4,
        "TS uses rounded-full for the streaming pill"
    );
    assert!(
        backend.texts.iter().any(|text| text == "Thinking"),
        "TS shows the Thinking label before the animated dots"
    );
    assert_eq!(backend.ovals, 3);
}

#[test]
fn assistant_thinking_collapsed_has_header_but_no_body_lines() {
    let mut m = ChatMessage::assistant("the answer");
    m.thinking = "a long private chain of reasoning".into();
    // Default: thinking_collapsed == true.
    let items = build_transcript(
        std::slice::from_ref(&m),
        body(),
        op_editor_core::Locale::EnUs,
    );
    let t = items[0].thinking.as_ref().expect("thinking block present");
    assert!(t.collapsed);
    assert!(t.lines.is_empty(), "collapsed body carries no lines");
    assert!(t.header.size.y > 0.0, "header is still clickable");
    assert!((t.body.size.y - 0.0).abs() < 1e-4, "collapsed body is flat");
}

#[test]
fn assistant_thinking_expanded_has_wrapped_body_lines() {
    let mut m = ChatMessage::assistant("the answer");
    m.thinking = "a long private chain of reasoning that must wrap \
                  across several lines inside the narrow panel"
        .into();
    m.thinking_collapsed = false;
    let items = build_transcript(
        std::slice::from_ref(&m),
        body(),
        op_editor_core::Locale::EnUs,
    );
    let t = items[0].thinking.as_ref().unwrap();
    assert!(!t.collapsed);
    assert!(t.lines.len() > 1, "long reasoning wraps to many lines");
    assert!(t.body.size.y > 0.0);
}
