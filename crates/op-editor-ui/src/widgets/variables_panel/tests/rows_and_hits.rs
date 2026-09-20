//! Row resolution, panel metrics, and hit-test coverage for the
//! variables panel.

use super::super::*;
use super::support::*;
use crate::widgets::PaintCx;
use jian_ops_schema::variable::{ThemedValue, VariableScalar, VariableValue};
use std::collections::BTreeMap;

#[test]
fn variable_rows_resolve_per_variant_values_like_ts() {
    let mut s = state_with_ts_like_themes();
    let mut default_theme = BTreeMap::new();
    default_theme.insert("Theme-1".to_string(), "Default".to_string());
    let mut variant_theme = BTreeMap::new();
    variant_theme.insert("Theme-1".to_string(), "Variant-1".to_string());
    s.doc.variables.get_or_insert_with(Default::default).insert(
        "color-1".into(),
        jian_ops_schema::variable::VariableDefinition {
            kind: VariableKind::Color,
            value: VariableValue::Themed(vec![
                ThemedValue {
                    value: VariableScalar::Str("#c81919".into()),
                    theme: Some(default_theme),
                },
                ThemedValue {
                    value: VariableScalar::Str("#0066ff".into()),
                    theme: Some(variant_theme),
                },
            ]),
        },
    );
    let p = VariablesPanel::for_editor(&s);

    assert_eq!(
        p.variant_scalar_for(&p.rows[0], "Theme-1", "Default"),
        Some(&VariableScalar::Str("#c81919".into()))
    );
    assert_eq!(
        p.variant_scalar_for(&p.rows[0], "Theme-1", "Variant-1"),
        Some(&VariableScalar::Str("#0066ff".into()))
    );
}

#[test]
fn intrinsic_height_grows_with_rows_and_chips() {
    let s_empty = EditorState::new();
    let p = VariablesPanel::for_editor(&s_empty);
    let empty_h = p.intrinsic_height();
    assert!(
        (empty_h - (HEADER_HEIGHT + COLUMN_HEADER_HEIGHT + FOOTER_HEIGHT)).abs() < f32::EPSILON
    );
    let s = state_with_three_vars();
    let p2 = VariablesPanel::for_editor(&s);
    assert!(p2.intrinsic_height() > empty_h);
}

#[test]
fn axis_dropdown_hit_routes_to_named_value() {
    let mut s = state_with_three_vars();
    s.doc.themes.get_or_insert_with(Default::default).insert(
        "mode".into(),
        vec!["light".into(), "dark".into(), "system".into()],
    );
    let mut p = VariablesPanel::for_editor(&s);
    p.dropdown_open = Some("mode".into());
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(VARIABLES_PANEL_WIDTH, p.intrinsic_height()),
    };
    let chip = p.chip_rect(rect, 0);
    let menu_y = chip.origin.y + chip.size.y + 4.0;
    let click_y = menu_y + DROPDOWN_ROW_HEIGHT * 0.5;
    let click_x = chip.origin.x + 10.0;
    match p.hit_test(rect, Point2D::new(click_x, click_y)) {
        Some(VariablesPanelHit::AxisDropdownItem { axis, value }) => {
            assert_eq!(axis, "mode");
            assert_eq!(value, "light");
        }
        other => panic!("expected AxisDropdownItem for row 0, got {other:?}"),
    }
    let click_y_sys = menu_y + DROPDOWN_ROW_HEIGHT * 2.5;
    match p.hit_test(rect, Point2D::new(click_x, click_y_sys)) {
        Some(VariablesPanelHit::AxisDropdownItem { axis, value }) => {
            assert_eq!(axis, "mode");
            assert_eq!(value, "system");
        }
        other => panic!("expected AxisDropdownItem for row 2, got {other:?}"),
    }
}

#[test]
fn hit_test_returns_row_index_for_in_row_click() {
    let s = state_with_three_vars();
    let p = VariablesPanel::for_editor(&s);
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(VARIABLES_PANEL_WIDTH, p.intrinsic_height()),
    };
    let y = HEADER_HEIGHT + COLUMN_HEADER_HEIGHT + ROW_HEIGHT * 1.0 + ROW_HEIGHT / 2.0;
    match p.hit_test(rect, Point2D::new(PAD_X + 4.0, y)) {
        Some(VariablesPanelHit::Row(1)) => {}
        other => panic!("expected Row(1), got {other:?}"),
    }
}

