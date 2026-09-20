//! Tests for [`crate::js_text`] — the JS-semantics text primitives.

use crate::js_text::{is_js_space, js_trim, truncate_utf16};

#[test]
fn js_space_matches_the_js_regex_class() {
    for c in [
        ' ', '\t', '\n', '\r', '\u{0b}', '\u{0c}', '\u{a0}', '\u{feff}', '\u{3000}',
    ] {
        assert!(is_js_space(c), "expected {c:?} to be JS whitespace");
    }
    // `U+0085` (NEL) has the Unicode `White_Space` property but is NOT in JS
    // `\s`, so `Number("\u{85}12")` is NaN and `" \u{85}x".trim()` keeps it.
    for c in ['a', '.', '-', '\u{200b}', '\u{85}'] {
        assert!(!is_js_space(c), "expected {c:?} not to be JS whitespace");
    }
}

#[test]
fn trim_keeps_the_next_line_control_that_js_keeps() {
    assert_eq!(js_trim("\u{85}12"), "\u{85}12");
    assert_eq!(js_trim(" \u{85} hi \u{85} "), "\u{85} hi \u{85}");
}

#[test]
fn trim_strips_the_byte_order_mark_like_js_does() {
    assert_eq!(js_trim("\u{feff} hi \u{feff}"), "hi");
    assert_eq!(js_trim("  "), "");
    assert_eq!(js_trim("plain"), "plain");
}

#[test]
fn truncate_never_splits_a_surrogate_pair() {
    assert_eq!(truncate_utf16("abcdef", 3), "abc");
    assert_eq!(truncate_utf16("abc", 10), "abc");
    // Limit 2 lands mid-pair for the emoji, so the emoji is dropped whole.
    assert_eq!(truncate_utf16("a😀b", 2), "a");
    assert_eq!(truncate_utf16("a😀b", 3), "a😀");
    assert_eq!(truncate_utf16("abc", 0), "");
}
