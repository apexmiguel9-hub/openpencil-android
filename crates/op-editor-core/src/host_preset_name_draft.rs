//! The variables preset dropdown's "save as name" input (#20).
//!
//! Unlike every other chrome text field this one has **no
//! `TextInputState`**: the ThemePresetMenu widget paints the flat
//! `ui.property_input_draft` / `ui.property_caret_pos` pair directly,
//! so its editing primitives are plain-`String` splice operations and
//! live here rather than in `host_keyboard_transitions`.
//!
//! Both hosts route the same four keystrokes into this module — typed
//! char, Backspace, forward-Delete, Left/Right — so the caret rules
//! (UTF-8 boundary clamping, select-all replace) can never drift.
//!
//! `Option<bool>` mirrors the keyboard-router convention: `None` means
//! "the preset input is not focused, fall through to the next arm".

use crate::state::EditorState;

/// Insert `c` at the caret (replacing a select-all draft). `false`
/// when the preset input isn't focused or `c` is a control character,
/// so the caller falls through.
pub fn preset_name_text(state: &mut EditorState, c: char, now_ms: u64) -> bool {
    if !state.editor_ui.preset_name_input_active() || c.is_control() {
        return false;
    }
    let replace_selection = state.ui.property_draft_select_all;
    let pos = if replace_selection {
        0
    } else {
        text_boundary_at_or_before(&state.ui.property_input_draft, state.ui.property_caret_pos)
    };
    if replace_selection {
        state.ui.property_input_draft.clear();
        state.ui.property_caret_pos = 0;
    }
    state.ui.property_draft_select_all = false;
    state.ui.property_input_draft.insert(pos, c);
    state.ui.property_caret_pos = pos + c.len_utf8();
    state.ui.property_caret_anchor_ms = now_ms;
    true
}

/// Backspace: clear a select-all draft in one press, else delete the
/// character before the caret. `Some(false)` at the draft head — the
/// input still owns the key, so the host must not fall through to a
/// canvas delete.
pub fn preset_name_backspace(state: &mut EditorState, now_ms: u64) -> Option<bool> {
    if !state.editor_ui.preset_name_input_active() {
        return None;
    }
    if state.ui.property_draft_select_all {
        state.ui.property_input_draft.clear();
        state.ui.property_caret_pos = 0;
        state.ui.property_draft_select_all = false;
        state.ui.property_caret_anchor_ms = now_ms;
        return Some(true);
    }
    let pos =
        text_boundary_at_or_before(&state.ui.property_input_draft, state.ui.property_caret_pos);
    if pos == 0 {
        return Some(false);
    }
    let prev = previous_text_boundary(&state.ui.property_input_draft, pos);
    state.ui.property_input_draft.drain(prev..pos);
    state.ui.property_caret_pos = prev;
    state.ui.property_caret_anchor_ms = now_ms;
    Some(true)
}

/// Forward-delete: clear a select-all draft, else delete the
/// character after the caret.
pub fn preset_name_delete_forward(state: &mut EditorState, now_ms: u64) -> Option<bool> {
    if !state.editor_ui.preset_name_input_active() {
        return None;
    }
    if state.ui.property_draft_select_all {
        state.ui.property_input_draft.clear();
        state.ui.property_caret_pos = 0;
        state.ui.property_draft_select_all = false;
        state.ui.property_caret_anchor_ms = now_ms;
        return Some(true);
    }
    let pos =
        text_boundary_at_or_before(&state.ui.property_input_draft, state.ui.property_caret_pos);
    if pos >= state.ui.property_input_draft.len() {
        return Some(false);
    }
    let next = next_text_boundary(&state.ui.property_input_draft, pos);
    state.ui.property_input_draft.drain(pos..next);
    state.ui.property_caret_pos = pos;
    state.ui.property_caret_anchor_ms = now_ms;
    Some(true)
}

/// Left / Right arrow. `Some(true)` whenever the input is focused —
/// even at a text boundary where the caret cannot move — because an
/// arrow over a focused draft must never fall through to nudging the
/// selected node. The inner `bool` reports whether the caret actually
/// moved, so the host only repaints when something changed.
pub fn preset_name_caret_move(state: &mut EditorState, forward: bool, now_ms: u64) -> Option<bool> {
    if !state.editor_ui.preset_name_input_active() {
        return None;
    }
    let pos =
        text_boundary_at_or_before(&state.ui.property_input_draft, state.ui.property_caret_pos);
    state.ui.property_draft_select_all = false;
    let next = if forward {
        next_text_boundary(&state.ui.property_input_draft, pos)
    } else {
        previous_text_boundary(&state.ui.property_input_draft, pos)
    };
    if next == state.ui.property_caret_pos {
        return Some(false);
    }
    state.ui.property_caret_pos = next;
    state.ui.property_caret_anchor_ms = now_ms;
    Some(true)
}

/// Clamp `pos` down to the nearest UTF-8 boundary at or before it.
/// The draft and the caret are stored separately, so a stale caret can
/// point mid-codepoint after the draft is replaced.
fn text_boundary_at_or_before(value: &str, pos: usize) -> usize {
    let mut clipped = pos.min(value.len());
    while clipped > 0 && !value.is_char_boundary(clipped) {
        clipped -= 1;
    }
    clipped
}

fn previous_text_boundary(value: &str, pos: usize) -> usize {
    let pos = text_boundary_at_or_before(value, pos);
    value[..pos]
        .char_indices()
        .last()
        .map(|(idx, _)| idx)
        .unwrap_or(0)
}

fn next_text_boundary(value: &str, pos: usize) -> usize {
    let pos = text_boundary_at_or_before(value, pos);
    if pos >= value.len() {
        return value.len();
    }
    pos + value[pos..].chars().next().map(char::len_utf8).unwrap_or(0)
}
