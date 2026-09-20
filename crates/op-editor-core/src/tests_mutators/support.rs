//! Shared fixtures for the mutator test siblings.

use crate::pen_node_ext::PenNodeExt;
use crate::test_support::{rect, state_with};

pub(super) fn three_rects() -> crate::state::EditorState {
    state_with(vec![
        rect("n1", "A", 0.0, 0.0, 10.0, 10.0),
        rect("n2", "B", 0.0, 0.0, 10.0, 10.0),
        rect("n3", "C", 0.0, 0.0, 10.0, 10.0),
    ])
}

pub(super) fn root_ids(s: &crate::state::EditorState) -> Vec<String> {
    s.active_children()
        .iter()
        .map(|n| n.id_str().to_string())
        .collect()
}
