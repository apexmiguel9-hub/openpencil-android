//! Theme / variant header tests — axis tabs, menus, rename inputs, and
//! the shared name / value cell chrome.

use super::super::*;
use super::support::*;
use crate::widgets::PaintCx;

#[test]
fn row_count_matches_variable_count() {
    let s = state_with_three_vars();
    let p = VariablesPanel::for_editor(&s);
    assert_eq!(p.row_count(), 3);
}

#[test]
fn panel_does_not_paint_drop_shadow() {
    let p = VariablesPanel::for_editor(&EditorState::new());
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(VARIABLES_PANEL_WIDTH, 480.0),
    };
    let mut backend = TextCaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    p.paint(&mut cx, rect);

    assert!(
        !backend.round_fills.iter().any(|(fill, radius, color)| {
            (*radius - PANEL_RADIUS).abs() < 0.01
                && fill.size == rect.size
                && fill.origin.x == rect.origin.x
                && fill.origin.y > rect.origin.y
                && color.r == 0.0
                && color.g == 0.0
                && color.b == 0.0
                && color.a > 0.0
        }),
        "variables panel should not paint a detached drop shadow behind itself"
    );
}

#[test]
fn axis_count_reflects_active_theme() {
    let s = state_with_three_vars();
    let p = VariablesPanel::for_editor(&s);
    assert_eq!(p.axis_count(), 1);
}

#[test]
fn theme_tabs_follow_document_axes_like_ts() {
    let s = state_with_ts_like_themes();
    let p = VariablesPanel::for_editor(&s);

    assert_eq!(p.theme_tab_labels(), vec!["Theme-1", "Theme-2"]);
    assert_eq!(p.active_axis_label(), "Theme-1");
}

#[test]
fn variant_columns_follow_active_axis_values_like_ts() {
    let s = state_with_ts_like_themes();
    let p = VariablesPanel::for_editor(&s);

    assert_eq!(p.variant_column_labels(), vec!["Default", "Variant-1"]);
    assert_eq!(p.variant_column_count(), 2);
}

#[test]
fn variables_without_themes_show_implicit_default_theme() {
    let mut s = EditorState::new();
    s.create_variable(
        "color-1",
        VariableKind::Color,
        VariableScalar::Str("#000000".into()),
    );
    let p = VariablesPanel::for_editor(&s);

    assert_eq!(p.theme_tab_labels(), vec!["Theme-1"]);
    assert_eq!(p.variant_column_labels(), vec!["Default"]);
}

#[test]
fn theme_tab_hit_targets_document_axis_like_ts() {
    let s = state_with_ts_like_themes();
    let p = VariablesPanel::for_editor(&s);
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(VARIABLES_PANEL_WIDTH, p.intrinsic_height()),
    };

    match p.hit_test(rect, Point2D::new(120.0, 22.0)) {
        Some(VariablesPanelHit::ThemeTab(axis)) => assert_eq!(axis, "Theme-2"),
        other => panic!("expected ThemeTab(Theme-2), got {other:?}"),
    }
}

#[test]
fn active_theme_tab_hit_toggles_theme_menu_like_ts() {
    let s = state_with_ts_like_themes();
    let p = VariablesPanel::for_editor(&s);
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(VARIABLES_PANEL_WIDTH, p.intrinsic_height()),
    };

    match p.hit_test(rect, Point2D::new(22.0, 22.0)) {
        Some(VariablesPanelHit::ToggleThemeMenu(axis)) => assert_eq!(axis, "Theme-1"),
        other => panic!("expected ToggleThemeMenu(Theme-1), got {other:?}"),
    }
}

#[test]
fn variant_header_hit_toggles_variant_menu_like_ts() {
    let s = state_with_ts_like_themes();
    let p = VariablesPanel::for_editor(&s);
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(VARIABLES_PANEL_WIDTH, p.intrinsic_height()),
    };
    let point = Point2D::new(value_column_x(rect) + 12.0, HEADER_HEIGHT + 20.0);

    match p.hit_test(rect, point) {
        Some(VariablesPanelHit::ToggleVariantMenu(value)) => assert_eq!(value, "Default"),
        other => panic!("expected ToggleVariantMenu(Default), got {other:?}"),
    }
}

