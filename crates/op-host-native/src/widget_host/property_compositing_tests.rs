//! Native host coverage for newly editable Figma compositing/page fields.

use super::WidgetHostNative;
use jian_ops_schema::node::base::MaskType;
use jian_ops_schema::style::BlendMode;
use op_editor_core::{CompositingPickerTarget, NodeId, PropertyFocus};
use op_editor_ui::widgets::PropertyPanelAction;

const VIEWPORT_W: f32 = 1200.0;
const VIEWPORT_H: f32 = 800.0;

fn seed(host: &mut WidgetHostNative, json: &str) {
    let doc = jian_ops_schema::load_str(json)
        .expect("fixture JSON parses")
        .value;
    *host.editor_state_mut() = op_editor_core::EditorState::from_document(doc);
}

fn commit_focus(host: &mut WidgetHostNative, focus: PropertyFocus, draft: &str) {
    host.editor_state_mut().ui.property_focus = Some(focus);
    host.editor_state_mut().ui.property_input.set_text(draft);
    host.commit_property_focus_if_any();
}

fn focus_tile_scale(host: &mut WidgetHostNative, draft: &str) {
    let state = host.editor_state_mut();
    state.editor_ui.image_fill_popover_open = true;
    state.ui.property_focus = Some(PropertyFocus::ImageTileScale);
    state.ui.property_input.set_text(draft);
    state.ui.property_input_draft = draft.to_string();
}

fn selected_tile_scale(host: &WidgetHostNative) -> f32 {
    op_editor_core::first_image_fill_summary(host.editor_state().selected_node().unwrap())
        .unwrap()
        .tile_scale
        .unwrap()
}

#[test]
fn native_compositing_actions_are_noop_safe_single_undo_steps() {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r##"{"version":"1.0.0","children":[{"type":"rectangle","id":"r","fill":[{"type":"solid","color":"#123456"}]}]}"##,
    );
    host.editor_state_mut()
        .set_single_selection(NodeId::new("r"));

    host.apply_property_action(PropertyPanelAction::SetNodeBlendMode(Some(
        BlendMode::Multiply,
    )));
    assert_eq!(
        host.editor_state().selected_node_blend_mode(),
        Some(BlendMode::Multiply)
    );
    assert_eq!(host.editor_state().history.past.len(), 1);

    // Selecting the already-authored option must not create a ghost undo.
    host.apply_property_action(PropertyPanelAction::SetNodeBlendMode(Some(
        BlendMode::Multiply,
    )));
    assert_eq!(host.editor_state().history.past.len(), 1);

    host.apply_property_action(PropertyPanelAction::SetFillBlendMode {
        index: 0,
        mode: Some(BlendMode::Screen),
    });
    assert_eq!(host.editor_state().history.past.len(), 2);
    assert_eq!(
        op_editor_core::fill_blend_mode_at(host.editor_state().selected_node().unwrap(), 0),
        Some(BlendMode::Screen)
    );
    assert!(host.editor_state_mut().undo());
    assert_eq!(
        op_editor_core::fill_blend_mode_at(host.editor_state().selected_node().unwrap(), 0),
        None
    );
    assert_eq!(
        host.editor_state().selected_node_blend_mode(),
        Some(BlendMode::Multiply)
    );
}

#[test]
fn native_mask_action_disables_legacy_path_mask_and_undo_restores_it() {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r#"{"version":"1.0.0","children":[{"type":"path","id":"p","mask":true}]}"#,
    );
    host.editor_state_mut()
        .set_single_selection(NodeId::new("p"));
    assert_eq!(
        host.editor_state().selected_node_mask_type(),
        Some(MaskType::Alpha)
    );

    host.apply_property_action(PropertyPanelAction::SetNodeMaskType(None));
    assert_eq!(host.editor_state().selected_node_mask_type(), None);
    assert_eq!(host.editor_state().history.past.len(), 1);
    assert!(host.editor_state_mut().undo());
    assert_eq!(
        host.editor_state().selected_node_mask_type(),
        Some(MaskType::Alpha)
    );
}

