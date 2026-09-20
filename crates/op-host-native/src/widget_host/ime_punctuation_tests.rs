//! Contract tests for IME commits that arrive WITHOUT a preceding
//! `Ime::Preedit` composition.
//!
//! CJK punctuation (《》【】——……) is committed by the system IME the
//! instant the key is pressed — there is no candidate list, so no
//! composition session ever opens. The platform therefore delivers a
//! bare `Ime::Commit("《")` with no preedit before it and no preedit
//! clear after it.
//!
//! These tests pin the host end of that contract: a bare commit must
//! land in whichever input owns the keyboard, at the caret, with the
//! caret advancing by the committed string's byte length — the same
//! way a commit that ends a real composition does.

use crate::WidgetHostNative;
use op_editor_core::NodeId;

const TEXT_DOC: &str = r#"{"version":"1.0.0","children":[
  {"type":"text","id":"t1","name":"Label","x":0,"y":0,"width":100,"height":40,
   "content":"","fontSize":20}
]}"#;

fn chat_host() -> WidgetHostNative {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut().chat.focused = true;
    host
}

fn canvas_text_host() -> WidgetHostNative {
    let mut host = WidgetHostNative::new();
    let doc = jian_ops_schema::load_str(TEXT_DOC)
        .expect("fixture JSON parses")
        .value;
    *host.editor_state_mut() = op_editor_core::EditorState::from_document(doc);
    assert!(host.editor_state_mut().start_text_edit(NodeId::new("t1")));
    host
}

/// A single bare commit — the exact event the IME emits for 《.
#[test]
fn chat_takes_a_commit_with_no_preceding_preedit() {
    let mut host = chat_host();

    assert!(
        host.apply_ime_commit("《"),
        "a bare commit must be consumed even though no composition opened"
    );

    assert_eq!(host.editor_state().chat.input.text(), "《");
    assert_eq!(
        host.editor_state().chat.input_caret(),
        "《".len(),
        "caret must sit after the committed punctuation"
    );
}

/// The full run of punctuation that has no composition session.
#[test]
fn chat_takes_every_bare_punctuation_commit_in_order() {
    let mut host = chat_host();
    for piece in ["《", "》", "【", "】", "——", "……", "，", "。"] {
        assert!(
            host.apply_ime_commit(piece),
            "bare commit {piece:?} dropped"
        );
    }

    let expected = "《》【】——……，。";
    assert_eq!(host.editor_state().chat.input.text(), expected);
    assert_eq!(host.editor_state().chat.input_caret(), expected.len());
}

/// The reported user string, replayed as the platform delivers it:
/// composed words arrive as preedit-then-commit, punctuation as a bare
/// commit with no composition around it. Character order and the caret
/// must both survive the interleaving.
#[test]
fn chat_keeps_order_and_caret_across_mixed_composed_and_bare_commits() {
    let mut host = chat_host();

    // "设计一套电影" — a real composition: preedit, preedit clear, commit.
    host.apply_ime_preedit("shejiyitaodianying", Some((18, 18)));
    host.apply_ime_preedit("", None);
    assert!(host.apply_ime_commit("设计一套电影"));

    // "《" — bare commit, no composition.
    assert!(host.apply_ime_commit("《"));

    // "流浪" — composition again.
    host.apply_ime_preedit("liulang", Some((7, 7)));
    host.apply_ime_preedit("", None);
    assert!(host.apply_ime_commit("流浪"));

    // "》" — bare commit.
    assert!(host.apply_ime_commit("》"));

    let expected = "设计一套电影《流浪》";
    assert_eq!(host.editor_state().chat.input.text(), expected);
    assert_eq!(
        host.editor_state().chat.input_caret(),
        expected.len(),
        "caret must track the end of the text, not a stale composition region"
    );
    assert!(
        host.editor_state()
            .ui
            .text_edit_input
            .composition()
            .is_none(),
        "no composition may be left in flight after the last bare commit"
    );
}

/// A bare commit landing mid-text inserts at the caret and leaves the
/// tail intact — the caret must not jump to either end.
#[test]
fn chat_bare_commit_inserts_at_the_caret_not_at_the_end() {
    let mut host = chat_host();
    host.editor_state_mut().chat.set_input_text("ab");
    host.editor_state_mut().chat.set_input_caret(1, 0);

    assert!(host.apply_ime_commit("《"));

    assert_eq!(host.editor_state().chat.input.text(), "a《b");
    assert_eq!(
        host.editor_state().chat.input_caret(),
        "a《".len(),
        "caret must advance past the insertion, not move to a text edge"
    );
}

/// The canvas text editor takes a bare commit too — it must not need a
/// composition region to exist first.
#[test]
fn canvas_text_edit_takes_a_commit_with_no_preceding_preedit() {
    let mut host = canvas_text_host();

    assert!(host.apply_ime_commit("《"));

    assert_eq!(host.editor_state().text_edit_content(), Some("《"));
    assert!(
        host.editor_state()
            .ui
            .text_edit_input
            .composition()
            .is_none(),
        "a bare commit must not leave a composition in flight"
    );
}

/// Same interleaving as the chat case, against the canvas text editor.
#[test]
fn canvas_text_edit_keeps_order_across_mixed_composed_and_bare_commits() {
    let mut host = canvas_text_host();

    host.apply_ime_preedit("dianying", Some((8, 8)));
    assert!(host.apply_ime_commit("电影"));
    assert!(host.apply_ime_commit("《"));
    host.apply_ime_preedit("liulang", Some((7, 7)));
    assert!(host.apply_ime_commit("流浪"));
    assert!(host.apply_ime_commit("》"));

    assert_eq!(
        host.editor_state().text_edit_content(),
        Some("电影《流浪》")
    );
    assert!(host
        .editor_state()
        .ui
        .text_edit_input
        .composition()
        .is_none());
}
