//! Native-host keyboard routing for the Scene Template Center's generate row.
//!
//! The panel floats over the canvas, so every key it does not use still has
//! to stop at it. These tests are about that boundary as much as about the
//! row: a keystroke that leaks becomes a tool switch or a node nudge under an
//! open panel, and the user sees the document change while typing a topic.

use super::WidgetHostNative;
use op_editor_core::{SceneTemplateFocus, Tool};
use op_editor_ui::widgets::SceneTemplatePanel;
use op_editor_ui::Point2D;

const VIEWPORT_W: f32 = 1_200.0;
const VIEWPORT_H: f32 = 800.0;

/// A host with the capability bit set, which is what desktop does at boot.
fn generate_host() -> WidgetHostNative {
    let mut host = WidgetHostNative::new();
    host.last_viewport_w = VIEWPORT_W;
    host.last_viewport_h = VIEWPORT_H;
    host.editor_state_mut()
        .editor_ui
        .scene_template_generate_supported = true;
    host.editor_state_mut()
        .editor_ui
        .open_scene_template_center(1);
    host.editor_state_mut()
        .editor_ui
        .scene_template_center
        .focus = SceneTemplateFocus::Generate;
    host
}

fn type_str(host: &mut WidgetHostNative, text: &str) {
    for character in text.chars() {
        assert!(
            host.apply_text(character),
            "the open panel must own `{character}`"
        );
    }
}

fn topic(host: &WidgetHostNative) -> String {
    host.editor_state()
        .editor_ui
        .scene_template_center
        .generate
        .text()
        .to_string()
}

#[test]
fn typing_a_topic_never_reaches_the_canvas_shortcuts() {
    let mut host = generate_host();
    let tool_before = host.editor_state().tool;

    // Every one of these is a canvas shortcut: r/e/t/f pick tools, v selects.
    type_str(&mut host, "rectangle vs frame");

    assert_eq!(topic(&host), "rectangle vs frame");
    assert_eq!(
        host.editor_state().tool,
        tool_before,
        "a typed topic must not switch the active tool"
    );
    assert_ne!(host.editor_state().tool, Tool::Rect);
}

#[test]
fn backspace_and_delete_edit_the_topic_instead_of_the_selection() {
    let mut host = generate_host();
    type_str(&mut host, "ab");

    assert!(host.apply_backspace());
    assert_eq!(topic(&host), "a");

    host.editor_state_mut()
        .editor_ui
        .scene_template_center
        .generate
        .set_caret(0, 1);
    assert!(host.apply_delete());
    assert!(topic(&host).is_empty());

    // Still owned when there is nothing left to delete — falling through
    // here is what would delete the selected node.
    assert!(host.apply_backspace());
    assert!(host.apply_delete());
}

#[test]
fn arrows_move_the_caret_rather_than_nudging_the_selection() {
    let mut host = generate_host();
    type_str(&mut host, "abc");
    assert_eq!(
        host.editor_state()
            .editor_ui
            .scene_template_center
            .generate
            .caret(),
        3
    );

    assert!(host.apply_scene_template_caret(false, false));
    assert_eq!(
        host.editor_state()
            .editor_ui
            .scene_template_center
            .generate
            .caret(),
        2
    );
    assert!(host.apply_scene_template_caret(true, false));
    assert_eq!(
        host.editor_state()
            .editor_ui
            .scene_template_center
            .generate
            .caret(),
        3
    );
}

#[test]
fn enter_submits_the_topic_and_never_sends_a_chat_message() {
    let mut host = generate_host();
    type_str(&mut host, "Q3 复盘");

    assert!(host.apply_send(), "Enter is owned by the open panel");

    let center = &host.editor_state().editor_ui.scene_template_center;
    assert_eq!(center.pending_generate.as_deref(), Some("Q3 复盘"));
    assert!(!center.open, "submitting dismisses the panel");
    assert!(
        host.editor_state().chat.pending_send.is_none(),
        "the request goes through the host drain, not straight to chat"
    );
    assert!(host.editor_state().chat.messages.is_empty());
}

#[test]
fn enter_on_the_search_field_is_swallowed_rather_than_generating() {
    let mut host = generate_host();
    host.editor_state_mut()
        .editor_ui
        .scene_template_center
        .focus = SceneTemplateFocus::Search;
    host.editor_state_mut()
        .editor_ui
        .scene_template_center
        .generate
        .set_text("Q3 复盘");

    assert!(host.apply_send());

    let center = &host.editor_state().editor_ui.scene_template_center;
    assert_eq!(center.pending_generate, None);
    assert!(center.open, "Enter in the search field changes nothing");
    assert!(host.editor_state().chat.pending_send.is_none());
}

/// Escape peels the focus before the panel, so a mis-click into the topic
/// field costs one press to undo rather than the whole typed topic.
#[test]
fn escape_leaves_the_panel_one_layer_at_a_time() {
    let mut host = generate_host();
    type_str(&mut host, "Q3 复盘");

    assert!(host.apply_escape());
    let center = &host.editor_state().editor_ui.scene_template_center;
    assert!(center.open, "the first press only moves focus");
    assert_eq!(center.focus, SceneTemplateFocus::Search);
    assert_eq!(center.generate.text(), "Q3 复盘", "the topic survives");

    assert!(host.apply_escape());
    assert!(!host.editor_state().editor_ui.scene_template_center.open);
}

/// A press on the button routes through the shared flow and lands on the
/// host as a pending request — the wiring between the two halves.
#[test]
fn pressing_the_generate_button_raises_a_request_on_the_host() {
    let mut host = generate_host();
    type_str(&mut host, "季度复盘");
    let panel_rect = host
        .scene_template_panel_rect(VIEWPORT_W, VIEWPORT_H)
        .expect("scene template rect");
    let button = SceneTemplatePanel::for_editor(host.editor_state())
        .expect("open")
        .generate_button_rect(panel_rect)
        .expect("the desktop capability bit shows the row");
    let point = Point2D::new(
        button.origin.x + button.size.x / 2.0,
        button.origin.y + button.size.y / 2.0,
    );

    assert!(host.apply_press(point.x, point.y, VIEWPORT_W, VIEWPORT_H));

    let center = &host.editor_state().editor_ui.scene_template_center;
    assert_eq!(center.pending_generate.as_deref(), Some("季度复盘"));
    assert!(!center.open);
}
