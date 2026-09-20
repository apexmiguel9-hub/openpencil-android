//! PropertyPanel + image-popover input editing: focus seeding, commits,
//! step/delete/escape, and the Backspace-vs-delete-selection guards.
//!
//! Split out of `input_tests.rs` to keep every file under the repo's
//! 800-line cap.

use super::*;

#[test]
fn pick_fill_image_keeps_image_popover_open_for_mode_selection() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut().editor_ui.image_fill_popover_open = true;

    host.apply_property_action(op_editor_ui::widgets::PropertyPanelAction::PickFillImage);

    assert_eq!(
        host.editor_state().editor_ui.pending_file_action,
        Some(op_editor_core::editor_ui_state::FileAction::PickFillImage),
    );
    assert!(
        host.editor_state().editor_ui.image_fill_popover_open,
        "the image popover must stay open so Fill/Fit/Crop/Tile remain selectable",
    );
}

#[test]
fn image_adjustment_drag_updates_live_after_press() {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r##"{ "version": "1.0.0", "children": [
              {"type":"rectangle","id":"n60","name":"Photo fill",
               "x":40,"y":40,"width":180,"height":120,
               "fill":[{"type":"image","url":"","mode":"fill",
                 "exposure":0,"contrast":0,"saturation":0,
                 "temperature":0,"tint":0,"highlights":0,"shadows":0}]}
        ]}"##,
    );
    host.editor_state_mut()
        .set_single_selection(NodeId::new("n60"));
    host.editor_state_mut().editor_ui.image_fill_popover_open = true;
    host.image_adjustment_drag = Some(op_editor_core::ImageAdjustmentField::Exposure);
    host.last_viewport_w = 900.0;
    host.last_viewport_h = 760.0;

    assert!(host.apply_cursor_move(0.0, 0.0));

    let node = host
        .editor_state()
        .selected_node()
        .expect("selected image-fill node");
    match op_editor_core::fills::node_fills(node)
        .unwrap()
        .first()
        .unwrap()
    {
        jian_ops_schema::style::PenFill::Image(body) => {
            assert_eq!(body.exposure, Some(-100.0));
        }
        other => panic!("expected image fill, got {other:?}"),
    }
}

#[test]
fn image_fill_actions_refresh_the_render_scene() {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r##"{ "version": "1.0.0", "children": [
              {"type":"rectangle","id":"n61","name":"Photo fill",
               "x":40,"y":40,"width":180,"height":120,
               "fill":[{"type":"image","url":"data:image/png;base64,AA==","mode":"fill",
                 "exposure":0,"contrast":0,"saturation":0,
                 "temperature":0,"tint":0,"highlights":0,"shadows":0}]}
        ]}"##,
    );
    host.editor_state_mut()
        .set_single_selection(NodeId::new("n61"));
    host.mark_paint_dirty_for_test();

    let initial_fit = host
        .layout_scene()
        .active_page()
        .unwrap()
        .find("n61")
        .unwrap()
        .image_fit;
    assert_eq!(initial_fit, op_editor_ui::layout_scene::SceneImageFit::Fill);

    host.apply_property_action(
        op_editor_ui::widgets::PropertyPanelAction::SetImageFillMode(
            op_editor_core::ImageFillMode::Fit,
        ),
    );
    host.apply_property_action(
        op_editor_ui::widgets::PropertyPanelAction::SetImageAdjustment {
            field: op_editor_core::ImageAdjustmentField::Exposure,
            value: 64.0,
        },
    );

    let rendered = host
        .layout_scene()
        .active_page()
        .unwrap()
        .find("n61")
        .unwrap();
    assert_eq!(
        rendered.image_fit,
        op_editor_ui::layout_scene::SceneImageFit::Fit
    );
    assert_eq!(rendered.image_adjustments.exposure, 64.0);
}

