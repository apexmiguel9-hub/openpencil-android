//! Shared Prompt Center press transitions for native and web hosts.

use op_editor_core::{ButtonPressTarget, EditorState, PromptCenterFocus};

use crate::widgets::{PromptCenterHit, PromptCenterPanel};
use crate::{Point2D, Rect};

/// Route one pointer press to the non-modal Prompt Center.
///
/// `None` means the press was outside the panel and must fall through.
/// `Some(changed)` means the panel swallowed it; hosts repaint when
/// `changed` is true. `created_at` is host-provided epoch milliseconds
/// and is only consumed by the save action.
pub fn press_prompt_center(
    state: &mut EditorState,
    panel_rect: Rect,
    point: Point2D,
    now_ms: u64,
    created_at: u64,
) -> Option<bool> {
    let panel = PromptCenterPanel::for_editor(state)?;
    let hover = panel.hover_at(panel_rect, point);
    let hit = panel.hit_test(panel_rect, point)?;
    let pressed = hover.map(ButtonPressTarget::PromptCenter);
    let pressed_changed = state.editor_ui.pressed_button != pressed;
    state.editor_ui.pressed_button = pressed;

    let changed = match hit {
        PromptCenterHit::Close => state.editor_ui.close_prompt_center(),
        PromptCenterHit::OpenSave => {
            let prompt_center = &mut state.editor_ui.prompt_center;
            if !prompt_center.custom_store_writable || state.chat.input.text().trim().is_empty() {
                false
            } else {
                prompt_center.save_open = true;
                prompt_center.focus = PromptCenterFocus::SaveTitle;
                prompt_center.save_title.set_text("");
                prompt_center.save_title.touch(now_ms);
                prompt_center.scroll.offset = 0.0;
                true
            }
        }
        PromptCenterHit::FocusSearch(offset) => {
            let prompt_center = &mut state.editor_ui.prompt_center;
            let changed = prompt_center.focus != PromptCenterFocus::Search
                || prompt_center.search.caret() != offset;
            prompt_center.focus = PromptCenterFocus::Search;
            prompt_center.search.set_caret(offset, now_ms);
            changed
        }
        PromptCenterHit::SelectFilter(filter) => {
            let prompt_center = &mut state.editor_ui.prompt_center;
            let changed = prompt_center.filter != filter
                || prompt_center.scroll.offset != 0.0
                || prompt_center.hover.is_some();
            prompt_center.filter = filter;
            prompt_center.scroll.offset = 0.0;
            prompt_center.hover = None;
            changed
        }
        PromptCenterHit::FocusSaveTitle(offset) => {
            let prompt_center = &mut state.editor_ui.prompt_center;
            let changed = prompt_center.focus != PromptCenterFocus::SaveTitle
                || prompt_center.save_title.caret() != offset;
            prompt_center.focus = PromptCenterFocus::SaveTitle;
            prompt_center.save_title.set_caret(offset, now_ms);
            changed
        }
        PromptCenterHit::SelectSaveCategory(category) => {
            let prompt_center = &mut state.editor_ui.prompt_center;
            let changed = prompt_center.save_category != category;
            prompt_center.save_category = category;
            changed
        }
        PromptCenterHit::SaveCurrent => {
            let title = state.editor_ui.prompt_center.save_title.text().to_owned();
            let body = state.chat.input.text().to_owned();
            let category = state.editor_ui.prompt_center.save_category;
            state
                .editor_ui
                .prompt_center
                .add_custom_prompt(title, body, category, created_at)
                .is_some()
        }
        PromptCenterHit::CancelSave => {
            let prompt_center = &mut state.editor_ui.prompt_center;
            let changed = prompt_center.save_open
                || !prompt_center.save_title.text().is_empty()
                || prompt_center.focus != PromptCenterFocus::Search;
            prompt_center.save_open = false;
            prompt_center.save_title.set_text("");
            prompt_center.focus = PromptCenterFocus::Search;
            prompt_center.search.touch(now_ms);
            changed
        }
        PromptCenterHit::SelectPrompt { body, .. } => {
            state.chat.set_input_text(body);
            state.chat.focus_input_at_end(now_ms);
            state.chat.transcript_selection = None;
            state.editor_ui.close_prompt_center();
            true
        }
        PromptCenterHit::DeleteCustom(id) => {
            state.editor_ui.prompt_center.delete_custom_prompt(&id)
        }
        PromptCenterHit::Inside => false,
    };
    Some(changed || pressed_changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_editor_core::prompt_center_catalog::PromptCategory;
    use op_editor_core::PromptFilter;

    fn open_state() -> EditorState {
        let mut state = EditorState::new();
        state.editor_ui.open_prompt_center(1);
        state
    }

    #[test]
    fn outside_is_non_modal_and_blank_chrome_is_swallowed() {
        let mut state = open_state();
        let rect = Rect::xywh(100.0, 100.0, 720.0, 520.0);
        assert_eq!(
            press_prompt_center(&mut state, rect, Point2D::new(10.0, 10.0), 2, 3),
            None
        );
        assert_eq!(
            press_prompt_center(&mut state, rect, Point2D::new(110.0, 110.0), 2, 3),
            Some(false)
        );
        assert!(state.editor_ui.prompt_center.open);
    }

    #[test]
    fn selecting_card_fills_without_sending() {
        let mut state = open_state();
        state.editor_ui.locale = op_editor_core::Locale::ZhCn;
        let rect = Rect::xywh(0.0, 0.0, 720.0, 520.0);
        let card = PromptCenterPanel::for_editor(&state)
            .unwrap()
            .card_rects(rect)[0]
            .1;
        let point = Point2D::new(card.origin.x + 8.0, card.origin.y + 8.0);
        let messages = state.chat.messages.len();

        assert_eq!(
            press_prompt_center(&mut state, rect, point, 9, 10),
            Some(true)
        );
        assert!(!state.editor_ui.prompt_center.open);
        assert!(state.chat.focused);
        assert_eq!(state.chat.input_caret(), state.chat.input.text().len());
        assert!(state.chat.pending_send.is_none());
        assert_eq!(state.chat.messages.len(), messages);
        assert!(!state.chat.input.text().is_empty());
    }

    #[test]
    fn search_click_places_caret_from_pointer_x() {
        let mut state = open_state();
        state.editor_ui.prompt_center.search.set_text("a旅b");
        let rect = Rect::xywh(0.0, 0.0, 720.0, 520.0);
        let search = PromptCenterPanel::search_rect(rect);
        let y = search.origin.y + search.size.y / 2.0;

        assert_eq!(
            press_prompt_center(
                &mut state,
                rect,
                Point2D::new(search.origin.x + 32.0, y),
                4,
                5
            ),
            Some(true)
        );
        assert_eq!(state.editor_ui.prompt_center.search.caret(), 0);

        assert_eq!(
            press_prompt_center(
                &mut state,
                rect,
                Point2D::new(search.origin.x + search.size.x - 2.0, y),
                6,
                7
            ),
            Some(true)
        );
        assert_eq!(
            state.editor_ui.prompt_center.search.caret(),
            state.editor_ui.prompt_center.search.text().len()
        );
    }

    #[test]
    fn save_and_delete_mark_custom_store_dirty() {
        let mut state = open_state();
        state
            .editor_ui
            .prompt_center
            .install_custom_prompts(Vec::new(), true);
        state.chat.set_input_text("Reusable body");
        state.editor_ui.prompt_center.save_open = true;
        state
            .editor_ui
            .prompt_center
            .save_title
            .set_text("My prompt");
        state.editor_ui.prompt_center.save_category = PromptCategory::Modify;
        let rect = Rect::xywh(0.0, 0.0, 720.0, 520.0);
        let save = PromptCenterPanel::save_button_rect(rect);

        assert_eq!(
            press_prompt_center(
                &mut state,
                rect,
                Point2D::new(save.origin.x + 2.0, save.origin.y + 2.0),
                4,
                123
            ),
            Some(true)
        );
        assert_eq!(state.editor_ui.prompt_center.filter, PromptFilter::Custom);
        assert_eq!(state.editor_ui.prompt_center.custom_prompts.len(), 1);
        assert!(state.editor_ui.prompt_center.custom_store_dirty);
    }
}
