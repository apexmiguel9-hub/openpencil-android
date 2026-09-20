//! One wheel convention for every scroll surface.
//!
//! A positive `delta_y` means the content travelled down (the reader
//! moved up), so a scroll offset must SHRINK. The property-panel font
//! picker used to add the delta instead, which made it the only surface
//! in either host that scrolled backwards — including against the
//! settings font picker, which is the same widget backed by the same
//! `font_picker.scroll` field. These tests pin every rail surface to the
//! same sign so the outlier cannot come back.

use super::WidgetHostNative;
use op_editor_core::NodeId;
use op_editor_ui::widgets::PropertyPanel;
use op_editor_ui::{Point2D, Rect};

const VIEWPORT_W: f32 = 1200.0;
const VIEWPORT_H: f32 = 800.0;
/// Enough of a starting offset that a shrink can't be hidden by the
/// clamp at zero, and enough headroom that a grow can't be hidden by
/// the clamp at `max_scroll`.
const START_OFFSET: f32 = 60.0;
const WHEEL_DELTA: f32 = 40.0;

const TEXT_DOC: &str = r##"{"version":"1.0.0","children":[
    {"type":"text","id":"t1","name":"Label","x":0,"y":0,"width":200,"height":40,
     "content":"Hello","fontSize":16,
     "fill":[{"type":"solid","color":"#111111"}]}
]}"##;

/// A host with a text node selected — the one node kind that shows both
/// the Typography section (font picker) and a Fill section (colour
/// variables), so both rail popups can be exercised on one fixture.
fn host_with_text_selection() -> WidgetHostNative {
    let doc = jian_ops_schema::load_str(TEXT_DOC)
        .expect("fixture parses")
        .value;
    let mut host = WidgetHostNative::new();
    *host.editor_state_mut() = op_editor_core::EditorState::from_document(doc);
    host.editor_state_mut()
        .set_single_selection(NodeId::new("t1"));
    host
}

fn property_rect() -> Rect {
    let width = 280.0;
    Rect {
        origin: Point2D::new(VIEWPORT_W - width, op_editor_ui::widgets::TOP_BAR_HEIGHT),
        size: Point2D::new(
            width,
            (VIEWPORT_H - op_editor_ui::widgets::TOP_BAR_HEIGHT).max(0.0),
        ),
    }
}

/// Seed enough system families that the font list overflows its capped
/// viewport and therefore has a scroll range to move.
fn seed_many_fonts(host: &mut WidgetHostNative) {
    let families: Vec<String> = (0..80).map(|i| format!("Test Family {i:03}")).collect();
    host.editor_state_mut().editor_ui.system_font_families = std::sync::Arc::new(families);
    host.editor_state_mut().editor_ui.system_fonts_loaded = true;
}

/// Walk down the rail's centre line looking for a point the predicate
/// accepts. Avoids duplicating popup geometry in the test.
fn point_inside(rect: Rect, mut hit: impl FnMut(Point2D) -> bool) -> Point2D {
    let x = rect.origin.x + rect.size.x / 2.0;
    let mut y = rect.origin.y + 4.0;
    let bottom = rect.origin.y + rect.size.y;
    while y < bottom {
        let point = Point2D::new(x, y);
        if hit(point) {
            return point;
        }
        y += 4.0;
    }
    panic!("no point on the rail centre line landed inside the popup");
}

/// The property-panel font picker — the surface that was inverted.
#[test]
fn font_picker_wheel_shrinks_offset_on_positive_delta() {
    let mut host = host_with_text_selection();
    seed_many_fonts(&mut host);
    host.editor_state_mut().editor_ui.toggle_font_picker();
    assert!(host.editor_state().editor_ui.font_picker.open);

    let rect = property_rect();
    let panel = PropertyPanel::for_selection(host.editor_state()).expect("text panel");
    let max = panel.font_picker_max_scroll(rect);
    assert!(
        max > START_OFFSET,
        "fixture must overflow the font list (max_scroll {max})"
    );
    let point = point_inside(rect, |p| panel.font_picker_contains(rect, p));
    drop(panel);

    host.editor_state_mut().editor_ui.font_picker.scroll.offset = START_OFFSET;
    assert!(host.apply_wheel(point.x, point.y, WHEEL_DELTA, VIEWPORT_W, VIEWPORT_H));
    assert!(
        host.editor_state().editor_ui.font_picker.scroll.offset < START_OFFSET,
        "a positive wheel delta must shrink the font picker's offset, got {}",
        host.editor_state().editor_ui.font_picker.scroll.offset
    );

    host.editor_state_mut().editor_ui.font_picker.scroll.offset = START_OFFSET;
    assert!(host.apply_wheel(point.x, point.y, -WHEEL_DELTA, VIEWPORT_W, VIEWPORT_H));
    assert!(
        host.editor_state().editor_ui.font_picker.scroll.offset > START_OFFSET,
        "a negative wheel delta must grow the font picker's offset"
    );
}