#[test]
fn corner_radius_property_focus_updates_selected_rectangle() {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r##"{ "version": "1.0.0", "children": [
              {"type":"rectangle","id":"n62","name":"Rounded",
               "x":40,"y":40,"width":180,"height":120,
               "fill":[{"type":"solid","color":"#BDC7D9"}]}
        ]}"##,
    );
    host.editor_state_mut()
        .set_single_selection(NodeId::new("n62"));
    host.editor_state_mut().ui.property_focus = Some(PropertyFocus::PositionR);
    host.editor_state_mut().ui.property_input.set_text("24");

    host.commit_property_focus_if_any();

    let node = host.editor_state().selected_node().unwrap();
    match node {
        jian_ops_schema::node::PenNode::Rectangle(rect) => {
            assert_eq!(
                rect.container.corner_radius,
                Some(jian_ops_schema::node::container::CornerRadius::Uniform(
                    24.0
                )),
            );
        }
        other => panic!("expected rectangle, got {other:?}"),
    }
    let rendered = host
        .layout_scene()
        .active_page()
        .unwrap()
        .find("n62")
        .unwrap();
    assert_eq!(rendered.corner_radius, 24.0);
}

#[test]
fn property_focus_commit_reads_text_input_state() {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r##"{ "version": "1.0.0", "children": [
              {"type":"rectangle","id":"n62","name":"Wide",
               "x":40,"y":40,"width":180,"height":120,
               "fill":[{"type":"solid","color":"#BDC7D9"}]}
        ]}"##,
    );
    host.editor_state_mut()
        .set_single_selection(NodeId::new("n62"));
    host.editor_state_mut().ui.property_focus = Some(PropertyFocus::SizeW);
    host.editor_state_mut().ui.property_input.set_text("321");

    host.commit_property_focus_if_any();

    let bounds = own_bounds(host.editor_state().selected_node().unwrap());
    assert_eq!(bounds.w, 321.0);
    assert!(host.editor_state().ui.property_input.text().is_empty());
}

#[test]
fn widget_leading_icon_focus_commits_onto_text_input() {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r##"{ "version": "1.0.0", "children": [
              {"type":"text_input","id":"email","name":"Email",
               "x":24,"y":32,"width":220,"height":40,"placeholder":"Email"}
        ]}"##,
    );
    host.editor_state_mut()
        .set_single_selection(NodeId::new("email"));
    host.editor_state_mut().ui.property_focus = Some(PropertyFocus::WidgetLeadingIcon);
    // Type the glyph name char-by-char to exercise the free-text gate
    // (letters must NOT be rejected as non-numeric).
    for c in "mail".chars() {
        assert!(host.apply_text(c), "letter '{c}' must be accepted");
    }
    host.commit_property_focus_if_any();

    match host.editor_state().selected_node().unwrap() {
        jian_ops_schema::node::PenNode::TextInput(t) => {
            assert_eq!(t.leading_icon.as_deref(), Some("mail"));
        }
        other => panic!("expected text_input, got {other:?}"),
    }
}

#[test]
fn widget_bind_key_focus_writes_state_binding() {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r##"{ "version": "1.0.0", "children": [
              {"type":"text_input","id":"email","name":"Email",
               "x":24,"y":32,"width":220,"height":40,"placeholder":"Email"}
        ]}"##,
    );
    host.editor_state_mut()
        .set_single_selection(NodeId::new("email"));
    host.editor_state_mut().ui.property_focus = Some(PropertyFocus::WidgetBindKey);
    host.editor_state_mut().ui.property_input.set_text("email");
    host.commit_property_focus_if_any();

    match host.editor_state().selected_node().unwrap() {
        jian_ops_schema::node::PenNode::TextInput(t) => {
            let bindings = t.bindings.as_ref().expect("bindings written");
            assert_eq!(
                bindings.get("bind:value").map(|e| e.0.as_str()),
                Some("$state.email"),
            );
        }
        other => panic!("expected text_input, got {other:?}"),
    }
}