#[test]
fn open_theme_and_variant_menus_route_rename_rows() {
    let mut s = state_with_ts_like_themes();
    s.editor_ui.variables_theme_menu_axis = Some("Theme-1".into());
    let p = VariablesPanel::for_editor(&s);
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(VARIABLES_PANEL_WIDTH, p.intrinsic_height()),
    };
    match p.hit_test(rect, Point2D::new(18.0, HEADER_HEIGHT + 22.0)) {
        Some(VariablesPanelHit::ThemeMenuRename(axis)) => assert_eq!(axis, "Theme-1"),
        other => panic!("expected ThemeMenuRename(Theme-1), got {other:?}"),
    }

    s.editor_ui.variables_theme_menu_axis = None;
    s.editor_ui.variables_variant_menu_value = Some("Variant-1".into());
    let p = VariablesPanel::for_editor(&s);
    let menu_x = value_column_x(rect) + variant_column_width(rect, 2) + 8.0;
    match p.hit_test(
        rect,
        Point2D::new(menu_x, HEADER_HEIGHT + COLUMN_HEADER_HEIGHT + 20.0),
    ) {
        Some(VariablesPanelHit::VariantMenuRename(value)) => assert_eq!(value, "Variant-1"),
        other => panic!("expected VariantMenuRename(Variant-1), got {other:?}"),
    }
}

#[test]
fn theme_rename_input_reserves_header_space_like_ts() {
    let mut s = EditorState::new();
    s.doc
        .themes
        .get_or_insert_with(Default::default)
        .insert("Theme-1".into(), vec!["Default".into()]);
    s.ui.variables
        .active_theme
        .insert("Theme-1".into(), "Default".into());
    s.editor_ui.variables_current_axis = Some("Theme-1".into());
    s.editor_ui.variables_theme_rename_axis = Some("Theme-1".into());
    s.editor_ui.variables_header_input.set_text("ewe");
    let p = VariablesPanel::for_editor(&s);
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(VARIABLES_PANEL_WIDTH, p.intrinsic_height()),
    };
    let input_end = rect.origin.x + PAD_X - 2.0 + (label_width("ewe", 13.0) + 28.0).max(96.0);

    assert!(
        p.add_theme_rect(rect).origin.x >= input_end + 4.0,
        "add theme button must sit after the active rename input"
    );
}

#[test]
fn theme_rename_caret_hides_at_blink_off_phase() {
    let mut s = EditorState::new();
    s.doc
        .themes
        .get_or_insert_with(Default::default)
        .insert("Theme-1".into(), vec!["Default".into()]);
    s.ui.variables
        .active_theme
        .insert("Theme-1".into(), "Default".into());
    s.editor_ui.variables_current_axis = Some("Theme-1".into());
    s.editor_ui.variables_theme_rename_axis = Some("Theme-1".into());
    s.editor_ui.variables_header_input.set_text("Theme-1");
    s.editor_ui
        .variables_header_input
        .set_caret("Theme-1".len(), 0);
    let p = VariablesPanel::for_editor_at(&s, 500);
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(VARIABLES_PANEL_WIDTH, p.intrinsic_height()),
    };
    let mut backend = TextCaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    p.paint(&mut cx, rect);

    assert!(
        caret_fills(&backend.fills, p.theme).is_empty(),
        "variables-panel inline rename caret should blink off at the off phase"
    );
}

