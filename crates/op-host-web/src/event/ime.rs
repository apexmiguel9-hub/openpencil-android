//! Pure W3C `CompositionEvent` → `jian_core::gesture::ImeEvent` mapping.
//!
//! Spec §2.4 invariant: `ImeEvent.text` is UTF-8 (Rust `String`) and
//! `ImeEvent.kind == CompositionUpdate { selection }`'s selection
//! Range is in UTF-8 byte offsets. Browsers expose the in-flight
//! preedit as `CompositionEvent.data` (a JS UTF-16 string) and the
//! IME-highlighted segment via `CompositionEvent.getTargetRanges()`
//! (UTF-16 code-unit offsets where supported). This module is the
//! shim that converts both.
//!
//! Pure: no DOM access. Phase C2 reads `CompositionEvent.data` /
//! `getTargetRanges()` and calls into here with already-converted
//! Rust strings + UTF-16 code-unit ranges.

use op_editor_ui::{ImeEvent, ImeKind};
use std::ops::Range;

/// Convert a UTF-16 code-unit selection range to UTF-8 byte offsets
/// within `text`. Walks `text.char_indices()` accumulating
/// `len_utf16()` so we know the UTF-16 prefix length at each UTF-8
/// boundary; sets `start` / `end` as soon as the running prefix
/// reaches `sel.start` / `sel.end`.
///
/// Out-of-range selection bounds clamp to `text.len()` (the byte
/// length of the full UTF-8 string). The caller may treat that as a
/// "select all the way to the end" hint or as a sentinel value.
pub fn utf16_selection_to_utf8(
    text: &str,
    sel_utf16: Option<Range<usize>>,
) -> Option<Range<usize>> {
    let sel = sel_utf16?;
    if sel.end < sel.start {
        // Mis-ordered range — treat as no selection rather than
        // panicking on the caller's behalf.
        return None;
    }
    let mut utf16_pos = 0usize;
    let mut start: Option<usize> = None;
    let mut end: Option<usize> = None;
    for (byte_idx, ch) in text.char_indices() {
        if start.is_none() && utf16_pos >= sel.start {
            start = Some(byte_idx);
        }
        if end.is_none() && utf16_pos >= sel.end {
            end = Some(byte_idx);
            break;
        }
        utf16_pos += ch.len_utf16();
    }
    let s = start.unwrap_or(text.len());
    let e = end.unwrap_or(text.len());
    Some(s..e)
}

/// `compositionstart`: text is empty per spec §2.4.
pub fn composition_start() -> ImeEvent {
    ImeEvent {
        kind: ImeKind::CompositionStart,
        text: String::new(),
    }
}

/// `compositionupdate`: `text` is the preedit so far (UTF-8 already
/// converted by the caller). `sel_utf16` is the IME-highlighted
/// segment in UTF-16 code units (whatever `getTargetRanges()` gave
/// us); we remap to UTF-8 byte offsets so widget code never has to
/// know about UTF-16.
pub fn composition_update(text_utf8: String, sel_utf16: Option<Range<usize>>) -> ImeEvent {
    let selection = utf16_selection_to_utf8(&text_utf8, sel_utf16);
    ImeEvent {
        kind: ImeKind::CompositionUpdate { selection },
        text: text_utf8,
    }
}

/// `compositionend`: `text` is the final commit string (UTF-8). No
/// selection — the IME is finished.
pub fn composition_end(text_utf8: String) -> ImeEvent {
    ImeEvent {
        kind: ImeKind::CompositionEnd,
        text: text_utf8,
    }
}

/// Text a `beforeinput` event on the hidden IME capture input should
/// deliver to the editor, or `None` when this path does not own the
/// event.
///
/// Composition events only cover IME output that opens a candidate
/// session. CJK punctuation (《 》 【 】 —— ……) is resolved the instant
/// the key is pressed, so no composition ever starts and
/// `compositionend` never fires — the character reaches the page only
/// as a plain `insertText` on the focused element.
///
/// Everything else is left alone: an in-flight composition is owned by
/// `compositionend`, and non-text input types (Backspace, Enter, paste)
/// keep their existing handlers.
pub fn beforeinput_text(
    input_type: &str,
    data: Option<&str>,
    is_composing: bool,
) -> Option<String> {
    if is_composing || input_type != "insertText" {
        return None;
    }
    let data = data?;
    (!data.is_empty()).then(|| data.to_string())
}

/// Whether the window `keydown` handler should type printable text.
///
/// While the hidden IME capture input actually owns DOM focus, every
/// text-producing key also raises `beforeinput` on it, and that is the
/// authoritative source: it carries what the IME produced (《) rather
/// than the raw key the layout would have given (`<`). Typing from both
/// paths would double every character, so `keydown` yields.
///
/// The gate reads REAL DOM focus, not the intent to focus, so a failed
/// `focus()` degrades to the pre-IME `keydown` behaviour instead of
/// silently swallowing every keystroke.
pub fn keydown_should_insert_text(ime_input_owns_focus: bool) -> bool {
    !ime_input_owns_focus
}

#[cfg(test)]
mod beforeinput_tests {
    use super::{beforeinput_text, keydown_should_insert_text};

    /// The reported bug's web twin: 《 opens no composition, so it
    /// arrives only as a plain `insertText`.
    #[test]
    fn plain_insert_text_carries_cjk_punctuation() {
        assert_eq!(
            beforeinput_text("insertText", Some("《"), false).as_deref(),
            Some("《")
        );
        for piece in ["》", "【", "】", "——", "……", "，", "。"] {
            assert_eq!(
                beforeinput_text("insertText", Some(piece), false).as_deref(),
                Some(piece),
                "{piece:?} must be delivered by the beforeinput path"
            );
        }
    }

    /// An in-flight composition is owned by `compositionend`; taking it
    /// here too would double every composed word.
    #[test]
    fn composing_input_is_left_to_composition_end() {
        assert_eq!(beforeinput_text("insertText", Some("你好"), true), None);
        assert_eq!(
            beforeinput_text("insertCompositionText", Some("ni"), true),
            None
        );
    }

    /// Non-text input types keep their existing handlers.
    #[test]
    fn other_input_types_are_not_ours() {
        for input_type in [
            "deleteContentBackward",
            "deleteContentForward",
            "insertLineBreak",
            "insertParagraph",
            "insertFromPaste",
            "insertFromDrop",
            "historyUndo",
        ] {
            assert_eq!(
                beforeinput_text(input_type, Some("x"), false),
                None,
                "{input_type:?} must not be typed by the IME path"
            );
        }
    }

    #[test]
    fn absent_or_empty_data_delivers_nothing() {
        assert_eq!(beforeinput_text("insertText", None, false), None);
        assert_eq!(beforeinput_text("insertText", Some(""), false), None);
    }

    /// Ordinary Latin typing also arrives as `insertText` — it is
    /// delivered here because `keydown` yields while the hidden input
    /// owns focus (the two gates are complementary, never both on).
    #[test]
    fn ordinary_typing_is_delivered_once_by_exactly_one_path() {
        assert_eq!(
            beforeinput_text("insertText", Some("a"), false).as_deref(),
            Some("a")
        );
        assert!(!keydown_should_insert_text(true));
        assert!(keydown_should_insert_text(false));
    }
}