#[test]
fn property_focus_commit_is_undoable() {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r##"{ "version": "1.0.0", "children": [
              {"type":"rectangle","id":"n62","name":"Wide",
               "x":40,"y":40,"width":180,"height":120,
               "fill":[{"type":"solid","color":"#BDC7D9"}]}
        ]}"##,
    );
    host.editor_state_mut()
        .set_single_selection(NodeId::new("n62"));
    host.editor_state_mut().ui.property_focus = Some(PropertyFocus::SizeW);
    host.editor_state_mut().ui.property_input.set_text("321");

    host.commit_property_focus_if_any();

    let bounds = own_bounds(host.editor_state().selected_node().unwrap());
    assert_eq!(bounds.w, 321.0);
    assert!(host.editor_state().history.can_undo());

    assert!(host.editor_state_mut().undo());
    let bounds = own_bounds(host.editor_state().selected_node().unwrap());
    assert_eq!(bounds.w, 180.0);
}

#[test]
fn property_press_seeds_text_input_state() {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r##"{ "version": "1.0.0", "children": [
              {"type":"rectangle","id":"n62","name":"Wide",
               "x":40,"y":40,"width":180,"height":120,
               "fill":[{"type":"solid","color":"#BDC7D9"}]}
        ]}"##,
    );
    host.editor_state_mut()
        .set_single_selection(NodeId::new("n62"));

    let viewport_w = 1200.0;
    let viewport_h = 800.0;
    let property_rect = host.property_rect(viewport_w, viewport_h);
    let panel = op_editor_ui::widgets::PropertyPanel::for_selection(host.editor_state())
        .expect("selection should show the property panel");
    let mut point = None;
    'outer: for y in 0..property_rect.size.y as i32 {
        for x in 0..property_rect.size.x as i32 {
            let candidate = op_editor_ui::Point2D::new(
                property_rect.origin.x + x as f32 + 0.5,
                property_rect.origin.y + y as f32 + 0.5,
            );
            if panel.hit_test(property_rect, candidate) == Some(PropertyFocus::SizeW) {
                point = Some(candidate);
                break 'outer;
            }
        }
    }
    let point = point.expect("width input should be hit-testable");

    assert!(host.apply_press(point.x, point.y, viewport_w, viewport_h));

    assert_eq!(
        host.editor_state().ui.property_focus,
        Some(PropertyFocus::SizeW)
    );
    assert_eq!(host.editor_state().ui.property_input.text(), "180");
    assert_eq!(host.editor_state().ui.property_input.caret(), 3);
}

#[test]
fn effect_param_focus_seeds_text_input_state() {
    let mut host = WidgetHostNative::new();

    host.apply_property_action(
        op_editor_ui::widgets::PropertyPanelAction::FocusEffectParam {
            effect: 0,
            field: op_editor_core::EffectField::OffsetX,
            value: 12.5,
        },
    );

    assert_eq!(
        host.editor_state().editor_ui.effect_param_focus,
        Some(op_editor_core::editor_ui_state::EffectParamFocus {
            effect: 0,
            field: op_editor_core::EffectField::OffsetX,
        })
    );
    assert_eq!(host.editor_state().ui.property_input.text(), "12.5");
    assert_eq!(host.editor_state().ui.property_input.caret(), 4);
}

#[test]
fn effect_param_input_uses_text_input_state_for_editing() {
    let mut host = WidgetHostNative::new();
    {
        let editor = host.editor_state_mut();
        editor.editor_ui.effect_param_focus =
            Some(op_editor_core::editor_ui_state::EffectParamFocus {
                effect: 0,
                field: op_editor_core::EffectField::OffsetX,
            });
        editor.ui.property_input.set_text("1234");
    }

    assert!(host.apply_property_caret(false));
    assert!(host.apply_property_caret(false));
    assert_eq!(host.editor_state().ui.property_input.caret(), 2);

    assert!(host.apply_text('9'));
    assert_eq!(host.editor_state().ui.property_input.text(), "12934");
    assert_eq!(host.editor_state().ui.property_input.caret(), 3);
}

