//! Inline select-popup tests (font family, export scale) plus the
//! hex-formatting / stroke-swatch default regressions.
//!
//! Split out of `property_panel_tests.rs` to keep both files under
//! the openpencil 800-line cap.

use crate::widgets::property_panel::{PropertyPanel, PropertyPanelAction};
use crate::widgets::property_panel_sections as sections;
use crate::widgets::property_panel_test_support::visible_for;
use crate::{Color, Point2D, Rect};
use op_editor_core::{EditorState, NodeId};

#[test]
fn font_family_picker_rows_are_clickable() {
    let mut state = EditorState::sample();
    state.set_single_selection(NodeId::new("n11"));
    state.editor_ui.font_picker.open = true;
    state.editor_ui.font_picker_purpose = Some(op_editor_core::FontPickerPurpose::PropertyText);
    // Type-ahead narrows the overlay to one row (TS search filter) —
    // "geor" leaves only the fallback-system "Georgia".
    state.editor_ui.font_picker_search = "geor".to_string();
    let panel = PropertyPanel::for_selection(&state).expect("text panel");
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(280.0, 1200.0),
    };
    let entries = panel.font_picker_entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].family, "Georgia");
    let layout = crate::widgets::property_panel_typography::font_picker_layout(
        rect,
        panel.visible_sections_for_test(),
        &entries,
        panel.font_import_supported,
        0.0,
    )
    .expect("picker layout");
    let georgia = layout
        .rows
        .iter()
        .find_map(|(row, r)| {
            matches!(
                row,
                crate::widgets::property_panel_typography::FontPickerRow::Entry(0)
            )
            .then_some(*r)
        })
        .expect("Georgia font row");
    let center = Point2D::new(
        georgia.origin.x + georgia.size.x / 2.0,
        georgia.origin.y + georgia.size.y / 2.0,
    );
    assert!(matches!(
        panel.hit_test_action(rect, center),
        Some(PropertyPanelAction::SetFontFamilyIndex(0))
    ));
}

#[test]
fn export_scale_picker_open_emits_option_rows() {
    let mut state = EditorState::sample();
    state.set_single_selection(NodeId::new("n10"));
    // Opening the scale picker makes the option rows part of the
    // panel's hit surface.
    state.editor_ui.export_scale_picker_open = true;
    let panel = PropertyPanel::for_selection(&state).expect("frame panel");
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(280.0, 1600.0),
    };
    let rects = sections::action_button_rects_with_fill_picker(
        rect,
        visible_for(&panel),
        &panel.snapshot.effects,
        &panel.snapshot.fills,
        &panel.snapshot.interactions,
        false,
        0,
        false,
        false,
        true,
        false,
        false,
    );
    let rows: Vec<_> = rects
        .iter()
        .filter(|(a, _)| matches!(a, PropertyPanelAction::SetExportScale(_)))
        .collect();
    assert_eq!(rows.len(), 3, "open scale picker emits 1x/2x/3x rows");
    // A click on an option row wins over the dropdown toggle it
    // overlaps — `hit_test_action` walks the rects in `rev()`.
    let row = rows[0].1;
    let row_center = Point2D::new(
        row.origin.x + row.size.x / 2.0,
        row.origin.y + row.size.y / 2.0,
    );
    assert!(
        matches!(
            panel.hit_test_action(rect, row_center),
            Some(PropertyPanelAction::SetExportScale(_))
        ),
        "click on a picker row resolves to SetExportScale",
    );
}

#[test]
fn format_color_hex_pads_to_six_chars() {
    use crate::widgets::property_panel_inputs::format_color_hex;
    assert_eq!(format_color_hex(Color::WHITE), "#FFFFFF");
    assert_eq!(format_color_hex(Color::BLACK), "#000000");
    assert_eq!(format_color_hex(Color::RED), "#FF0000");
}

#[test]
fn no_stroke_swatch_defaults_to_slate_not_black() {
    // Regression: clicking the stroke hex used to seed #000000 while the
    // swatch painted slate. Paint and the edit-seed now read ONE source
    // (`stroke_swatch_color`), whose no-stroke default is `#374151`.
    use crate::widgets::property_panel_inputs::format_color_hex;
    use crate::widgets::property_panel_snapshot::NodeSnapshot;
    let hex = format_color_hex(NodeSnapshot::DEFAULT_STROKE_SWATCH);
    assert_eq!(hex, "#374151");
    assert_ne!(hex, "#000000");
}
