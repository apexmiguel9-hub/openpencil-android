use super::WidgetHostNative;
use op_editor_core::{
    CollabAvailability, CollabConnectionPhase, CollabPanelHover, CollabPanelView, CollabUiAction,
    EditorState, NodeId, PathAnchorMenuState, PenNodeExt, Tool,
};
use op_editor_ui::widgets::{
    login_modal::{LoginModal, LoginModalHit},
    CollabPanel, TopBar, TOP_BAR_HEIGHT,
};
use op_editor_ui::{Point2D, Rect};

const TWO_RECTS: &str = r#"{"version":"1.0.0","children":[
  {"type":"rectangle","id":"n1","name":"One","x":10,"y":20,"width":100,"height":50},
  {"type":"rectangle","id":"n2","name":"Two","x":200,"y":20,"width":100,"height":50}
]}"#;

fn focus_join(host: &mut WidgetHostNative) {
    let collab = &mut host.editor_state_mut().editor_ui.collab;
    collab.availability = CollabAvailability::Ready;
    collab.phase = CollabConnectionPhase::Idle;
    collab.panel.open = true;
    collab.panel.view = CollabPanelView::Join;
    collab.panel.join_address_focused = true;
    collab.panel.join_input.set_text("");
}

fn host_with_selected_node() -> WidgetHostNative {
    let document = jian_ops_schema::load_str(TWO_RECTS)
        .expect("fixture JSON parses")
        .value;
    let mut host = WidgetHostNative::new();
    *host.editor_state_mut() = EditorState::from_document(document);
    host.editor_state_mut()
        .set_single_selection(NodeId::new("n1"));
    focus_join(&mut host);
    host
}

#[test]
fn join_field_owns_native_text_ime_paste_backspace_and_enter() {
    let mut host = WidgetHostNative::new();
    focus_join(&mut host);

    assert!(host.input_active_pub());
    assert!(host.non_chat_input_owns_keyboard_pub());
    assert!(host.settings_focus_active(), "desktop shortcut guard");
    assert!(
        host.text_input_focus_active(),
        "native IME must stay focused"
    );

    assert!(host.apply_input_paste("opc1_Ab-9\n"));
    assert!(host.apply_ime_commit("Z"));
    assert_eq!(
        host.editor_state().editor_ui.collab.panel.join_input.text(),
        "opc1_Ab-9Z"
    );
    assert!(host.apply_text('/'), "rejected input is still consumed");
    assert_eq!(
        host.editor_state().editor_ui.collab.panel.join_input.text(),
        "opc1_Ab-9Z"
    );
    assert!(host.apply_backspace());
    assert_eq!(
        host.editor_state().editor_ui.collab.panel.join_input.text(),
        "opc1_Ab-9"
    );

    assert!(host.apply_send());
    assert!(
        !host
            .editor_state()
            .editor_ui
            .collab
            .panel
            .join_address_focused
    );
    assert_eq!(
        host.editor_state().editor_ui.collab.pending_action,
        Some(CollabUiAction::JoinAddress {
            endpoint: "opc1_Ab-9".to_string(),
        })
    );
}

#[test]
fn join_field_blocks_native_canvas_shortcuts() {
    let mut host = host_with_selected_node();
    let before = host
        .editor_state()
        .selected_node()
        .expect("selected node")
        .base()
        .x;

    assert!(!host.apply_delete());
    assert!(!host.apply_duplicate());
    assert!(!host.apply_nudge(10.0, 0.0));
    assert!(host.apply_select_all(), "Cmd/Ctrl+A remains owned");
    assert_eq!(host.editor_state().active_children().len(), 2);
    assert_eq!(host.editor_state().selection.set.len(), 1);
    assert_eq!(
        host.editor_state()
            .selected_node()
            .expect("selection survives")
            .base()
            .x,
        before
    );
    assert_eq!(host.editor_state().tool, Tool::Select);
}