#[test]
fn property_delete_uses_text_input_state() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut()
        .set_single_selection(NodeId::new("n10"));
    {
        let ui = &mut host.editor_state_mut().ui;
        ui.property_focus = Some(PropertyFocus::PositionX);
        ui.property_input.set_text("123");
    }
    assert!(host.apply_property_caret(false));

    assert!(host.apply_delete());

    assert_eq!(host.editor_state().ui.property_input.text(), "12");
    assert_eq!(host.editor_state().selection.anchor, NodeId::new("n10"));
}

#[test]
fn property_step_reads_and_updates_text_input_state() {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r##"{ "version": "1.0.0", "children": [
              {"type":"rectangle","id":"n62","name":"Wide",
               "x":40,"y":40,"width":180,"height":120,
               "fill":[{"type":"solid","color":"#BDC7D9"}]}
        ]}"##,
    );
    host.editor_state_mut()
        .set_single_selection(NodeId::new("n62"));
    {
        let ui = &mut host.editor_state_mut().ui;
        ui.property_focus = Some(PropertyFocus::SizeW);
        ui.property_input.set_text("180");
    }

    assert!(host.apply_property_step(5.0));

    let bounds = own_bounds(host.editor_state().selected_node().unwrap());
    assert_eq!(bounds.w, 185.0);
    assert_eq!(host.editor_state().ui.property_input.text(), "185");
    assert_eq!(host.editor_state().ui.property_input.caret(), 3);
}

#[test]
fn property_escape_clears_text_input_state() {
    let mut host = WidgetHostNative::new();
    {
        let ui = &mut host.editor_state_mut().ui;
        ui.property_focus = Some(PropertyFocus::PositionX);
        ui.property_input.set_text("123");
    }

    assert!(host.apply_escape());

    assert!(host.editor_state().ui.property_focus.is_none());
    assert!(host.editor_state().ui.property_input.text().is_empty());
}

#[test]
fn polygon_sides_property_focus_updates_selected_polygon() {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r##"{ "version": "1.0.0", "children": [
              {"type":"polygon","id":"poly","name":"Polygon",
               "x":40,"y":40,"width":120,"height":120,
               "polygonCount":3}
        ]}"##,
    );
    host.editor_state_mut()
        .set_single_selection(NodeId::new("poly"));
    host.editor_state_mut().ui.property_focus = Some(PropertyFocus::PolygonSides);
    host.editor_state_mut().ui.property_input.set_text("7");

    host.commit_property_focus_if_any();

    let node = host.editor_state().selected_node().unwrap();
    match node {
        jian_ops_schema::node::PenNode::Polygon(poly) => {
            assert_eq!(poly.polygon_count, 7);
        }
        other => panic!("expected polygon, got {other:?}"),
    }
    let rendered = host
        .layout_scene()
        .active_page()
        .unwrap()
        .find("poly")
        .unwrap();
    assert_eq!(rendered.polygon_sides, 7);
}

#[test]
fn ellipse_arc_property_focus_updates_selected_ellipse() {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r##"{ "version": "1.0.0", "children": [
              {"type":"ellipse","id":"ell","name":"Ellipse",
               "x":40,"y":40,"width":120,"height":100}
        ]}"##,
    );
    host.editor_state_mut()
        .set_single_selection(NodeId::new("ell"));

    host.editor_state_mut().ui.property_focus = Some(PropertyFocus::EllipseStart);
    host.editor_state_mut().ui.property_input.set_text("45");
    host.commit_property_focus_if_any();

    host.editor_state_mut().ui.property_focus = Some(PropertyFocus::EllipseSweep);
    host.editor_state_mut().ui.property_input.set_text("180");
    host.commit_property_focus_if_any();

    host.editor_state_mut().ui.property_focus = Some(PropertyFocus::EllipseInnerRadius);
    host.editor_state_mut().ui.property_input.set_text("25");
    host.commit_property_focus_if_any();

    let node = host.editor_state().selected_node().unwrap();
    match node {
        jian_ops_schema::node::PenNode::Ellipse(ell) => {
            assert_eq!(ell.start_angle, Some(45.0));
            assert_eq!(ell.sweep_angle, Some(180.0));
            assert_eq!(ell.inner_radius, Some(0.25));
        }
        other => panic!("expected ellipse, got {other:?}"),
    }
    let rendered = host
        .layout_scene()
        .active_page()
        .unwrap()
        .find("ell")
        .unwrap();
    assert_eq!(rendered.arc_start_angle, Some(45.0));
    assert_eq!(rendered.arc_sweep_angle, Some(180.0));
    assert_eq!(rendered.arc_inner_radius, Some(0.25));
}