#[test]
fn variant_rename_caret_blinks_in_painted_header_input() {
    let mut s = EditorState::new();
    s.doc
        .themes
        .get_or_insert_with(Default::default)
        .insert("Theme-1".into(), vec!["Default".into()]);
    s.ui.variables
        .active_theme
        .insert("Theme-1".into(), "Default".into());
    s.editor_ui.variables_current_axis = Some("Theme-1".into());
    s.editor_ui.variables_variant_rename_value = Some("Default".into());
    s.editor_ui.variables_header_input.set_text("Default");
    s.editor_ui
        .variables_header_input
        .set_caret("Default".len(), 0);
    let p_visible = VariablesPanel::for_editor_at(&s, 0);
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(VARIABLES_PANEL_WIDTH, p_visible.intrinsic_height()),
    };
    let mut visible_backend = TextCaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut visible_backend,
    };
    p_visible.paint(&mut cx, rect);

    assert!(
        !caret_fills(&visible_backend.fills, p_visible.theme).is_empty(),
        "variant header input should paint its caret at the blink anchor"
    );

    let p_hidden = VariablesPanel::for_editor_at(&s, 500);
    let mut hidden_backend = TextCaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut hidden_backend,
    };
    p_hidden.paint(&mut cx, rect);

    assert!(
        caret_fills(&hidden_backend.fills, p_hidden.theme).is_empty(),
        "variant header input caret should disappear at the blink off phase"
    );
}

#[test]
fn editing_variable_name_caret_uses_raw_name_like_ts() {
    let mut s = state_with_three_vars();
    s.editor_ui.variable_row_focus = Some(VariableRowFocus::Name(0));
    s.editor_ui.variable_row_input.set_text("color-1");
    s.editor_ui.variable_row_input.set_caret(3, 0);
    let p = VariablesPanel::for_editor_at(&s, 0);

    assert_eq!(p.name_caret_for_row(0), Some(3));
}

#[test]
fn editing_value_cell_uses_shared_input_chrome() {
    let mut s = EditorState::new();
    s.create_variable("spacing", VariableKind::Number, VariableScalar::Num(16.0));
    s.editor_ui.variable_row_focus = Some(VariableRowFocus::Number(0));
    s.editor_ui.variable_row_input.set_text("24");
    s.editor_ui.variable_row_input.set_caret("24".len(), 0);
    let p = VariablesPanel::for_editor_at(&s, 0);
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(VARIABLES_PANEL_WIDTH, p.intrinsic_height()),
    };
    let mut backend = TextCaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    p.paint(&mut cx, rect);

    let row_y = p.rows_start_y(rect);
    let value_input = backend
        .round_strokes
        .iter()
        .find_map(|(stroke, radius, color, width)| {
            (color_eq(*color, p.theme.primary)
                && (*radius - 8.0).abs() < 0.01
                && (*width - 1.5).abs() < 0.01
                && (stroke.origin.x - (value_column_x(rect) - 8.0)).abs() < 0.01
                && (stroke.origin.y - (row_y + 7.0)).abs() < 0.01
                && (stroke.size.y - 30.0).abs() < 0.01)
                .then_some(*stroke)
        })
        .expect("editing a variable value should use the same focused input chrome as header/name editing");

    assert!(
        value_input.size.x <= 160.0,
        "short variable value edits should not stretch across the full value column; got {}",
        value_input.size.x
    );
}

#[test]
fn variable_name_display_paints_two_literal_hyphens_like_ts() {
    let s = state_with_three_vars();
    let p = VariablesPanel::for_editor(&s);
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(VARIABLES_PANEL_WIDTH, p.intrinsic_height()),
    };
    let mut backend = TextCaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    p.paint(&mut cx, rect);

    assert!(
        !backend.texts.iter().any(|text| text == "--color-1"),
        "display mode should not shape the variable prefix as one text run"
    );
    let idx = backend
        .texts
        .iter()
        .position(|text| text == "color-1")
        .expect("painted variable name text");
    assert!(idx >= 2, "name should be preceded by two hyphen runs");
    assert_eq!(backend.texts[idx - 2], "-");
    assert_eq!(backend.texts[idx - 1], "-");
    assert!(
        backend.origins[idx - 1].x - backend.origins[idx - 2].x >= 8.0,
        "the two variable prefix hyphens should be visually separated"
    );
    assert!(
        backend.origins[idx].x - backend.origins[idx - 1].x >= 8.0,
        "the variable name should start after the second prefix hyphen"
    );
}