#[test]
fn hidden_or_external_join_switch_drops_native_focus() {
    let mut host = WidgetHostNative::new();
    focus_join(&mut host);
    host.editor_state_mut().editor_ui.collab.panel.open = false;

    assert!(
        !host.input_active_pub(),
        "hidden focus cannot own shortcuts"
    );
    assert!(!host.non_chat_input_owns_keyboard_pub());
    assert!(!host.apply_text('x'), "hidden focus cannot receive text");
    assert!(host.blur_text_inputs_on_blank_press());
    assert!(
        !host
            .editor_state()
            .editor_ui
            .collab
            .panel
            .join_address_focused
    );

    focus_join(&mut host);
    assert!(host.apply_toggle_agent_settings());
    assert!(
        !host
            .editor_state()
            .editor_ui
            .collab
            .panel
            .join_address_focused
    );
}

#[test]
fn native_cursor_tracks_collab_control_hover_and_clears_on_exit() {
    let mut host = WidgetHostNative::new();
    host.last_viewport_w = 1200.0;
    host.last_viewport_h = 800.0;
    let collab = &mut host.editor_state_mut().editor_ui.collab;
    collab.availability = CollabAvailability::Ready;
    collab.panel.open = true;
    collab.panel.view = CollabPanelView::Home;

    let ui = &host.editor_state().editor_ui;
    let top_bar_rect = Rect::xywh(0.0, 0.0, host.last_viewport_w, TOP_BAR_HEIGHT);
    let top_bar = TopBar::for_editor_ui(ui);
    let panel = CollabPanel::for_editor_ui(ui).expect("open collaboration panel");
    let rect = panel.rect_at(
        top_bar.collaboration_chip_rect_estimated(top_bar_rect),
        Rect::xywh(0.0, 0.0, host.last_viewport_w, host.last_viewport_h),
    );
    let close_x = rect.origin.x + rect.size.x - 23.0;
    let close_y = rect.origin.y + 22.0;
    host.editor_state_mut().editor_ui.canvas_hover_node = Some(NodeId::new("stale"));

    assert!(host.apply_cursor_move(close_x, close_y));
    assert_eq!(
        host.editor_state().editor_ui.collab.panel.hover,
        Some(CollabPanelHover::Close)
    );
    assert_eq!(host.editor_state().editor_ui.canvas_hover_node, None);

    assert!(host.apply_cursor_move(2.0, 2.0));
    assert_eq!(host.editor_state().editor_ui.collab.panel.hover, None);
}

#[test]
fn native_path_context_menu_keeps_hover_priority_over_collab_panel() {
    let mut host = WidgetHostNative::new();
    host.last_viewport_w = 1200.0;
    host.last_viewport_h = 800.0;
    let collab = &mut host.editor_state_mut().editor_ui.collab;
    collab.availability = CollabAvailability::Ready;
    collab.panel.open = true;
    collab.panel.view = CollabPanelView::Home;
    collab.panel.hover = Some(CollabPanelHover::Close);

    let ui = &host.editor_state().editor_ui;
    let top_bar_rect = Rect::xywh(0.0, 0.0, host.last_viewport_w, TOP_BAR_HEIGHT);
    let top_bar = TopBar::for_editor_ui(ui);
    let panel = CollabPanel::for_editor_ui(ui).unwrap();
    let rect = panel.rect_at(
        top_bar.collaboration_chip_rect_estimated(top_bar_rect),
        Rect::xywh(0.0, 0.0, host.last_viewport_w, host.last_viewport_h),
    );
    let point =
        op_editor_ui::Point2D::new(rect.origin.x + rect.size.x - 23.0, rect.origin.y + 22.0);
    host.editor_state_mut().ui.path_anchor_menu = Some(PathAnchorMenuState {
        node_id: NodeId::new("n1"),
        anchor_index: 0,
        x: point.x - 20.0,
        y: point.y - 10.0,
        menu: Default::default(),
    });

    assert!(host.apply_cursor_move(point.x, point.y));
    assert_eq!(
        host.editor_state()
            .ui
            .path_anchor_menu
            .as_ref()
            .unwrap()
            .menu
            .hover,
        Some(0)
    );
    assert_eq!(host.editor_state().editor_ui.collab.panel.hover, None);
}

