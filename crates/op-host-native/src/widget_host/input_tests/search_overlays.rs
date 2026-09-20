//! Overlays that own the keyboard while open — component browser, chat
//! model picker, icon picker — plus select-all replacement across every
//! chrome text input.
//!
//! Split out of `input_tests.rs` to keep every file under the repo's
//! 800-line cap.

use super::*;

#[test]
fn component_browser_open_owns_keyboard_search() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut().editor_ui.component_browser_open = true;

    assert!(host.input_active_pub());
    assert!(host.apply_text('b'));
    assert!(host.apply_text('a'));
    assert_eq!(host.editor_state().editor_ui.component_browser_search, "ba");
    assert!(host.apply_backspace());
    assert_eq!(host.editor_state().editor_ui.component_browser_search, "b");
    assert!(host.apply_escape());
    assert!(!host.editor_state().editor_ui.component_browser_open);
}

#[test]
fn component_browser_header_buttons_queue_kit_io_requests() {
    // Press → dispatch → `component_browser_kit_request` seam: the
    // desktop runner drains the queued request into the rfd dialogs
    // (`op-host-desktop/src/kit_io.rs`).
    let mut host = WidgetHostNative::new();
    host.editor_state_mut().editor_ui.component_browser_open = true;
    let (vw, vh) = (1440.0, 900.0);
    let rect = host
        .component_browser_panel_rect(vw, vh)
        .expect("open panel rect");
    let right = rect.origin.x + rect.size.x;
    let y = rect.origin.y + 20.0; // header-button row centre (HEADER_H / 2)
                                  // Header buttons right-aligned: ✕ centre at right-26, Upload
                                  // (import) at right-54, Download (export) at right-82 — from
                                  // PAD 14 + 24-px buttons + 4-px gaps.
    assert!(host.apply_press(right - 54.0, y, vw, vh));
    assert_eq!(
        host.editor_state().editor_ui.component_browser_kit_request,
        Some(op_editor_core::KitIoRequest::Import)
    );
    host.editor_state_mut()
        .editor_ui
        .component_browser_kit_request = None;
    assert!(host.apply_press(right - 82.0, y, vw, vh));
    assert_eq!(
        host.editor_state().editor_ui.component_browser_kit_request,
        Some(op_editor_core::KitIoRequest::Export)
    );
}

#[test]
fn component_browser_header_press_sets_and_release_clears_pressed_button() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut().editor_ui.component_browser_open = true;
    let (vw, vh) = (1440.0, 900.0);
    let rect = host
        .component_browser_panel_rect(vw, vh)
        .expect("open panel rect");
    let x = rect.origin.x + rect.size.x - 54.0;
    let y = rect.origin.y + 20.0;

    assert!(host.apply_press(x, y, vw, vh));
    assert_eq!(
        host.editor_state().editor_ui.pressed_button,
        Some(op_editor_core::ButtonPressTarget::ComponentBrowser(
            op_editor_core::ComponentBrowserButton::ImportKit
        ))
    );

    assert!(host.apply_release_with_viewport(vw, vh));
    assert_eq!(host.editor_state().editor_ui.pressed_button, None);
}

#[test]
fn chat_model_picker_open_owns_keyboard_search() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut().editor_ui.chat_model_picker.open = true;
    host.editor_state_mut().chat.focused = true;

    assert!(host.input_active_pub());
    assert!(host.apply_text('g'));
    assert!(host.apply_text('p'));
    assert_eq!(
        host.editor_state().editor_ui.chat_model_picker_input.text(),
        "gp"
    );
    assert!(host.editor_state().chat.input.text().is_empty());
    assert!(host.apply_backspace());
    assert_eq!(
        host.editor_state().editor_ui.chat_model_picker_input.text(),
        "g"
    );
    assert!(host.apply_escape());
    assert!(!host.editor_state().editor_ui.chat_model_picker.open);
}

#[test]
fn select_all_in_chat_input_replaces_next_typed_text() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut().chat.focused = true;
    host.editor_state_mut().chat.set_input_text("abcdef");

    assert!(host.apply_select_all());
    assert_eq!(host.editor_state().chat.input.text(), "abcdef");
    assert!(host.apply_text('X'));
    assert_eq!(host.editor_state().chat.input.text(), "X");
}

#[test]
fn select_all_in_settings_input_replaces_next_typed_text() {
    let mut host = WidgetHostNative::new();
    {
        let ui = &mut host.editor_state_mut().editor_ui;
        ui.agent_settings.focus = Some(
            op_editor_core::agent_settings::SettingsFocus::BuiltinAgentDraft(
                op_editor_core::agent_settings::BuiltinAgentField::BaseUrl,
            ),
        );
        ui.settings_input.set_text("https://example.invalid");
    }

    assert!(host.apply_select_all());
    assert!(host.apply_text('x'));
    assert_eq!(host.editor_state().editor_ui.settings_input.text(), "x");
    assert_eq!(host.editor_state().editor_ui.settings_input.caret(), 1);
}

#[test]
fn select_all_in_property_input_replaces_next_typed_text() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut().ui.property_focus = Some(PropertyFocus::PositionX);
    host.editor_state_mut().ui.property_input.set_text("123");

    assert!(host.apply_select_all());
    assert!(host.apply_text('4'));
    assert_eq!(host.editor_state().ui.property_input.text(), "4");
    assert_eq!(host.editor_state().ui.property_input.caret(), 1);
}

