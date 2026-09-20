//! Tool-call blocks: hidden XML, headers, expanded cards and statuses.
//!
//! Split out of `ai_chat_transcript_tests.rs` to keep that file under the
//! 800-line cap.

use super::*;

#[test]
fn assistant_tool_call_xml_is_hidden_from_answer_bubble() {
    let message = ChatMessage::assistant(
        r#"before
<function_calls><invoke name="batch_design">secret</invoke></function_calls>
<result>{"ok":true}</result>
<!-- APPLIED -->
after"#,
    );

    let items = build_transcript(
        std::slice::from_ref(&message),
        body(),
        op_editor_core::Locale::EnUs,
    );
    let text = items[0].bubble.as_ref().unwrap().lines.join("\n");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("function_calls"));
    assert!(!text.contains("invoke"));
    assert!(!text.contains("secret"));
    assert!(!text.contains("APPLIED"));
}

#[test]
fn hidden_completed_assistant_action_shows_completion_placeholder() {
    let message = ChatMessage::assistant(
        r#"<function_calls><invoke name="batch_design">secret</invoke></function_calls>
<result>{"ok":true}</result>
<!-- APPLIED -->"#,
    );

    let items = build_transcript(
        std::slice::from_ref(&message),
        body(),
        op_editor_core::Locale::EnUs,
    );
    let text = items[0].bubble.as_ref().unwrap().lines.join("\n");

    assert_eq!(text, "(Automated action completed)");
}

#[test]
fn streaming_unclosed_invoke_is_hidden_from_answer_bubble() {
    let mut message = ChatMessage::assistant_streaming();
    message.content = r#"visible
<invoke name="batch_design"><parameter name="dsl">internal"#
        .into();

    let items = build_transcript(
        std::slice::from_ref(&message),
        body(),
        op_editor_core::Locale::EnUs,
    );
    let text = items[0].bubble.as_ref().unwrap().lines.join("\n");

    assert!(text.contains("visible"));
    assert!(!text.contains("invoke"));
    assert!(!text.contains("parameter"));
    assert!(!text.contains("internal"));
}

#[test]
fn tool_calls_block_header_label_counts_the_calls() {
    let mut m = ChatMessage::assistant("done");
    m.tool_calls = vec![
        ChatToolCall {
            name: "insert_node".into(),
            args: "{}".into(),
            content_offset: None,
        },
        ChatToolCall {
            name: "set_fill_hex".into(),
            args: "{}".into(),
            content_offset: None,
        },
    ];
    let items = build_transcript(
        std::slice::from_ref(&m),
        body(),
        op_editor_core::Locale::EnUs,
    );
    let t = items[0].tools.as_ref().expect("tools block present");
    // The header label substitutes the call count into the
    // `ai.toolCalls` template's `{{count}}` placeholder.
    let expected =
        op_i18n::translate(op_editor_core::Locale::EnUs, "ai.toolCalls").replace("{{count}}", "2");
    assert_eq!(t.label, expected, "header label counts the calls");
}

#[test]
fn expanded_tool_card_surfaces_status_source_and_result() {
    let mut m = ChatMessage::assistant("done");
    m.tools_collapsed = false;
    m.tool_calls = vec![ChatToolCall {
        name: "batch_design".into(),
        args: r#"{"source":"designer-1","status":"error","args":{"dsl":"I(\"root\",{})"},"result":{"success":false,"error":"node not found"}}"#.into(),
        content_offset: None,
    }];

    let items = build_transcript(
        std::slice::from_ref(&m),
        body(),
        op_editor_core::Locale::EnUs,
    );
    let t = items[0].tools.as_ref().expect("tools block present");

    assert!(
        t.lines.iter().any(|line| line == "  Source: designer-1"),
        "tool card should expose the originating agent/source"
    );
    assert!(
        t.lines.iter().any(|line| line == "  Status: error"),
        "tool card should expose the tool status"
    );
    assert!(
        t.lines
            .iter()
            .any(|line| line == "  Result: node not found"),
        "tool card should expose failure result text"
    );
    assert!(
        t.lines
            .iter()
            .any(|line| line.contains(r#""dsl":"I(\"root\",{})""#)),
        "tool card should still show the actual call arguments"
    );
}

#[test]
fn streaming_tool_card_falls_back_to_running_status() {
    let mut m = ChatMessage::assistant_streaming();
    m.tools_collapsed = false;
    m.tool_calls = vec![ChatToolCall {
        name: "snapshot_layout".into(),
        args: "{}".into(),
        content_offset: None,
    }];

    let items = build_transcript(
        std::slice::from_ref(&m),
        body(),
        op_editor_core::Locale::EnUs,
    );
    let t = items[0].tools.as_ref().expect("tools block present");

    assert!(
        t.lines.iter().any(|line| line == "  Status: running"),
        "in-flight tool card should not look like a completed raw JSON dump"
    );
}

#[test]
fn paint_expanded_tool_calls_as_individual_cards_like_ts() {
    let mut m = ChatMessage::assistant("done");
    m.tools_collapsed = false;
    m.tool_calls = vec![
        ChatToolCall {
            name: "batch_design".into(),
            args: r#"{"args":{"dsl":"I(\"root\",{})"},"status":"running"}"#.into(),
            content_offset: None,
        },
        ChatToolCall {
            name: "delete_node".into(),
            args: r#"{"args":{"id":"old-node"},"result":{"success":false,"error":"missing"}}"#
                .into(),
            content_offset: None,
        },
    ];
    let mut backend = TranscriptPaintBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    paint_transcript(
        &mut cx,
        &crate::Theme::dark(),
        body(),
        &[m],
        0,
        op_editor_core::Locale::EnUs,
    );

    assert!(
        backend.round_rects.len() >= 3,
        "TS renders each tool call as its own bordered card, not one text body"
    );
}

#[test]
fn mixed_tool_calls_expand_only_write_level_card_bodies_like_ts() {
    let mut m = ChatMessage::assistant("done");
    m.tools_collapsed = false;
    m.tool_calls = vec![
        ChatToolCall {
            name: "snapshot_layout".into(),
            args: r#"{"args":{"pageId":"page-1"}}"#.into(),
            content_offset: None,
        },
        ChatToolCall {
            name: "batch_design".into(),
            args: r#"{"args":{"dsl":"I(\"root\",{})"}}"#.into(),
            content_offset: None,
        },
    ];

    let items = build_transcript(
        std::slice::from_ref(&m),
        body(),
        op_editor_core::Locale::EnUs,
    );
    let tools = items[0].tools.as_ref().expect("tools block present");

    assert_eq!(tools.cards.len(), 2);
    assert!(
        tools.cards[0].body.size.y == 0.0,
        "TS keeps read tool cards collapsed by default"
    );
    assert!(
        tools.cards[1].body.size.y > 0.0,
        "TS opens modify tool cards by default"
    );
}
