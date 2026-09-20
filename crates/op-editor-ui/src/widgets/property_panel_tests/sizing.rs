//! Size / layout geometry tests — fill + hug sizing keeping their
//! numeric inputs, the padding edit-mode input count, and the flex
//! section's gap-mode column not colliding with the padding rows.
//!
//! Split out of `property_panel_tests.rs` to keep both files under
//! the openpencil 800-line cap.

use crate::widgets::property_panel::{PropertyPanel, PropertyPanelAction};
use crate::widgets::property_panel_sections as sections;
use crate::widgets::property_panel_test_support::{state_from, visible_for, CountingBackend};
use crate::widgets::{PaintCx, Widget};
use crate::{Point2D, Rect};
use op_editor_core::NodeId;

#[test]
fn fill_width_keeps_both_numeric_inputs_visible_and_hittable() {
    use op_editor_core::PropertyFocus;
    let fill = {
        let mut s = state_from(
            r##"{ "version": "1.0.0", "children": [
                  {"type":"frame","id":"ff","name":"Frame",
                   "x":40,"y":40,"width":"fill_container","height":240,
                   "layout":"vertical","children":[]}
            ]}"##,
        );
        s.set_single_selection(NodeId::new("ff"));
        PropertyPanel::for_selection(&s).expect("fill-width frame panel")
    };
    assert!(fill.snapshot.size_fill_width, "width sizing should be fill");
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(280.0, 1200.0),
    };
    let fill_rects = sections::editable_input_rects(
        rect,
        visible_for(&fill),
        &fill.snapshot.fills,
        &fill.snapshot.effects,
    );
    for focus in [PropertyFocus::SizeW, PropertyFocus::SizeH] {
        let target = fill_rects
            .iter()
            .find(|(candidate, _)| *candidate == focus)
            .map(|(_, rect)| *rect)
            .expect("fill sizing must keep the numeric input");
        let center = Point2D::new(
            target.origin.x + target.size.x / 2.0,
            target.origin.y + target.size.y / 2.0,
        );
        assert_eq!(fill.hit_test(rect, center), Some(focus));
    }
}

#[test]
fn both_dimensions_fill_keep_the_size_input_row() {
    use op_editor_core::PropertyFocus;
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(280.0, 1200.0),
    };
    let panel_for = |w: &str, h: &str| {
        let json = format!(
            r##"{{ "version": "1.0.0", "children": [
                  {{"type":"frame","id":"ff","name":"Frame",
                   "x":40,"y":40,"width":{w},"height":{h},
                   "layout":"vertical","children":[]}}
            ]}}"##
        );
        let mut s = state_from(&json);
        s.set_single_selection(NodeId::new("ff"));
        PropertyPanel::for_selection(&s).expect("frame panel")
    };
    let chk_y = |p: &PropertyPanel| {
        sections::action_button_rects_with_fill_picker(
            rect,
            visible_for(p),
            &p.snapshot.effects,
            &p.snapshot.fills,
            &p.snapshot.interactions,
            false,
            0,
            false,
            false,
            false,
            false,
            false,
        )
        .into_iter()
        .find(|(a, _)| matches!(a, PropertyPanelAction::ToggleSizeFillWidth))
        .map(|(_, r)| r.origin.y)
        .expect("fill-width checkbox rect")
    };
    let one = panel_for("\"fill_container\"", "240");
    let both = panel_for("\"fill_container\"", "\"fill_container\"");
    assert!(one.snapshot.size_fill_width && both.snapshot.size_fill_height);
    assert!(
        (chk_y(&one) - chk_y(&both)).abs() < 0.01,
        "checkbox rows must not jump when both axes use fill"
    );
    let both_inputs = sections::editable_input_rects(
        rect,
        visible_for(&both),
        &both.snapshot.fills,
        &both.snapshot.effects,
    );
    for focus in [PropertyFocus::SizeW, PropertyFocus::SizeH] {
        assert!(
            both_inputs.iter().any(|(candidate, _)| *candidate == focus),
            "both fill axes must still emit the numeric hit rect"
        );
    }
}

