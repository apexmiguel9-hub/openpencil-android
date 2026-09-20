//! Keyboard editing for the Prompt Center's focused text field.
//!
//! The native and web hosts only route platform key events here; all
//! selection, UTF-8 caret, and filter-reset semantics stay shared.

use crate::editor_ui_state::PromptCenterFocus;
use crate::EditorState;

/// Insert one printable character into the focused Prompt Center input.
///
/// `None` means the panel is closed. `Some(changed)` means the panel owns
/// the key, with `changed` indicating whether a repaint is needed.
pub fn text(state: &mut EditorState, c: char, now_ms: u64) -> Option<bool> {
    if !state.editor_ui.prompt_center.open {
        return None;
    }
    if c.is_control() {
        return Some(false);
    }
    let mut encoded = [0_u8; 4];
    let prompt_center = &mut state.editor_ui.prompt_center;
    prompt_center
        .focused_input_mut()
        .insert_str(c.encode_utf8(&mut encoded), now_ms);
    reset_filter_feedback(prompt_center);
    Some(true)
}

/// Delete the previous character in the focused Prompt Center input.
pub fn backspace(state: &mut EditorState, now_ms: u64) -> Option<bool> {
    edit_if_open(state, |input| input.backspace(now_ms))
}

/// Delete the next character in the focused Prompt Center input.
pub fn delete_forward(state: &mut EditorState, now_ms: u64) -> Option<bool> {
    edit_if_open(state, |input| input.delete_forward(now_ms))
}

/// Move the focused Prompt Center caret left or right.
pub fn move_caret(state: &mut EditorState, forward: bool, extend: bool, now_ms: u64) -> bool {
    if !state.editor_ui.prompt_center.open {
        return false;
    }
    let input = state.editor_ui.prompt_center.focused_input_mut();
    if forward {
        input.move_right(extend, now_ms);
    } else {
        input.move_left(extend, now_ms);
    }
    true
}

/// Select all text in the focused Prompt Center field.
pub fn select_all(state: &mut EditorState, now_ms: u64) -> bool {
    if !state.editor_ui.prompt_center.open {
        return false;
    }
    let input = state.editor_ui.prompt_center.focused_input_mut();
    input.select_all();
    input.touch(now_ms);
    true
}

/// Return selected text from the focused Prompt Center field.
pub fn selected_text(state: &EditorState) -> Option<&str> {
    if !state.editor_ui.prompt_center.open {
        return None;
    }
    let prompt_center = &state.editor_ui.prompt_center;
    let input = match prompt_center.focus {
        PromptCenterFocus::Search => &prompt_center.search,
        PromptCenterFocus::SaveTitle => &prompt_center.save_title,
    };
    let (start, end) = input.highlight_range()?;
    input.text().get(start..end)
}

fn edit_if_open(
    state: &mut EditorState,
    edit: impl FnOnce(&mut jian_core::text_input::TextInputState),
) -> Option<bool> {
    if !state.editor_ui.prompt_center.open {
        return None;
    }
    let prompt_center = &mut state.editor_ui.prompt_center;
    let before = prompt_center.focused_input_mut().text().to_owned();
    edit(prompt_center.focused_input_mut());
    let changed = prompt_center.focused_input_mut().text() != before;
    if changed {
        reset_filter_feedback(prompt_center);
    }
    Some(changed)
}

fn reset_filter_feedback(prompt_center: &mut crate::PromptCenterState) {
    prompt_center.hover = None;
    if prompt_center.focus == PromptCenterFocus::Search {
        prompt_center.scroll.offset = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_search_and_save_title_without_crossing_fields() {
        let mut state = EditorState::new();
        assert_eq!(text(&mut state, 'x', 1), None);

        state.editor_ui.open_prompt_center(2);
        assert_eq!(text(&mut state, '旅', 3), Some(true));
        assert_eq!(state.editor_ui.prompt_center.search.text(), "旅");
        assert_eq!(backspace(&mut state, 4), Some(true));
        assert!(state.editor_ui.prompt_center.search.text().is_empty());

        state.editor_ui.prompt_center.focus = PromptCenterFocus::SaveTitle;
        assert_eq!(text(&mut state, 'A', 5), Some(true));
        assert_eq!(state.editor_ui.prompt_center.save_title.text(), "A");
        assert!(state.editor_ui.prompt_center.search.text().is_empty());
    }

    #[test]
    fn empty_backspace_is_owned_without_change() {
        let mut state = EditorState::new();
        state.editor_ui.open_prompt_center(0);
        assert_eq!(backspace(&mut state, 1), Some(false));
    }

    #[test]
    fn shift_arrows_extend_utf8_selection_in_both_directions() {
        let mut state = EditorState::new();
        state.editor_ui.open_prompt_center(0);
        state.editor_ui.prompt_center.search.set_text("a旅b");
        state.editor_ui.prompt_center.search.set_caret(1, 1);

        assert!(move_caret(&mut state, true, true, 2));
        assert_eq!(selected_text(&state), Some("旅"));

        assert!(move_caret(&mut state, false, true, 3));
        assert_eq!(selected_text(&state), None);
        assert!(move_caret(&mut state, false, true, 4));
        assert_eq!(selected_text(&state), Some("a"));
    }
}
