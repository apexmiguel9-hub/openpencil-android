//! Recognising the untouched document `EditorState::starter()` produces.
//!
//! Two features ask the same question for different reasons. Generation asks
//! it to decide whether a request means "build me this" or "add to what I
//! have"; the Asset Center asks it to decide whether bringing a template in
//! should replace the document or append to it. Both need the same answer, so
//! the predicate lives here rather than beside either of them.
//!
//! It is deliberately exact — the starter's id, name, position, size, fill,
//! and emptiness all have to match. A looser test (any single empty frame,
//! say) would treat a frame the user drew and has not filled in yet as
//! disposable, and the whole point of the check is to know that nothing would
//! be lost.

use jian_ops_schema::node::PenNode;
use jian_ops_schema::sizing::SizingBehavior;
use jian_ops_schema::style::PenFill;

use crate::state::EditorState;

/// Whether the active page holds nothing but the pristine starter frame.
pub fn active_page_is_blank_starter(state: &EditorState) -> bool {
    let [only] = state.active_children() else {
        return false;
    };
    is_blank_starter_frame(only)
}

fn is_blank_starter_frame(node: &PenNode) -> bool {
    let PenNode::Frame(frame) = node else {
        return false;
    };
    if frame.base.id != "n10" || frame.base.name.as_deref() != Some("Frame") {
        return false;
    }
    if frame
        .children
        .as_ref()
        .map(|c| !c.is_empty())
        .unwrap_or(false)
    {
        return false;
    }
    if !near(frame.base.x.unwrap_or(0.0), 0.0) || !near(frame.base.y.unwrap_or(0.0), 0.0) {
        return false;
    }
    if !matches_number(&frame.container.width, 1200.0)
        || !matches_number(&frame.container.height, 800.0)
    {
        return false;
    }
    if frame.container.stroke.is_some() {
        return false;
    }
    match frame.container.fill.as_deref() {
        Some([PenFill::Solid(fill)]) => fill.color.eq_ignore_ascii_case("#ffffff"),
        _ => false,
    }
}

fn matches_number(value: &Option<SizingBehavior>, expected: f64) -> bool {
    matches!(value, Some(SizingBehavior::Number(n)) if near(*n, expected))
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.01
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_starter_document_is_recognised_and_an_edited_one_is_not() {
        let mut state = EditorState::starter();
        assert!(active_page_is_blank_starter(&state));

        // Renaming the frame is the smallest edit a user can make to it, and
        // it is enough: the document is no longer the one we shipped.
        if let Some(PenNode::Frame(frame)) = state.active_children_mut().first_mut() {
            frame.base.name = Some("Cover".into());
        }
        assert!(!active_page_is_blank_starter(&state));
    }

    #[test]
    fn an_empty_page_is_not_a_starter() {
        // Nothing on the page is not the same as the starter being there:
        // generation clears the page while a turn runs, and treating that as
        // "safe to replace" would let a template land on top of live work.
        let mut state = EditorState::starter();
        state.active_children_mut().clear();
        assert!(!active_page_is_blank_starter(&state));
    }
}
