//! Mutator tests — selection / tree-ops / grouping / history.
//! Ported from `openpencil-shell-core::document::tests_mutators`,
//! retargeted onto `EditorState` + the canonical `PenNode` tree.
//!
//! Slim spine; the cases live in sibling modules under
//! `tests_mutators/` to keep every file under the 800-line cap.

#![cfg(test)]

mod chat_models;
mod history;
mod selection;
mod support;
mod tree_ops;
mod ui_state;
