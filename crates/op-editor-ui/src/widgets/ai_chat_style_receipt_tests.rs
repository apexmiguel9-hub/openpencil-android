//! Pinned-style receipt tests.
//!
//! The row exists because two different failures were indistinguishable from
//! the editor. Most of what is asserted here is therefore about *not* showing
//! something: a row that appears when the pin is dead, or that keeps its ✕ on
//! a pin it cannot clear, would be a new way to mislead rather than a fix.

use super::*;
use crate::widgets::ai_chat_hit::AIChatHit;
use crate::widgets::asset_center_style_cards::style_test_support::exclusive_user_styles as exclusive_registry_for_tests;
use crate::widgets::AIChatPlaceholder;
use op_editor_core::EditorState;

const PANEL: Rect = Rect {
    origin: Point2D { x: 40.0, y: 60.0 },
    size: Point2D { x: 380.0, y: 520.0 },
};

const IMPORTED: &str = "\
---
name: Dimension
---

## Tokens — Colors

| Name | Value | Token | Role |
| --- | --- | --- | --- |
| Void Canvas | `#0a0a0a` | `--color-void-canvas` | Primary page background, base surface |
| Graphite | `#161616` | `--color-graphite` | Elevated surface for floating panels |
| Dusk Violet | `linear-gradient(90deg, rgba(0,0,0,0), rgba(107,98,242,0.565) 50%)` | `--color-dusk-violet` | The only chromatic accent |
| Bone | `#ededed` | `--color-bone` | Primary readable text on dark surfaces |
";

fn state_with_pin(pin: Option<&str>) -> EditorState {
    let mut state = EditorState::new();
    state.editor_ui.pinned_style_guide = pin.map(str::to_string);
    state
}

#[test]
fn no_pin_means_no_row() {
    let _guard = exclusive_registry_for_tests();
    let state = state_with_pin(None);
    assert_eq!(StyleReceipt::for_state(&state), None);

    let panel = AIChatPlaceholder::from_editor_at(&state, 0);
    assert_eq!(panel.chip_row_h(), 0.0);
}

/// The reported failure made visible: the name says which style is in force,
/// and the band says its values could actually be read.
#[test]
fn a_pinned_import_is_named_and_banded() {
    let _guard = exclusive_registry_for_tests();
    let imported = op_ai_skills::style_guide::import_design_md(IMPORTED, "d.md").expect("imports");
    let state = state_with_pin(Some(&imported.id));

    let receipt = StyleReceipt::for_state(&state).expect("a live pin shows a row");
    // The author's name, not `user:dimension`.
    assert_eq!(receipt.name, "Dimension");
    assert_eq!(receipt.swatches.len(), 4);
    assert!(receipt.clearable);

    let panel = AIChatPlaceholder::from_editor_at(&state, 0);
    assert!(panel.chip_row_h() > 0.0);
    let label = panel.style_receipt_label().expect("a row has a label");
    assert!(label.contains("Dimension"), "{label}");
    assert!(
        !label.contains("{{name}}"),
        "the placeholder must be substituted: {label}"
    );
}

/// A pin whose guide was deleted resolves to nothing. Generation has already
/// fallen back to choosing its own style, so naming the dead guide would
/// replace one silent failure with a confident false one.
#[test]
fn a_stale_pin_shows_no_row_at_all() {
    let _guard = exclusive_registry_for_tests();
    let state = state_with_pin(Some("user:deleted-last-week"));
    assert_eq!(StyleReceipt::for_state(&state), None);
    assert_eq!(
        AIChatPlaceholder::from_editor_at(&state, 0).chip_row_h(),
        0.0
    );
}

/// A guide whose values nothing could read still gets a row — with no band.
/// That empty band is the point: it is the visible form of "pinned, but this
/// file yielded no colours", which previously required a probe to detect.
#[test]
fn an_unreadable_guide_shows_a_name_with_no_band() {
    let _guard = exclusive_registry_for_tests();
    let imported =
        op_ai_skills::style_guide::import_design_md("# Prose Only\n\nQuiet and plain.\n", "p.md")
            .expect("imports");
    let state = state_with_pin(Some(&imported.id));

    let receipt = StyleReceipt::for_state(&state).expect("still a row");
    assert_eq!(receipt.name, "Prose Only");
    assert!(receipt.swatches.is_empty());
}

