//! Selection-biased standard-route intent tests.

use op_ai::chat_provider::{ChatDelta, ChatProvider, ChatRequest, StopReason};
use op_editor_core::EditorState;
use std::time::Duration;

use super::*;

struct Scripted;

impl ChatProvider for Scripted {
    fn provider_label(&self) -> &str {
        "scripted"
    }

    fn send(&self, _request: ChatRequest) -> Box<dyn Iterator<Item = ChatDelta> + Send> {
        Box::new(
            vec![
                ChatDelta::TextDelta("DESIGN_NEW".to_string()),
                ChatDelta::Done {
                    stop_reason: StopReason::EndTurn,
                },
            ]
            .into_iter()
            .inspect(|_| std::thread::sleep(Duration::ZERO)),
        )
    }
}

fn frame(id: &str, name: &str, children: Vec<PenNode>) -> PenNode {
    let mut node: PenNode = serde_json::from_value(serde_json::json!({
        "type": "frame",
        "id": id,
        "name": name,
        "x": 0.0,
        "y": 0.0,
        "width": 390.0,
        "height": 800.0,
        "children": [],
    }))
    .expect("valid frame json");
    if let Some(kids) = node.children_mut() {
        *kids = children;
    }
    node
}

fn state_with_selected_card() -> EditorState {
    let mut state = EditorState::new();
    state.active_children_mut().clear();
    state.active_children_mut().push(frame(
        "screen",
        "Home",
        vec![frame("card", "Selected Card", Vec::new())],
    ));
    state.set_single_selection(op_editor_core::NodeId::new("card"));
    state
}

#[test]
fn selection_bias_routes_keywordless_instruction_to_modify() {
    let provider = Scripted;
    let state = state_with_selected_card();

    assert_eq!(
        classify_intent_for_standard_route(&provider, &state, "给它加一个边框", None),
        DesignIntent::Modify
    );
}

#[test]
fn selection_bias_does_not_hijack_whole_new_screen_or_chat() {
    let provider = Scripted;
    let state = state_with_selected_card();

    assert_eq!(
        classify_intent_for_standard_route(&provider, &state, "重新画一个首页", None),
        DesignIntent::New
    );
    assert_eq!(
        classify_intent_for_standard_route(&provider, &state, "这是什么字体", None),
        DesignIntent::Chat
    );
}

#[test]
fn selection_does_not_hijack_english_section_heavy_new_design() {
    // Regression: "Design a … page" whose spec mentions "section" three times
    // trips `is_section_add_request`, so `requests_new_whole_screen` is false;
    // a stray selection then dragged the whole new-design prompt into modify
    // (measured: M3 flat-JSONL → "Could not parse design nodes"). The
    // creation-signal veto keeps it New.
    let provider = Scripted;
    let state = state_with_selected_card();
    let prompt = "Design a travel booking mobile app explore page. Include a search section with \"Where to?\" input, date picker chips, and guest count. \"Deals of the Week\" section with 2 featured deal cards. Recently viewed section with 2 compact cards. Bottom tab bar. Warm, inviting design with orange accents.";
    assert!(
        crate::chat_intent::has_new_screen_creation_signal(prompt),
        "creation signal must fire on a design-a-page prompt"
    );
    assert_ne!(
        classify_intent_for_standard_route(&provider, &state, prompt, None),
        DesignIntent::Modify,
        "a section-heavy new-design prompt must not be hijacked to modify by a selection"
    );
}

#[test]
fn selection_does_not_hijack_listed_follow_on_screens() {
    let provider = Scripted;
    let state = state_with_selected_card();
    for prompt in [
        "继续完成 explore/profile界面",
        "Continue generating the explore/profile interface",
    ] {
        assert!(
            requests_new_whole_screen(prompt),
            "an explicit list of sibling interfaces is a whole-screen request: {prompt}"
        );
        assert!(
            detect_append_intent(&state, prompt).is_none(),
            "listed sibling interfaces must not append into the first existing frame: {prompt}"
        );
        assert_eq!(
            classify_intent_for_standard_route(&provider, &state, prompt, None),
            DesignIntent::New,
            "a stale selection must not turn a multi-screen continuation into modify: {prompt}"
        );
        assert!(
            should_auto_generate_design_md(&state, prompt, None),
            "follow-on screens should inherit the existing canvas design system: {prompt}"
        );
    }
}

#[test]
fn ambiguous_current_interface_completion_is_not_a_new_screen() {
    for prompt in [
        "继续完成这个界面",
        "继续完成当前界面的推荐区块",
        "继续完成 profile 界面的 header",
        "继续完成 explore/profile 界面之间的跳转",
    ] {
        assert!(
            !requests_new_whole_screen(prompt),
            "current-screen or subobject work must stay in-place: {prompt}"
        );
    }
}
