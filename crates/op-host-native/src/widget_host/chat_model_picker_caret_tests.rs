use super::WidgetHostNative;
use op_editor_core::chat::{AgentProvider, ModelEntry};
use op_editor_core::NodeId;
use op_editor_ui::widgets::{AIChatHit, AIChatPlaceholder};
use op_editor_ui::Point2D;

#[test]
fn chat_model_picker_arrows_move_caret_for_insert_and_backspace() {
    let mut host = WidgetHostNative::new();
    {
        let ui = &mut host.editor_state_mut().editor_ui;
        ui.chat_model_picker.open = true;
        ui.chat_model_picker_input.set_text("abcd");
    }

    assert!(host.apply_chat_model_picker_caret(false));
    assert!(host.apply_chat_model_picker_caret(false));
    assert_eq!(
        host.editor_state()
            .editor_ui
            .chat_model_picker_input
            .caret(),
        2
    );

    assert!(host.apply_text('X'));
    assert_eq!(
        host.editor_state().editor_ui.chat_model_picker_input.text(),
        "abXcd"
    );
    assert_eq!(
        host.editor_state()
            .editor_ui
            .chat_model_picker_input
            .caret(),
        3
    );

    assert!(host.apply_backspace());
    assert_eq!(
        host.editor_state().editor_ui.chat_model_picker_input.text(),
        "abcd"
    );
    assert_eq!(
        host.editor_state()
            .editor_ui
            .chat_model_picker_input
            .caret(),
        2
    );
}

#[test]
fn chat_model_picker_clear_button_empties_search() {
    let mut host = WidgetHostNative::new();
    host.set_now_ms(456);
    {
        let ui = &mut host.editor_state_mut().editor_ui;
        ui.chat_model_picker.open = true;
        ui.chat_model_picker_input.set_text("231");
        ui.chat_model_picker.scroll.offset = 10.0;
        ui.chat_model_picker.hover = Some(0);
    }
    let chat_rect = host.ai_chat_rect(1200.0, 800.0).unwrap();
    let panel = AIChatPlaceholder::from_editor_at(host.editor_state(), 456);
    let picker = panel.model_picker_bounds(chat_rect).unwrap();
    let x = picker.origin.x + picker.size.x - 24.0;
    let y = picker.origin.y + 19.0;

    assert!(host.apply_click(x, y, 1200.0, 800.0));

    let ui = &host.editor_state().editor_ui;
    assert!(ui.chat_model_picker_input.text().is_empty());
    assert_eq!(ui.chat_model_picker_input.caret(), 0);
    assert_eq!(ui.chat_model_picker.scroll.offset, 0.0);
    assert_eq!(ui.chat_model_picker.hover, None);
    assert!(ui.chat_model_picker.open);
    assert_eq!(ui.chat_model_picker_input.next_blink_flip_ms(456), 956);
}

#[test]
fn opening_model_picker_clears_covered_hover_before_next_cursor_move() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut()
        .chat
        .available_models
        .push(ModelEntry::new(AgentProvider::CodexCli, "gpt-5", "GPT-5"));
    let (viewport_w, viewport_h) = (1200.0, 800.0);
    let chat_rect = host
        .ai_chat_rect(viewport_w, viewport_h)
        .expect("chat rect");
    let panel = AIChatPlaceholder::from_editor(host.editor_state());
    let y = chat_rect.origin.y + chat_rect.size.y - 19.0;
    let model_point = (chat_rect.origin.x as i32..=(chat_rect.origin.x + chat_rect.size.x) as i32)
        .map(|x| Point2D::new(x as f32, y))
        .find(|point| panel.hit_test(chat_rect, *point) == Some(AIChatHit::ToggleModelPicker))
        .expect("model picker chip");
    {
        let ui = &mut host.editor_state_mut().editor_ui;
        ui.canvas_hover_node = Some(NodeId::new("stale-canvas"));
        ui.hovered_layer_id = Some(NodeId::new("stale-layer"));
        ui.property_action_hover = Some(0);
        ui.chat_header_hover = Some(op_editor_core::ChatHeaderButton::NewChat);
    }

    assert!(host.apply_click(model_point.x, model_point.y, viewport_w, viewport_h));

    let ui = &host.editor_state().editor_ui;
    assert!(ui.chat_model_picker.open);
    assert_eq!(ui.canvas_hover_node, None);
    assert_eq!(ui.hovered_layer_id, None);
    assert_eq!(ui.property_action_hover, None);
    assert_eq!(ui.chat_header_hover, None);
}