#[test]
fn backspace_with_property_input_does_not_delete_selected() {
    // With a non-empty property input, Backspace must pop a char
    // from the input, not delete the selected node.
    let mut host = WidgetHostNative::new();
    host.editor_state_mut()
        .set_single_selection(NodeId::new("n10"));
    host.editor_state_mut().ui.property_focus = Some(PropertyFocus::PositionX);
    host.editor_state_mut().ui.property_input.set_text("123");
    // Caret at the input's end, as a real focus seeds it — Backspace
    // deletes the char *before* the caret.

    assert!(host.apply_backspace());
    assert_eq!(host.editor_state().ui.property_input.text(), "12");
    assert_eq!(host.editor_state().ui.property_input.caret(), 2);
    // Selection must be untouched.
    assert_eq!(host.editor_state().selection.anchor, NodeId::new("n10"));
}

#[test]
fn backspace_without_focus_deletes_selected() {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut()
        .set_single_selection(NodeId::new("n10"));
    host.editor_state_mut().ui.property_focus = None;
    host.editor_state_mut().chat.focused = false;

    assert!(host.apply_backspace());
    assert!(host.editor_state().selection.is_empty());
}

#[test]
fn delete_with_model_picker_open_does_not_delete_selected() {
    // The chat model-picker search owns the keyboard while open, so
    // Delete must be swallowed instead of dropping the canvas node
    // behind the dropdown.
    let mut host = WidgetHostNative::new();
    host.editor_state_mut()
        .set_single_selection(NodeId::new("n10"));
    host.editor_state_mut().editor_ui.chat_model_picker.open = true;

    assert!(!host.apply_delete());
    assert_eq!(host.editor_state().selection.anchor, NodeId::new("n10"));
}

#[test]
fn backspace_with_model_picker_open_edits_search_not_selection() {
    // Backspace pops from the model-picker search query, never the
    // selected node.
    let mut host = WidgetHostNative::new();
    host.editor_state_mut()
        .set_single_selection(NodeId::new("n10"));
    host.editor_state_mut().editor_ui.chat_model_picker.open = true;
    host.editor_state_mut()
        .editor_ui
        .chat_model_picker_input
        .set_text("gp");

    assert!(host.apply_backspace());
    assert_eq!(
        host.editor_state().editor_ui.chat_model_picker_input.text(),
        "g"
    );
    assert_eq!(host.editor_state().selection.anchor, NodeId::new("n10"));
}

#[test]
fn shortcuts_gated_while_model_picker_open() {
    // Nudge / duplicate / reorder all route through `input_active`,
    // which must report the model picker as owning the keyboard.
    let mut host = WidgetHostNative::new();
    host.editor_state_mut()
        .set_single_selection(NodeId::new("n10"));
    host.editor_state_mut().editor_ui.chat_model_picker.open = true;

    assert!(!host.apply_nudge(1.0, 0.0));
    assert!(!host.apply_duplicate());
    assert!(!host.apply_reorder(op_editor_core::ReorderDirection::Up));
    assert_eq!(host.editor_state().selection.anchor, NodeId::new("n10"));
}