#[test]
fn native_page_rgba_and_tile_scale_focus_commits_round_trip_history() {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r#"{"version":"1.0.0","children":[{"type":"rectangle","id":"r","fill":[{"type":"image","url":"asset.png","mode":"tile"}]}]}"#,
    );

    commit_focus(&mut host, PropertyFocus::PageBackgroundHex, "#1a2b3c80");
    assert_eq!(
        host.editor_state().active_page_background_color(),
        Some("#1A2B3C80")
    );
    assert_eq!(host.editor_state().history.past.len(), 1);
    assert!(host.editor_state_mut().undo());
    assert!(host.editor_state().doc.pages.is_none());

    // Empty draft on an unset legacy page remains a no-op.
    commit_focus(&mut host, PropertyFocus::PageBackgroundHex, "");
    assert!(host.editor_state().history.past.is_empty());

    host.editor_state_mut()
        .set_single_selection(NodeId::new("r"));
    commit_focus(&mut host, PropertyFocus::ImageTileScale, "0.38618907");
    assert_eq!(host.editor_state().history.past.len(), 1);
    assert_eq!(
        op_editor_core::first_image_fill_summary(host.editor_state().selected_node().unwrap())
            .unwrap()
            .tile_scale,
        Some(0.38618907)
    );
}

#[test]
fn native_compositing_picker_toggle_and_escape_keep_selection() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut()
        .set_single_selection(NodeId::new("n10"));
    host.apply_property_action(PropertyPanelAction::ToggleCompositingPicker(
        CompositingPickerTarget::NodeBlend,
    ));
    assert!(host.editor_state().editor_ui.compositing_picker.open);
    assert!(host.apply_escape());
    assert!(!host.editor_state().editor_ui.compositing_picker.open);
    assert_eq!(host.editor_state().selection.anchor.as_str(), "n10");
}

#[test]
fn native_tile_scale_popover_input_owns_press_and_keeps_popup_open() {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r#"{"version":"1.0.0","children":[{"type":"rectangle","id":"r","fill":[{"type":"image","url":"asset.png","mode":"tile","tileScale":0.38618907}]}]}"#,
    );
    host.editor_state_mut()
        .set_single_selection(NodeId::new("r"));
    host.editor_state_mut().editor_ui.image_fill_popover_open = true;
    let panel = op_editor_ui::widgets::PropertyPanel::for_selection(host.editor_state()).unwrap();
    let rect = host.property_rect(VIEWPORT_W, VIEWPORT_H);
    let point = find_tile_scale_input(&panel, rect);

    assert!(host.apply_press(point.x, point.y, VIEWPORT_W, VIEWPORT_H));
    assert_eq!(
        host.editor_state().ui.property_focus,
        Some(PropertyFocus::ImageTileScale)
    );
    assert_eq!(host.editor_state().ui.property_input.text(), "0.38618907");
    assert!(host.editor_state().editor_ui.image_fill_popover_open);

    // Focusing then blurring an untouched imported value is lossless and
    // must not manufacture a history entry.
    host.commit_property_focus_if_any();
    assert!(host.editor_state().history.past.is_empty());
    assert_eq!(
        op_editor_core::first_image_fill_summary(host.editor_state().selected_node().unwrap())
            .unwrap()
            .tile_scale,
        Some(0.38618907)
    );
}

#[test]
fn native_image_fill_popover_owns_status_bar_overlap() {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r#"{"version":"1.0.0","children":[{"type":"rectangle","id":"r","fill":[{"type":"image","url":"asset.png","mode":"tile","tileScale":0.5}]}]}"#,
    );
    host.editor_state_mut()
        .set_single_selection(NodeId::new("r"));
    host.editor_state_mut().editor_ui.image_fill_popover_open = true;

    let panel = op_editor_ui::widgets::PropertyPanel::for_selection(host.editor_state()).unwrap();
    let property_rect = host.property_rect(VIEWPORT_W, VIEWPORT_H);
    let status_rect = host.status_bar_rect(VIEWPORT_W, VIEWPORT_H).unwrap();
    let status = op_editor_ui::widgets::StatusBar::for_editor(host.editor_state());
    let point = find_status_bar_popover_overlap(&panel, property_rect, &status, status_rect);
    let zoom_before = host.editor_state().viewport.zoom;

    assert!(host.apply_press(point.x, point.y, VIEWPORT_W, VIEWPORT_H));
    assert!(host.editor_state().editor_ui.image_fill_popover_open);
    assert_eq!(host.editor_state().viewport.zoom, zoom_before);
    assert!(!matches!(
        host.editor_state().editor_ui.pressed_button,
        Some(op_editor_core::ButtonPressTarget::StatusBar(_))
    ));
}