#[test]
fn fill_and_hug_inputs_paint_snapshot_size_and_numeric_commit_makes_fixed() {
    use op_editor_core::PropertyFocus;
    let mut state = state_from(
        r##"{ "version": "1.0.0", "children": [
              {"type":"frame","id":"ff","name":"Frame",
               "x":40,"y":40,"width":"fill_container","height":"fit_content",
               "layout":"vertical","children":[
                 {"type":"rectangle","id":"child","x":0,"y":0,
                  "width":180,"height":90}
               ]}
        ]}"##,
    );
    state.set_single_selection(NodeId::new("ff"));
    let panel = PropertyPanel::for_selection(&state).expect("fill/hug frame panel");
    assert_eq!((panel.snapshot.width, panel.snapshot.height), (180, 90));
    assert!(panel.snapshot.size_fill_width && panel.snapshot.size_hug_height);

    let mut backend = CountingBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    panel.paint(
        &mut cx,
        Rect {
            origin: Point2D::ZERO,
            size: Point2D::new(280.0, 1200.0),
        },
    );
    assert!(backend.texts.iter().any(|text| text == "180"));
    assert!(backend.texts.iter().any(|text| text == "90"));
    assert!(state.commit_property_edit(PropertyFocus::SizeW, 220.0));
    assert!(state.commit_property_edit(PropertyFocus::SizeH, 130.0));
    let fixed = PropertyPanel::for_selection(&state).expect("fixed frame panel");
    assert_eq!((fixed.snapshot.width, fixed.snapshot.height), (220, 130));
    assert!(!fixed.snapshot.size_fill_width && !fixed.snapshot.size_hug_height);
}

#[test]
fn padding_mode_derives_from_values_and_drives_input_count() {
    use op_editor_core::{PaddingEditMode, PropertyFocus};
    // from_values mirrors TS parsePaddingValues.
    assert_eq!(
        PaddingEditMode::from_values(10.0, 10.0, 10.0, 10.0),
        PaddingEditMode::Single
    );
    assert_eq!(
        PaddingEditMode::from_values(10.0, 20.0, 10.0, 20.0),
        PaddingEditMode::Axis
    );
    assert_eq!(
        PaddingEditMode::from_values(8.0, 24.0, 32.0, 24.0),
        PaddingEditMode::Individual
    );

    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(280.0, 1200.0),
    };
    let padding_rects = |padding: &str| {
        let json = format!(
            r##"{{ "version": "1.0.0", "children": [
                  {{"type":"frame","id":"f","name":"F","x":0,"y":0,
                   "width":300,"height":200,"layout":"vertical",
                   "padding":{padding},"children":[]}}
            ]}}"##
        );
        let mut s = state_from(&json);
        s.set_single_selection(NodeId::new("f"));
        let panel = PropertyPanel::for_selection(&s).expect("frame panel");
        sections::editable_input_rects(
            rect,
            visible_for(&panel),
            &panel.snapshot.fills,
            &panel.snapshot.effects,
        )
        .into_iter()
        .filter(|(f, _)| {
            matches!(
                f,
                PropertyFocus::PaddingTop
                    | PropertyFocus::PaddingRight
                    | PropertyFocus::PaddingBottom
                    | PropertyFocus::PaddingLeft
            )
        })
        .count()
    };
    // Single → 1 input, Axis → 2, Individual → 4.
    assert_eq!(padding_rects("12"), 1, "uniform padding → 1 input");
    assert_eq!(padding_rects("[10, 20]"), 2, "axis padding → 2 inputs");
    assert_eq!(
        padding_rects("[8, 24, 32, 24]"),
        4,
        "individual padding → 4 inputs"
    );
}

#[test]
fn flex_advanced_rows_do_not_overlap_gap_modes() {
    let mut state = state_from(
        r##"{ "version": "1.0.0", "children": [
              {"type":"frame","id":"f","name":"Frame",
               "x":40,"y":40,"width":360,"height":240,
               "layout":"horizontal","gap":0,
               "children":[]}
        ]}"##,
    );
    state.set_single_selection(NodeId::new("f"));
    let panel = PropertyPanel::for_selection(&state).expect("frame panel");
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(280.0, 1200.0),
    };
    let visible = visible_for(&panel);
    let actions = sections::action_button_rects_with_fill_picker(
        rect,
        visible,
        &panel.snapshot.effects,
        &panel.snapshot.fills,
        &panel.snapshot.interactions,
        false,
        0,
        false,
        false,
        false,
        false,
        false,
    );
    let last_gap_mode = actions
        .iter()
        .find(|(action, _)| {
            matches!(
                action,
                PropertyPanelAction::SetLayoutJustify(
                    crate::widgets::property_panel::LayoutJustifyValue::SpaceAround
                )
            )
        })
        .map(|(_, r)| *r)
        .expect("space-around hit rect");
    let padding_top = sections::editable_input_rects(
        rect,
        visible,
        &panel.snapshot.fills,
        &panel.snapshot.effects,
    )
    .into_iter()
    .find(|(focus, _)| *focus == op_editor_core::PropertyFocus::PaddingTop)
    .map(|(_, r)| r)
    .expect("padding top input rect");

    assert!(
        padding_top.origin.y >= last_gap_mode.origin.y + last_gap_mode.size.y + 18.0,
        "padding inputs must start below the full gap-mode column"
    );
}