#[test]
fn native_collab_sign_in_hands_press_ownership_to_modal() {
    let (viewport_w, viewport_h) = (1200.0, 800.0);
    let mut host = WidgetHostNative::new();
    let ui = &mut host.editor_state_mut().editor_ui;
    ui.account_ui_available = true;
    ui.collab.availability = CollabAvailability::SignInRequired;
    ui.collab.panel.open = true;

    let top_bar_rect = Rect::xywh(0.0, 0.0, viewport_w, TOP_BAR_HEIGHT);
    let panel = CollabPanel::for_editor_ui(&host.editor_state().editor_ui).unwrap();
    let panel_rect = panel.rect_at(
        TopBar::for_editor_ui(&host.editor_state().editor_ui)
            .collaboration_chip_rect_estimated(top_bar_rect),
        Rect::xywh(0.0, 0.0, viewport_w, viewport_h),
    );
    let sign_in = Point2D::new(
        panel_rect.origin.x + panel_rect.size.x / 2.0,
        panel_rect.origin.y + 100.0,
    );
    assert!(matches!(
        panel.hit_test(panel_rect, sign_in),
        Some(op_editor_ui::widgets::CollabPanelHit::OpenSignIn)
    ));
    assert!(host.apply_press(sign_in.x, sign_in.y, viewport_w, viewport_h));
    assert!(host.editor_state().editor_ui.login_modal_open);
    assert!(!host.editor_state().editor_ui.collab.panel.open);

    host.editor_state_mut().editor_ui.collab.panel.open = true;
    let modal = LoginModal::for_editor(host.editor_state());
    let modal_rect = modal.rect(viewport_w, viewport_h);
    let close = Point2D::new(
        modal_rect.origin.x + modal_rect.size.x - 31.0,
        modal_rect.origin.y + 31.0,
    );
    assert_eq!(modal.hit_test(modal_rect, close), LoginModalHit::Close);
    assert!(host.apply_press(close.x, close.y, viewport_w, viewport_h));
    assert!(!host.editor_state().editor_ui.login_modal_open);
    assert!(host.editor_state().editor_ui.collab.panel.open);
}

#[test]
fn native_join_paste_replaces_and_select_all_clears() {
    let mut host = WidgetHostNative::new();
    focus_join(&mut host);

    assert!(host.apply_input_paste("opc1_first-code"));
    assert!(host.apply_input_paste("opc1_second-code"));
    assert_eq!(
        host.editor_state().editor_ui.collab.panel.join_input.text(),
        "opc1_second-code",
        "a pasted invite replaces the stale one instead of appending"
    );

    // Cmd/Ctrl+A owns the chord and selects the whole field...
    assert!(host.apply_select_all());
    assert!(host
        .editor_state()
        .editor_ui
        .collab
        .panel
        .join_input
        .highlight_range()
        .is_some());
    // ...so one Backspace clears it.
    assert!(host.apply_backspace());
    assert!(host
        .editor_state()
        .editor_ui
        .collab
        .panel
        .join_input
        .text()
        .is_empty());
}

#[test]
fn native_join_delete_clears_selection_and_never_deletes_nodes() {
    let mut host = host_with_selected_node();
    let before = host.editor_state().active_children().len();
    {
        let input = &mut host.editor_state_mut().editor_ui.collab.panel.join_input;
        input.set_text("opc1_code");
        input.select_all();
    }

    assert!(host.apply_delete());
    assert!(host
        .editor_state()
        .editor_ui
        .collab
        .panel
        .join_input
        .text()
        .is_empty());
    // Delete with an empty, unselected field is still swallowed.
    assert!(!host.apply_delete());
    assert_eq!(
        host.editor_state().active_children().len(),
        before,
        "canvas selection behind the panel must survive"
    );
}
