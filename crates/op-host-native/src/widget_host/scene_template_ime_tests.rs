//! Contract tests for typing into the Asset Center.
//!
//! The gallery is a full-canvas overlay with two text fields, and it reached
//! users unable to accept IME input: `input_active` — the one list of surfaces
//! that own the keyboard — did not name it, so `text_input_focus_active` read
//! false, the desktop shell called `set_ime_allowed(false)`, and the platform
//! never opened a composition session. Most Latin characters still worked,
//! because they arrive as ordinary key events and `apply_text` has always
//! routed the panel — but the same list gates the single-letter tool
//! switches, so `v r o l t f p y h` were consumed by the toolbar instead and
//! silently changed the canvas tool behind the overlay. That asymmetry is
//! what made the bug read as "the caret blinks but nothing types" to anyone
//! using pinyin, and as nothing at all to anyone testing with `abc`.
//!
//! So each field is pinned on both roads: a plain character and a committed
//! candidate. A test that only drove `apply_text` would have stayed green
//! through the entire outage.

use crate::WidgetHostNative;
use op_editor_core::{EditorState, NodeId, SceneTemplateFocus};

const TEXT_DOC: &str = r#"{"version":"1.0.0","children":[
  {"type":"text","id":"t1","name":"Label","x":0,"y":0,"width":100,"height":40,
   "content":"","fontSize":20}
]}"#;

/// The panel open on the tab a user lands on, with generation available.
fn gallery_host() -> WidgetHostNative {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut()
        .editor_ui
        .scene_template_generate_supported = true;
    host.editor_state_mut()
        .editor_ui
        .open_scene_template_center(0);
    host
}

/// The state "generate from this" leaves behind: topic field focused without
/// the user ever having clicked it.
fn topic_focused_host() -> WidgetHostNative {
    let mut host = gallery_host();
    let template = op_editor_core::scene_template_catalog::scene_template_by_id("slide-deck")
        .expect("the deck template ships");
    assert!(host
        .editor_state_mut()
        .editor_ui
        .use_scene_template_as_generate_basis(template));
    assert_eq!(
        host.editor_state().editor_ui.scene_template_center.focus,
        SceneTemplateFocus::Generate
    );
    host
}

fn topic(host: &WidgetHostNative) -> String {
    host.editor_state()
        .editor_ui
        .scene_template_center
        .generate
        .text()
        .to_string()
}

fn search(host: &WidgetHostNative) -> String {
    host.editor_state()
        .editor_ui
        .scene_template_center
        .search
        .text()
        .to_string()
}

/// The gate the whole bug hung on. Without this the shell disables IME and
/// no amount of correct routing below ever sees an event.
#[test]
fn an_open_gallery_owns_the_keyboard_for_ime_purposes() {
    let plain = WidgetHostNative::new();
    assert!(!plain.text_input_focus_active());

    assert!(
        gallery_host().text_input_focus_active(),
        "the Asset Center must report a live text input, or the desktop \
         shell turns the platform IME off while the caret is blinking"
    );
}

/// The same list gates the desktop shell's single-letter tool switches
/// (`keyboard_input.rs`: `Key::Character(..) if !input_active_pub()`), which
/// consume the key when they fire. While the gallery was missing from it, a
/// `t` typed into the search box switched the canvas tool to Text and never
/// reached the field — nine letters of the alphabet were unusable and each
/// one quietly changed the document's tool behind the overlay.
#[test]
fn an_open_gallery_suppresses_the_single_letter_tool_shortcuts() {
    assert!(
        gallery_host().input_active_pub(),
        "tool shortcuts would eat letters typed into the gallery"
    );
}

/// Programmatic focus, then a pinyin candidate. This is the user's exact
/// sequence: press "generate from this", type a Chinese topic.
#[test]
fn a_committed_candidate_lands_in_the_programmatically_focused_topic_field() {
    let mut host = topic_focused_host();

    assert!(host.apply_ime_commit("季度复盘"));

    assert_eq!(topic(&host), "季度复盘");
    assert!(
        search(&host).is_empty(),
        "the topic must not leak into search"
    );
}

/// The Latin road through the same focus — green before the fix, and the
/// reason the outage read as "only Chinese is broken".
#[test]
fn plain_characters_land_in_the_programmatically_focused_topic_field() {
    let mut host = topic_focused_host();

    for c in "Q3".chars() {
        assert!(host.apply_text(c));
    }

    assert_eq!(topic(&host), "Q3");
}

/// The other field, reached by its own focus rather than by the card button.
#[test]
fn the_search_field_takes_both_roads_too() {
    let mut host = gallery_host();
    assert_eq!(
        host.editor_state().editor_ui.scene_template_center.focus,
        SceneTemplateFocus::Search
    );

    assert!(host.apply_text('P'));
    assert!(host.apply_ime_commit("演示"));

    assert_eq!(search(&host), "P演示");
    assert!(topic(&host).is_empty());
}

/// A canvas text node left mid-edit sits underneath the gallery. The
/// candidate belongs to the panel the user is looking at, not to the node
/// the overlay is covering.
#[test]
fn the_gallery_beats_a_stale_canvas_text_edit_and_chat_focus() {
    let mut host = WidgetHostNative::new();
    let doc = jian_ops_schema::load_str(TEXT_DOC)
        .expect("fixture JSON parses")
        .value;
    *host.editor_state_mut() = EditorState::from_document(doc);
    assert!(host.editor_state_mut().start_text_edit(NodeId::new("t1")));
    host.editor_state_mut().chat.focused = true;
    host.editor_state_mut()
        .editor_ui
        .scene_template_generate_supported = true;
    host.editor_state_mut()
        .editor_ui
        .open_scene_template_center(0);

    assert!(host.apply_ime_commit("模板"));

    assert_eq!(search(&host), "模板");
    assert!(
        host.editor_state().chat.input.text().is_empty(),
        "stale chat focus must not take the commit"
    );
}

/// The candidate window has to open under the text being composed. Anchoring
/// it at the pointer — the fallback when no rect resolves — puts the
/// candidate list somewhere unrelated to the field.
#[test]
fn the_candidate_window_anchors_at_the_focused_field() {
    let mut host = topic_focused_host();

    let rect = host
        .ime_anchor_rect(1440.0, 900.0)
        .expect("an open gallery resolves an anchor");

    let panel_rect = host
        .scene_template_panel_rect(1440.0, 900.0)
        .expect("the panel has a rect while open");
    assert!(
        panel_rect.contains(rect.origin),
        "the anchor landed outside the gallery: {rect:?}"
    );
}
