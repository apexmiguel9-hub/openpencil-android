//! Tests for the variables / themes mutators.
//!
//! Split out of the `variables` spine (800-line file ceiling).

use super::*;

use crate::test_support::state_with;

fn doc_with_color_var(name: &str, hex: &str) -> EditorState {
    let mut s = state_with(vec![]);
    s.create_variable(
        name,
        VariableKind::Color,
        VariableScalar::Str(hex.to_string()),
    );
    s
}

#[test]
fn create_then_resolve_color_variable() {
    let s = doc_with_color_var("brand", "#ff8800");
    match s.resolve_variable("brand") {
        Some(VariableScalar::Str(hex)) => assert_eq!(hex, "#ff8800"),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn create_variable_rejects_duplicate_and_empty() {
    let mut s = doc_with_color_var("brand", "#ff0000");
    assert!(!s.create_variable(
        "brand",
        VariableKind::Color,
        VariableScalar::Str("#fff".into())
    ));
    assert!(!s.create_variable("  ", VariableKind::Number, VariableScalar::Num(1.0)));
}

#[test]
fn create_variable_rejects_kind_mismatch() {
    let mut s = state_with(vec![]);
    // Number kind with a string default → rejected.
    assert!(!s.create_variable(
        "x",
        VariableKind::Number,
        VariableScalar::Str("nope".into())
    ));
    // Color kind with a bad hex → rejected.
    assert!(!s.create_variable("c", VariableKind::Color, VariableScalar::Str("zzz".into())));
}

#[test]
fn set_variable_color_writes_and_validates() {
    let mut s = doc_with_color_var("brand", "#ff0000");
    assert!(s.set_variable_color("brand", "#00ff00"));
    match s.resolve_variable("brand") {
        Some(VariableScalar::Str(hex)) => assert_eq!(hex, "#00ff00"),
        other => panic!("unexpected {other:?}"),
    }
    // Bad hex → rejected, value unchanged.
    assert!(!s.set_variable_color("brand", "nothex"));
    // Unknown name → rejected.
    assert!(!s.set_variable_color("missing", "#000000"));
}

#[test]
fn set_variable_color_for_theme_targets_the_clicked_variant() {
    let mut s = doc_with_color_var("brand", "#111111");
    let mut themes = BTreeMap::new();
    themes.insert(
        "Theme-1".to_string(),
        vec!["Light".to_string(), "Dark".to_string()],
    );
    s.doc.themes = Some(themes);
    // Canvas is pinned to Light — the write must still land in
    // the clicked Dark column (TS setValueForTheme parity).
    s.ui.variables
        .active_theme
        .insert("Theme-1".into(), "Light".into());
    assert!(s.set_variable_color_for_theme("brand", "Theme-1", "Dark", "#abcdef"));
    // Scalar materialized into a per-variant themed array; the
    // untouched Light column keeps the old scalar.
    let def = s.find_variable("brand").unwrap();
    let VariableValue::Themed(entries) = &def.value else {
        panic!("expected themed value, got {:?}", def.value);
    };
    assert_eq!(entries.len(), 2);
    let value_for = |variant: &str| {
        entries
            .iter()
            .find(|e| {
                e.theme
                    .as_ref()
                    .and_then(|t| t.get("Theme-1"))
                    .is_some_and(|v| v == variant)
            })
            .map(|e| e.value.clone())
    };
    assert_eq!(
        value_for("Light"),
        Some(VariableScalar::Str("#111111".into()))
    );
    assert_eq!(
        value_for("Dark"),
        Some(VariableScalar::Str("#abcdef".into()))
    );
    // Active theme (Light) still resolves the old colour.
    assert_eq!(
        s.resolve_variable("brand"),
        Some(&VariableScalar::Str("#111111".into()))
    );
}

#[test]
fn set_variable_color_for_theme_rejects_bad_inputs() {
    let mut s = doc_with_color_var("brand", "#111111");
    let mut themes = BTreeMap::new();
    themes.insert("Theme-1".to_string(), vec!["Light".to_string()]);
    s.doc.themes = Some(themes);
    // Bad hex → rejected.
    assert!(!s.set_variable_color_for_theme("brand", "Theme-1", "Light", "nothex"));
    // Undeclared variant → rejected.
    assert!(!s.set_variable_color_for_theme("brand", "Theme-1", "Sepia", "#222222"));
    // Kind mismatch → rejected.
    s.create_variable("n", VariableKind::Number, VariableScalar::Num(1.0));
    assert!(!s.set_variable_color_for_theme("n", "Theme-1", "Light", "#222222"));
    // Value untouched after the rejections.
    assert_eq!(
        s.resolve_variable("brand"),
        Some(&VariableScalar::Str("#111111".into()))
    );
}

#[test]
fn set_variable_boolean_for_theme_targets_the_clicked_variant() {
    let mut s = state_with(vec![]);
    let mut themes = BTreeMap::new();
    themes.insert(
        "Theme-1".to_string(),
        vec!["Light".to_string(), "Dark".to_string()],
    );
    s.doc.themes = Some(themes);
    s.ui.variables
        .active_theme
        .insert("Theme-1".into(), "Light".into());
    s.create_variable("flag", VariableKind::Boolean, VariableScalar::Bool(false));
    assert!(s.set_variable_boolean_for_theme("flag", "Theme-1", "Dark", true));
    // Light (active) still false; Dark column flipped.
    assert_eq!(
        s.resolve_variable("flag"),
        Some(&VariableScalar::Bool(false))
    );
    let def = s.find_variable("flag").unwrap();
    let VariableValue::Themed(entries) = &def.value else {
        panic!("expected themed value");
    };
    assert!(entries.iter().any(|e| {
        e.value == VariableScalar::Bool(true)
            && e.theme
                .as_ref()
                .and_then(|t| t.get("Theme-1"))
                .is_some_and(|v| v == "Dark")
    }));
}

#[test]
fn set_variable_scalar_kind_checks() {
    let mut s = state_with(vec![]);
    s.create_variable("n", VariableKind::Number, VariableScalar::Num(1.0));
    assert!(s.set_variable_number("n", 42.0));
    // String write into a Number variable → rejected.
    assert!(!s.set_variable_string("n", "no"));
    // Color write into a Number variable → rejected.
    s.create_variable("flag", VariableKind::Boolean, VariableScalar::Bool(false));
    assert!(s.set_variable_boolean("flag", true));
    assert!(!s.set_variable_number("flag", 3.0));
}

#[test]
fn delete_variable_drops_it() {
    let mut s = doc_with_color_var("brand", "#ff0000");
    assert!(s.delete_variable("brand"));
    assert!(s.find_variable("brand").is_none());
    assert!(!s.delete_variable("brand"));
}

#[test]
fn rename_variable_moves_definition() {
    let mut s = doc_with_color_var("brand", "#ff0000");
    assert!(s.rename_variable("brand", "primary"));
    assert!(s.find_variable("brand").is_none());
    assert!(s.find_variable("primary").is_some());
    // Unknown old → rejected.
    assert!(!s.rename_variable("brand", "x"));
    // Empty new → rejected.
    assert!(!s.rename_variable("primary", "  "));
}

#[test]
fn rename_variable_collision_rejected() {
    let mut s = doc_with_color_var("a", "#000000");
    s.create_variable(
        "b",
        VariableKind::Color,
        VariableScalar::Str("#ffffff".into()),
    );
    assert!(!s.rename_variable("a", "b"));
}

#[test]
fn active_axis_value_requires_declared_theme() {
    let mut s = state_with(vec![]);
    // No themes declared → false.
    assert!(!s.set_active_axis_value("mode", "dark"));
    let mut themes = BTreeMap::new();
    themes.insert(
        "mode".to_string(),
        vec!["light".to_string(), "dark".to_string()],
    );
    s.doc.themes = Some(themes);
    assert!(s.set_active_axis_value("mode", "dark"));
    assert_eq!(
        s.ui.variables.active_theme.get("mode").map(|v| v.as_str()),
        Some("dark")
    );
    // Undeclared value → false.
    assert!(!s.set_active_axis_value("mode", "sepia"));
}

#[test]
fn cycle_active_axis_wraps() {
    let mut s = state_with(vec![]);
    let mut themes = BTreeMap::new();
    themes.insert(
        "mode".to_string(),
        vec!["light".to_string(), "dark".to_string()],
    );
    s.doc.themes = Some(themes);
    // First cycle seeds the first value.
    assert!(s.cycle_active_axis_value("mode"));
    assert_eq!(s.ui.variables.active_theme["mode"], "light");
    assert!(s.cycle_active_axis_value("mode"));
    assert_eq!(s.ui.variables.active_theme["mode"], "dark");
    // Wraps back to the first.
    assert!(s.cycle_active_axis_value("mode"));
    assert_eq!(s.ui.variables.active_theme["mode"], "light");
    // Unknown axis → false.
    assert!(!s.cycle_active_axis_value("density"));
}

#[test]
fn undo_of_variable_create_removes_it() {
    // Gap 3: `EditorSnapshot` clones the whole `PenDocument`
    // (variables included), so a variable create/delete/rename
    // undoes for free — no separate `var_table` snapshot needed.
    let mut s = state_with(vec![]);
    s.commit_history(); // capture the pre-create state
    assert!(s.create_variable(
        "brand",
        VariableKind::Color,
        VariableScalar::Str("#ff0000".into())
    ));
    assert!(s.find_variable("brand").is_some());
    assert!(s.undo(), "undo must pop the pre-create snapshot");
    assert!(
        s.find_variable("brand").is_none(),
        "undo removes the variable"
    );
    // Redo brings it back.
    assert!(s.redo());
    assert!(s.find_variable("brand").is_some());
}

#[test]
fn undo_of_variable_delete_and_rename_round_trips() {
    let mut s = doc_with_color_var("brand", "#ff8800");
    // Delete under history.
    s.commit_history();
    assert!(s.delete_variable("brand"));
    assert!(s.find_variable("brand").is_none());
    assert!(s.undo());
    assert!(
        s.find_variable("brand").is_some(),
        "undo restores deleted var"
    );
    // Rename under history.
    s.commit_history();
    assert!(s.rename_variable("brand", "primary"));
    assert!(s.undo());
    assert!(s.find_variable("brand").is_some(), "undo restores old name");
    assert!(s.find_variable("primary").is_none());
}

#[test]
fn themed_write_targets_active_theme_entry() {
    let mut s = state_with(vec![]);
    let mut themes = BTreeMap::new();
    themes.insert(
        "mode".to_string(),
        vec!["light".to_string(), "dark".to_string()],
    );
    s.doc.themes = Some(themes);
    // Seed a themed color variable directly.
    let mut vars = BTreeMap::new();
    vars.insert(
        "bg".to_string(),
        VariableDefinition {
            kind: VariableKind::Color,
            value: VariableValue::Themed(vec![ThemedValue {
                value: VariableScalar::Str("#ffffff".into()),
                theme: None,
            }]),
        },
    );
    s.doc.variables = Some(vars);
    // Under no active theme, write hits the default entry.
    assert!(s.set_variable_color("bg", "#eeeeee"));
    // Switch to dark and write — a new dark-keyed entry appends.
    s.set_active_axis_value("mode", "dark");
    assert!(s.set_variable_color("bg", "#111111"));
    match s.resolve_variable("bg") {
        Some(VariableScalar::Str(hex)) => assert_eq!(hex, "#111111"),
        other => panic!("unexpected {other:?}"),
    }
    // Switch back to light — the default entry still resolves.
    s.set_active_axis_value("mode", "light");
    match s.resolve_variable("bg") {
        Some(VariableScalar::Str(hex)) => assert_eq!(hex, "#eeeeee"),
        other => panic!("unexpected {other:?}"),
    }
}
