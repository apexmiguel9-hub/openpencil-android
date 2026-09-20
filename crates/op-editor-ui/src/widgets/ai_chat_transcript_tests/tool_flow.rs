//! Interleaved narration ↔ tool-chip flow driven by content offsets.
//!
//! Split out of `ai_chat_transcript_tests.rs` to keep that file under the
//! 800-line cap.

use super::*;

// ── interleaved narration ↔ tool-chip flow ──

fn loop_call(name: &str, offset: Option<u32>) -> ChatToolCall {
    ChatToolCall {
        name: name.into(),
        args: "{}".into(),
        content_offset: offset,
    }
}

fn loop_message() -> ChatMessage {
    // "First I plan." → [batch_design ×2] → "Then I check." → [get_screenshot]
    // → "All done."
    let mut msg = ChatMessage::assistant("First I plan. Then I check. All done.");
    msg.tool_calls = vec![
        loop_call("batch_design", Some(14)),
        loop_call("batch_design", Some(14)),
        loop_call("get_screenshot", Some(27)),
    ];
    msg
}

#[test]
fn offset_stamped_calls_interleave_prose_and_headerless_panels() {
    let (item, _) = build_item(
        &loop_message(),
        0,
        0.0,
        body(),
        op_editor_core::Locale::EnUs,
    );
    assert!(item.tools.is_none(), "grouped panel replaced by the flow");
    assert!(
        item.bubble.is_none(),
        "monolithic bubble replaced by the flow"
    );
    assert_eq!(item.flow_bubbles.len(), 3, "three prose segments");
    assert_eq!(item.flow_panels.len(), 2, "two call groups");
    assert_eq!(
        item.flow_panels[0].cards.len(),
        2,
        "same-offset calls stack"
    );
    // Document order: prose, panel, prose, panel, prose.
    let b = &item.flow_bubbles;
    let p = &item.flow_panels;
    assert!(b[0].rect.origin.y < p[0].cards[0].rect.origin.y);
    assert!(p[0].cards[1].rect.origin.y < b[1].rect.origin.y);
    assert!(b[1].rect.origin.y < p[1].cards[0].rect.origin.y);
    assert!(p[1].cards[0].rect.origin.y < b[2].rect.origin.y);
    // Headerless: nothing for the group-toggle hit to land on.
    assert_eq!(p[0].header.size.y, 0.0);
    // Cards carry their ORIGINAL indices for the expand override.
    assert_eq!(p[0].cards[0].index, 0);
    assert_eq!(p[0].cards[1].index, 1);
    assert_eq!(p[1].cards[0].index, 2);
}

#[test]
fn calls_without_offsets_keep_the_grouped_panel() {
    let mut msg = ChatMessage::assistant("plain chat answer");
    msg.tool_calls = vec![loop_call("get_node", None)];
    let (item, _) = build_item(&msg, 0, 0.0, body(), op_editor_core::Locale::EnUs);
    assert!(item.tools.is_some(), "no offsets → classic grouped panel");
    assert!(item.flow_panels.is_empty());
    assert!(item.bubble.is_some());
}

#[test]
fn flow_card_hit_returns_original_tool_index() {
    let msgs = [loop_message()];
    let canonical = crate::widgets::ai_chat_transcript_cache::unowned_for_tests(
        &msgs,
        body(),
        op_editor_core::Locale::EnUs,
    );
    let item = &canonical.items[0];
    let card = &item.flow_panels[1].cards[0];
    let hit = transcript_hit(
        &canonical,
        body(),
        card.header.origin.x + 4.0,
        card.header.origin.y + card.header.size.y / 2.0,
        0.0,
    );
    assert_eq!(
        hit,
        Some(TranscriptHit::SetToolCallCardExpanded(0, 2, !card.expanded)),
        "third call toggles override slot 2 even though it is its panel's first card"
    );
}

#[test]
fn trailing_calls_at_content_end_leave_no_empty_prose_segment() {
    let mut msg = ChatMessage::assistant("Building the header now.");
    let end = msg.content.len() as u32;
    msg.tool_calls = vec![loop_call("batch_design", Some(end))];
    msg.streaming = true;
    let (item, _) = build_item(&msg, 0, 0.0, body(), op_editor_core::Locale::EnUs);
    assert_eq!(item.flow_bubbles.len(), 1);
    assert_eq!(item.flow_panels.len(), 1);
    assert!(item.flow_bubbles[0].rect.origin.y < item.flow_panels[0].cards[0].rect.origin.y);
}

#[test]
fn prose_sits_equally_far_above_and_below_a_tool_chip() {
    let (item, _) = build_item(
        &loop_message(),
        0,
        0.0,
        body(),
        op_editor_core::Locale::EnUs,
    );
    let b = &item.flow_bubbles;
    let p = &item.flow_panels;
    let first_card = &p[0].cards[0];
    let last_card = p[0].cards.last().unwrap();
    let above = first_card.rect.origin.y - (b[0].rect.origin.y + b[0].rect.size.y);
    let below = b[1].rect.origin.y - (last_card.rect.origin.y + last_card.rect.size.y);
    assert!(
        (above - below).abs() < 0.01,
        "a chip must belong to the story on BOTH sides equally: {above} above vs {below} below"
    );
}
