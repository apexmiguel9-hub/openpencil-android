//! Unit wrapping and the streaming caret blink cycle.
//!
//! Split out of `ai_chat_transcript_tests.rs` to keep that file under the
//! 800-line cap.

use super::*;

#[test]
fn wrap_units_breaks_ascii_at_word_boundaries() {
    // Budget 10 units — "hello world" (11) must split after the
    // space, not mid-word.
    let lines = wrap_units("hello world", 10);
    assert_eq!(lines, vec!["hello".to_string(), "world".to_string()]);
}

#[test]
fn wrap_units_preserves_explicit_newlines() {
    let lines = wrap_units("a\nb", 80);
    assert_eq!(lines, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn wrap_units_counts_cjk_as_two_units_each() {
    // Five CJK glyphs = 10 units. Budget 6 fits 3 per line.
    let lines = wrap_units("设计登录页", 6);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].chars().count(), 3);
    assert_eq!(lines[1].chars().count(), 2);
}

#[test]
fn wrap_units_hard_breaks_a_token_with_no_spaces() {
    // No space to rewind to — a long token still gets chopped so
    // it cannot overflow the bubble.
    let lines = wrap_units("aaaaaaaa", 3);
    assert!(lines.len() >= 3);
    assert!(lines.iter().all(|l| l.chars().count() <= 3));
}

#[test]
fn wrap_units_empty_text_yields_one_empty_line() {
    assert_eq!(wrap_units("", 40), vec![String::new()]);
}

#[test]
fn streaming_caret_uses_shared_text_input_blink_period() {
    let period = jian_core::text_input::CARET_BLINK_PERIOD_MS;

    assert!(streaming_caret_visible(0));
    assert!(streaming_caret_visible(period - 1));
    assert!(!streaming_caret_visible(period));
    assert!(!streaming_caret_visible(period * 2 - 1));
    assert!(streaming_caret_visible(period * 2));
}