/// The font picker and the colour-variable picker sit on the same rail
/// and route through the same `try_scroll_property_panel`. For one wheel
/// delta they must move the same way.
#[test]
fn rail_popups_agree_on_wheel_direction() {
    // Colour-variable popup leg.
    let mut host = host_with_text_selection();
    for i in 0..40 {
        assert!(host.editor_state_mut().create_variable(
            &format!("color-{i:02}"),
            jian_ops_schema::variable::VariableKind::Color,
            jian_ops_schema::variable::VariableScalar::Str("#DBD8CB".into()),
        ));
    }
    host.editor_state_mut()
        .editor_ui
        .property_color_variable_picker_open = Some(op_editor_core::ColorTarget::Fill);

    let rect = property_rect();
    let panel = PropertyPanel::for_selection(host.editor_state()).expect("text panel");
    let layout = panel
        .color_variable_picker_layout(rect)
        .expect("open colour-variable popup");
    assert!(
        layout.max_scroll > START_OFFSET,
        "fixture must overflow the variable list"
    );
    let variable_point = Point2D::new(
        layout.popup.origin.x + layout.popup.size.x / 2.0,
        layout.popup.origin.y + layout.popup.size.y / 2.0,
    );
    drop(panel);

    host.editor_state_mut()
        .editor_ui
        .property_color_variable_picker_scroll
        .offset = START_OFFSET;
    assert!(host.apply_wheel(
        variable_point.x,
        variable_point.y,
        WHEEL_DELTA,
        VIEWPORT_W,
        VIEWPORT_H
    ));
    let variable_delta = host
        .editor_state()
        .editor_ui
        .property_color_variable_picker_scroll
        .offset
        - START_OFFSET;

    // Font-picker leg, same delta.
    let mut host = host_with_text_selection();
    seed_many_fonts(&mut host);
    host.editor_state_mut().editor_ui.toggle_font_picker();
    let panel = PropertyPanel::for_selection(host.editor_state()).expect("text panel");
    let font_point = point_inside(rect, |p| panel.font_picker_contains(rect, p));
    drop(panel);
    host.editor_state_mut().editor_ui.font_picker.scroll.offset = START_OFFSET;
    assert!(host.apply_wheel(
        font_point.x,
        font_point.y,
        WHEEL_DELTA,
        VIEWPORT_W,
        VIEWPORT_H
    ));
    let font_delta = host.editor_state().editor_ui.font_picker.scroll.offset - START_OFFSET;

    assert!(
        variable_delta != 0.0 && font_delta != 0.0,
        "both popups must actually move (variable {variable_delta}, font {font_delta})"
    );
    assert!(
        variable_delta.signum() == font_delta.signum(),
        "rail popups disagree on wheel direction: variable {variable_delta}, font {font_delta}"
    );
}

/// …and both agree with the inspector they float above.
#[test]
fn rail_popups_agree_with_the_panel_behind_them() {
    let mut host = host_with_text_selection();
    let rect = property_rect();
    let panel = PropertyPanel::for_selection(host.editor_state()).expect("text panel");
    let max = (panel.content_height(rect) - rect.size.y).max(0.0);
    drop(panel);
    assert!(
        max > START_OFFSET,
        "fixture must give the inspector a scroll range (max {max})"
    );

    host.editor_state_mut()
        .editor_ui
        .property_panel_scroll
        .offset = START_OFFSET;
    let point = Point2D::new(
        rect.origin.x + rect.size.x / 2.0,
        rect.origin.y + rect.size.y - 20.0,
    );
    assert!(host.apply_wheel(point.x, point.y, WHEEL_DELTA, VIEWPORT_W, VIEWPORT_H));
    let panel_delta = host.editor_state().editor_ui.property_panel_scroll.offset - START_OFFSET;
    assert!(
        panel_delta < 0.0,
        "a positive wheel delta must shrink the inspector's offset, got {panel_delta}"
    );
}