#[test]
fn property_input_uses_text_input_state_for_editing() {
    let mut host = WidgetHostNative::new();
    {
        let ui = &mut host.editor_state_mut().ui;
        ui.property_focus = Some(PropertyFocus::PositionX);
        ui.property_input.set_text("1234");
    }

    assert!(host.apply_property_caret(false));
    assert!(host.apply_property_caret(false));
    assert_eq!(host.editor_state().ui.property_input.caret(), 2);

    assert!(host.apply_text('9'));
    assert_eq!(host.editor_state().ui.property_input.text(), "12934");
    assert_eq!(host.editor_state().ui.property_input.caret(), 3);

    assert!(host.apply_backspace());
    assert_eq!(host.editor_state().ui.property_input.text(), "1234");
    assert_eq!(host.editor_state().ui.property_input.caret(), 2);

    assert!(host.apply_select_all());
    assert!(host.apply_text('5'));
    assert_eq!(host.editor_state().ui.property_input.text(), "5");
    assert_eq!(host.editor_state().ui.property_input.caret(), 1);
}

#[test]
fn select_all_in_chat_model_picker_replaces_next_typed_text() {
    let mut host = WidgetHostNative::new();
    {
        let ui = &mut host.editor_state_mut().editor_ui;
        ui.chat_model_picker.open = true;
        ui.chat_model_picker_input.set_text("gpt");
    }

    assert!(host.apply_select_all());
    assert!(host.apply_text('x'));
    assert_eq!(
        host.editor_state().editor_ui.chat_model_picker_input.text(),
        "x"
    );
    assert_eq!(
        host.editor_state()
            .editor_ui
            .chat_model_picker_input
            .caret(),
        1
    );
}

#[test]
fn shape_picker_icon_row_opens_icon_picker() {
    let mut host = WidgetHostNative::new();
    let viewport_w = 1440.0;
    let viewport_h = 900.0;
    host.editor_state_mut().editor_ui.shape_picker.open = true;

    let panel = host.shape_picker_rect(viewport_w, viewport_h);
    let picker = op_editor_ui::widgets::ShapePicker::for_editor_ui(&host.editor_state().editor_ui);
    let x = panel.origin.x + 24.0;
    let mut probe = panel.origin.y + 2.0;
    let mut icon_row_y = None;
    while probe < panel.origin.y + panel.size.y {
        if matches!(
            picker.hit_test(panel, op_editor_ui::Point2D::new(x, probe)),
            Some(op_editor_ui::widgets::ShapeChoice::OpenIconPicker)
        ) {
            icon_row_y = Some(probe);
            break;
        }
        probe += 2.0;
    }
    let icon_row_y = icon_row_y.expect("icon row present in the shape picker");
    assert!(host.apply_press(x, icon_row_y, viewport_w, viewport_h));

    assert!(!host.editor_state().editor_ui.shape_picker.open);
    assert!(host.editor_state().editor_ui.icon_picker.open);
}

#[test]
fn icon_picker_open_owns_keyboard_search() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut().editor_ui.icon_picker.open = true;

    assert!(host.input_active_pub());
    assert!(host.apply_text('h'));
    assert!(host.apply_text('o'));
    assert_eq!(host.editor_state().editor_ui.icon_picker_search, "ho");
    assert!(host.apply_backspace());
    assert_eq!(host.editor_state().editor_ui.icon_picker_search, "h");
    assert!(host.apply_escape());
    assert!(!host.editor_state().editor_ui.icon_picker.open);
}

#[test]
fn icon_picker_click_inserts_icon_font_node() {
    let mut host = WidgetHostNative::new();
    let viewport_w = 1440.0;
    let viewport_h = 900.0;
    host.editor_state_mut().editor_ui.icon_picker.open = true;
    host.editor_state_mut().editor_ui.icon_picker_search = "home".to_string();

    let panel = host
        .icon_picker_panel_rect(viewport_w, viewport_h)
        .expect("icon picker rect");
    let row_y = panel.origin.y + 40.0 + 42.0 + 20.0;
    assert!(host.apply_press(panel.origin.x + 40.0, row_y, viewport_w, viewport_h));

    assert!(!host.editor_state().editor_ui.icon_picker.open);
    let icon = host
        .editor_state()
        .doc
        .children
        .iter()
        .find_map(|node| match node {
            jian_ops_schema::node::PenNode::IconFont(icon) => Some(icon),
            _ => None,
        })
        .expect("inserted icon_font node");
    assert_eq!(icon.icon_font_name, "home");
    assert_eq!(icon.icon_font_family.as_deref(), Some("lucide"));
    assert_eq!(host.editor_state().selection.anchor.as_str(), icon.base.id);
}

#[test]
fn icon_picker_header_drag_moves_the_panel() {
    let mut host = WidgetHostNative::new();
    let viewport_w = 1440.0;
    let viewport_h = 900.0;
    host.editor_state_mut().editor_ui.icon_picker.open = true;

    let start = host
        .icon_picker_panel_rect(viewport_w, viewport_h)
        .expect("icon picker rect");
    let press_x = start.origin.x + 72.0;
    let press_y = start.origin.y + 20.0;
    assert!(host.apply_press(press_x, press_y, viewport_w, viewport_h));

    assert!(host.apply_cursor_move(press_x + 96.0, press_y + 44.0));
    let moved = host
        .icon_picker_panel_rect(viewport_w, viewport_h)
        .expect("icon picker rect after drag");

    assert_eq!(moved.origin.x, start.origin.x + 96.0);
    assert_eq!(moved.origin.y, start.origin.y + 44.0);
    assert!(host.apply_release_with_viewport(viewport_w, viewport_h));
}