/// design.md outranks a pin in the pipeline, so it outranks it here. Showing
/// the pinned name would claim a style that is not in force; showing nothing
/// would leave the user unable to learn why their pin stopped mattering.
#[test]
fn design_md_is_reported_instead_of_the_pin_and_carries_no_clear() {
    let _guard = exclusive_registry_for_tests();
    let imported = op_ai_skills::style_guide::import_design_md(IMPORTED, "d.md").expect("imports");
    let mut state = state_with_pin(Some(&imported.id));
    state.doc.design_md = Some(jian_ops_schema::DesignMdSpec {
        raw: String::new(),
        project_name: None,
        visual_theme: Some("warm minimal".into()),
        color_palette: None,
        typography: None,
        component_styles: None,
        layout_principles: None,
        generation_notes: None,
    });

    let receipt = StyleReceipt::for_state(&state).expect("a row");
    assert_eq!(receipt.name, "design.md");
    assert_ne!(receipt.name, "Dimension");
    // Nothing on this row is the user's to clear: design.md is bound and
    // unbound from its own panel, and the pin it displaced is not what the
    // row is reporting.
    assert!(!receipt.clearable);
    assert!(clear_rect(&receipt, Rect::xywh(0.0, 0.0, 120.0, 22.0)).is_none());
}

// ─── Layout + hit-test ─────────────────────────────────────────────────

#[test]
fn the_row_reserves_space_above_the_input_text() {
    let _guard = exclusive_registry_for_tests();
    let bare_state = state_with_pin(None);
    let bare = AIChatPlaceholder::from_editor_at(&bare_state, 0);

    let imported = op_ai_skills::style_guide::import_design_md(IMPORTED, "d.md").expect("imports");
    let pinned_state = state_with_pin(Some(&imported.id));
    let pinned = AIChatPlaceholder::from_editor_at(&pinned_state, 0);

    // The input block grows rather than the text area shrinking into the row.
    assert!(pinned.input_height_for_rect(PANEL) > bare.input_height_for_rect(PANEL));
    let chip = pinned
        .chip_row(pinned.input_rect(PANEL))
        .style
        .expect("a live pin shows its chip");
    assert!(
        chip.origin.y + chip.size.y <= pinned.input_text_rect(PANEL).origin.y,
        "the chip must sit above the text area, not over it"
    );
}

#[test]
fn pressing_the_clear_target_unpins_and_pressing_the_label_does_not() {
    let _guard = exclusive_registry_for_tests();
    let imported = op_ai_skills::style_guide::import_design_md(IMPORTED, "d.md").expect("imports");
    let state = state_with_pin(Some(&imported.id));
    let panel = AIChatPlaceholder::from_editor_at(&state, 0);
    let input_rect = panel.input_rect(PANEL);
    let clear = panel
        .style_receipt_clear_rect(input_rect)
        .expect("a clearable row has a target");

    let centre = Point2D::new(
        clear.origin.x + clear.size.x / 2.0,
        clear.origin.y + clear.size.y / 2.0,
    );
    assert_eq!(
        panel.hit_test(PANEL, centre),
        Some(AIChatHit::ClearPinnedStyle)
    );

    // The rest of the row is not a button — pressing the name focuses the
    // input, the same as pressing anywhere else in the block.
    let on_label = Point2D::new(input_rect.origin.x + 4.0, centre.y);
    assert_eq!(panel.hit_test(PANEL, on_label), Some(AIChatHit::FocusInput));
}

/// The chip must not overrun its block however long the style is named — a
/// name is user-supplied and can be anything.
#[test]
fn a_very_long_name_is_clamped_to_the_input_block() {
    let receipt = StyleReceipt {
        name: "N".repeat(400),
        swatches: vec![Color::rgba_u8(1, 2, 3, 1.0); 4],
        clearable: true,
    };
    let input_rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(200.0, 100.0),
    };
    let natural = chip_width(&receipt.name, receipt.swatches.len(), receipt.clearable);
    let chip = crate::widgets::ai_chat_chip_row::chip_row_layout(Some(natural), None, input_rect)
        .style
        .expect("a chip");
    assert!(chip.size.x <= input_rect.size.x);
    let clear = clear_rect(&receipt, chip).expect("clearable");
    assert!(clear.origin.x + clear.size.x <= input_rect.origin.x + input_rect.size.x);
}