#[test]
fn hit_test_returns_name_cell_for_variable_name_pill() {
    let s = state_with_three_vars();
    let p = VariablesPanel::for_editor(&s);
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(VARIABLES_PANEL_WIDTH, p.intrinsic_height()),
    };
    let y = HEADER_HEIGHT + COLUMN_HEADER_HEIGHT + ROW_HEIGHT / 2.0;
    match p.hit_test(rect, Point2D::new(PAD_X + 42.0, y)) {
        Some(VariablesPanelHit::NameCell(0)) => {}
        other => panic!("expected NameCell(0), got {other:?}"),
    }
    match p.hit_test(rect, Point2D::new(PAD_X + 4.0, y)) {
        Some(VariablesPanelHit::Row(0)) => {}
        other => panic!("expected Row(0), got {other:?}"),
    }
}

#[test]
fn hit_test_returns_variant_menu_for_column_header_click() {
    let s = state_with_three_vars();
    let p = VariablesPanel::for_editor(&s);
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(VARIABLES_PANEL_WIDTH, p.intrinsic_height()),
    };
    let y = HEADER_HEIGHT + 8.0 + CHIP_HEIGHT / 2.0;
    match p.hit_test(rect, Point2D::new(value_column_x(rect) + 4.0, y)) {
        Some(VariablesPanelHit::ToggleVariantMenu(value)) => assert_eq!(value, "Default"),
        other => panic!("expected ToggleVariantMenu(Default), got {other:?}"),
    }
}

#[test]
fn hit_test_returns_value_cell_for_variant_value_click() {
    let mut s = state_with_ts_like_themes();
    s.create_variable("number", VariableKind::Number, VariableScalar::Num(0.0));
    let p = VariablesPanel::for_editor(&s);
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(VARIABLES_PANEL_WIDTH, p.intrinsic_height()),
    };
    let col_w = variant_column_width(rect, 2);
    let x = value_column_x(rect) + col_w + 12.0;
    let y = HEADER_HEIGHT + COLUMN_HEADER_HEIGHT + ROW_HEIGHT / 2.0;

    match p.hit_test(rect, Point2D::new(x, y)) {
        Some(VariablesPanelHit::ValueCell { row, variant }) => {
            assert_eq!(row, 0);
            assert_eq!(variant, 1);
        }
        other => panic!("expected ValueCell(row=0, variant=1), got {other:?}"),
    }
}

#[test]
fn panel_buttons_are_hittable() {
    let p = VariablesPanel::for_editor(&EditorState::new());
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(VARIABLES_PANEL_WIDTH, 480.0),
    };
    for point in [
        Point2D::new(24.0, 22.0),
        Point2D::new(82.0, 22.0),
        Point2D::new(rect.size.x - 24.0, HEADER_HEIGHT + 18.0),
        Point2D::new(62.0, rect.size.y - 20.0),
    ] {
        assert!(p.hit_test(rect, point).is_some(), "{point:?}");
    }
}

#[test]
fn header_controls_use_shared_vertical_center() {
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(VARIABLES_PANEL_WIDTH, 480.0),
    };
    let center = header::control_center_y(rect);
    assert_eq!(center, 22.0);
    assert_eq!(header::icon_origin(rect, 16.0, 16.0).y, 14.0);
    assert_eq!(header::icon_origin(rect, 118.0, 12.0).y, 16.0);
    assert!((header::text_baseline(rect, 14.0) - 27.0).abs() < 0.1);
}

#[test]
fn preset_chevron_sits_after_localized_label() {
    let mut s = EditorState::new();
    s.editor_ui.locale = Locale::ZhCn;
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

    let preset = p.preset_rect(rect);
    let expected_x = preset.origin.x + 29.0 + label_width("预设", 13.0) + 7.0;
    let chevron = backend
        .svg_origins
        .iter()
        .zip(backend.svg_sizes.iter())
        .find(|(origin, size)| {
            (**size - 11.0).abs() < f32::EPSILON
                && origin.x >= preset.origin.x
                && origin.x < preset.origin.x + preset.size.x
        })
        .map(|(origin, _)| *origin)
        .expect("preset chevron should paint inside preset rect");

    assert!(
        (chevron.x - expected_x).abs() <= 1.0,
        "preset chevron should follow the localized label; got {}, expected {}",
        chevron.x,
        expected_x
    );
    assert!(
        ((chevron.y + 11.0 / 2.0) - header::control_center_y(rect)).abs() <= 0.1,
        "preset chevron should be vertically centered"
    );
}