#[test]
fn native_tile_scale_draft_commits_before_popup_actions_and_outside_dismiss() {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r#"{"version":"1.0.0","children":[{"type":"rectangle","id":"r","fill":[{"type":"image","url":"asset.png","mode":"tile","tileScale":0.5}]}]}"#,
    );
    host.editor_state_mut()
        .set_single_selection(NodeId::new("r"));

    // Switching mode hides the Tile-only row, so its draft must land first.
    focus_tile_scale(&mut host, "0.75");
    host.apply_property_action(PropertyPanelAction::SetImageFillMode(
        op_editor_core::ImageFillMode::Fit,
    ));
    assert_eq!(selected_tile_scale(&host), 0.75);
    assert_eq!(host.editor_state().ui.property_focus, None);
    assert!(host.editor_state().ui.property_input.text().is_empty());
    assert!(host.editor_state().ui.property_input_draft.is_empty());

    // The explicit close button follows the same commit-and-cleanup path.
    host.apply_property_action(PropertyPanelAction::SetImageFillMode(
        op_editor_core::ImageFillMode::Tile,
    ));
    focus_tile_scale(&mut host, "0.625");
    host.apply_property_action(PropertyPanelAction::CloseImageFillPopover);
    assert_eq!(selected_tile_scale(&host), 0.625);
    assert!(!host.editor_state().editor_ui.image_fill_popover_open);
    assert_eq!(host.editor_state().ui.property_focus, None);

    // A press outside the floating card is swallowed after committing too.
    focus_tile_scale(&mut host, "0.875");
    let panel = op_editor_ui::widgets::PropertyPanel::for_selection(host.editor_state()).unwrap();
    let rect = host.property_rect(VIEWPORT_W, VIEWPORT_H);
    let outside =
        op_editor_ui::Point2D::new(rect.origin.x + 20.0, rect.origin.y + rect.size.y - 20.0);
    assert!(!panel.image_fill_popover_contains(rect, outside));
    assert!(host.apply_press(outside.x, outside.y, VIEWPORT_W, VIEWPORT_H));
    assert_eq!(selected_tile_scale(&host), 0.875);
    assert!(!host.editor_state().editor_ui.image_fill_popover_open);
    assert_eq!(host.editor_state().ui.property_focus, None);
    assert!(host.editor_state().ui.property_input_draft.is_empty());
}

fn find_tile_scale_input(
    panel: &op_editor_ui::widgets::PropertyPanel,
    rect: op_editor_ui::Rect,
) -> op_editor_ui::Point2D {
    let mut y = rect.origin.y;
    while y < rect.origin.y + rect.size.y {
        let mut x = (rect.origin.x - 320.0).max(0.0);
        while x < rect.origin.x + rect.size.x {
            let point = op_editor_ui::Point2D::new(x, y);
            if panel.image_fill_popover_input_at(rect, point) == Some(PropertyFocus::ImageTileScale)
            {
                return point;
            }
            x += 2.0;
        }
        y += 2.0;
    }
    panic!("tile-scale input is not reachable in the open popover");
}

fn find_status_bar_popover_overlap(
    panel: &op_editor_ui::widgets::PropertyPanel,
    property_rect: op_editor_ui::Rect,
    status: &op_editor_ui::widgets::StatusBar,
    status_rect: op_editor_ui::Rect,
) -> op_editor_ui::Point2D {
    let mut y = status_rect.origin.y;
    while y < status_rect.origin.y + status_rect.size.y {
        let mut x = status_rect.origin.x;
        while x < status_rect.origin.x + status_rect.size.x {
            let point = op_editor_ui::Point2D::new(x, y);
            let closes_popup = matches!(
                panel.hit_test_action(property_rect, point),
                Some(PropertyPanelAction::CloseImageFillPopover)
            );
            if panel.image_fill_popover_contains(property_rect, point)
                && status.control_at(status_rect, point).is_some()
                && !closes_popup
            {
                return point;
            }
            x += 1.0;
        }
        y += 1.0;
    }
    panic!("image-fill popover does not overlap a StatusBar control");
}