#[test]
fn preset_chevron_clears_cjk_label_width() {
    let mut s = EditorState::new();
    s.editor_ui.locale = Locale::ZhCn;
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

    let preset = p.preset_rect(rect);
    let label_x = preset.origin.x + 29.0;
    let rendered_label_right = label_x + 2.0 * 13.0;
    let chevron = backend
        .svg_origins
        .iter()
        .zip(backend.svg_sizes.iter())
        .find(|(origin, size)| {
            (**size - 11.0).abs() < f32::EPSILON
                && origin.x >= preset.origin.x
                && origin.x < preset.origin.x + preset.size.x
        })
        .map(|(origin, _)| *origin)
        .expect("preset chevron should paint inside preset rect");

    assert!(
        chevron.x >= rendered_label_right + 6.0,
        "preset chevron should clear the CJK label; got {}, expected at least {}",
        chevron.x,
        rendered_label_right + 6.0
    );
}

#[test]
fn footer_add_variable_button_aligns_to_panel_padding() {
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(VARIABLES_PANEL_WIDTH, 480.0),
    };
    let button = add_variable_rect(rect);
    let footer_top = rect.origin.y + rect.size.y - FOOTER_HEIGHT;

    assert_eq!(button.origin.x, rect.origin.x + PAD_X);
    assert!(
        ((button.origin.y + button.size.y / 2.0) - (footer_top + FOOTER_HEIGHT / 2.0)).abs()
            < f32::EPSILON,
        "footer add button should be vertically centered in the footer"
    );
    assert_eq!(button.size.y, 30.0);
}

#[test]
fn footer_add_variable_chevron_clears_cjk_label() {
    let mut s = EditorState::new();
    s.editor_ui.locale = Locale::ZhCn;
    let p = VariablesPanel::for_editor(&s);
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(VARIABLES_PANEL_WIDTH, 480.0),
    };
    let button = add_variable_rect(rect);
    let mut backend = TextCaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    p.paint(&mut cx, rect);

    let label_x = button.origin.x + 16.0 + 12.0;
    let rendered_label_right = label_x + 4.0 * 14.0;
    let chevron = backend
        .svg_origins
        .iter()
        .zip(backend.svg_sizes.iter())
        .find(|(origin, size)| {
            (**size - 12.0).abs() < f32::EPSILON
                && origin.x >= button.origin.x
                && origin.x < button.origin.x + button.size.x
                && origin.y >= button.origin.y
                && origin.y < button.origin.y + button.size.y
        })
        .map(|(origin, _)| *origin)
        .expect("footer add-variable chevron should paint inside the button");

    assert!(
        chevron.x >= rendered_label_right + 10.0,
        "footer add-variable chevron should clear the CJK label; got {}, expected at least {}",
        chevron.x,
        rendered_label_right + 10.0
    );
}

#[test]
fn labels_follow_active_i18n_locale() {
    let mut s = EditorState::new();
    s.editor_ui.locale = Locale::Ja;
    let labels = VariablesPanel::for_editor(&s).labels();

    assert_eq!(labels.preset, "プリセット");
    assert_eq!(labels.name, "名前");
    assert_eq!(labels.empty, "変数が定義されていません");
    assert_eq!(labels.add_variable, "変数を追加");
    assert_eq!(labels.save_preset, "現在の設定をプリセットとして保存…");
    assert_eq!(labels.color, "色");
    assert_eq!(labels.number, "数値");
    assert_eq!(labels.string, "文字列");
}

#[test]
fn hit_test_returns_none_outside_rect() {
    let s = state_with_three_vars();
    let p = VariablesPanel::for_editor(&s);
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(VARIABLES_PANEL_WIDTH, 200.0),
    };
    assert!(p.hit_test(rect, Point2D::new(-10.0, 50.0)).is_none());
    assert!(p.hit_test(rect, Point2D::new(50.0, 1000.0)).is_none());
}

#[test]
fn axis_chip_table_mirrors_active_theme_btree_order() {
    let mut s = EditorState::new();
    s.ui.variables
        .active_theme
        .insert("z-axis".into(), "alpha".into());
    s.ui.variables
        .active_theme
        .insert("a-axis".into(), "omega".into());
    let p = VariablesPanel::for_editor(&s);
    assert_eq!(p.chips.len(), 2);
    assert_eq!(p.chips[0].axis, "a-axis");
    assert_eq!(p.chips[1].axis, "z-axis");
}

// Fit-content hover-wash tests (#26 variant header + #3 add-variable footer)
// live in the sibling `variables_panel/wash_tests.rs` to keep this file under
// the 800-line cap.
