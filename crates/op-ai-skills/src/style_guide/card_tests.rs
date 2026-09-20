//! Card-expansion tests.
//!
//! The card's whole value is that it distinguishes things the chip cannot —
//! which catalogue a pin points into, and what the guide says about itself —
//! so most of what is asserted here is that those two survive the trip, and
//! that a file which states nothing produces empty fields rather than
//! plausible ones.

use super::*;
use crate::style_guide::import_design_md;
// The SAME lock every other style-guide test module takes. A private mutex
// here would let this module and `summary_tests` empty the shared registry out
// from under each other — the exact failure that made the guard shared.
use crate::style_guide::user_registry::exclusive_registry_for_tests as exclusive;

const TOKEN_TABLE: &str = "\
---
name: Dimension
---

## Overview

A dark, quiet reference system built around one violet accent. It goes on to
say more here, which the card must not show.

## Tokens — Colors

| Name | Value | Token | Role |
| --- | --- | --- | --- |
| Void Canvas | `#0a0a0a` | `--color-void-canvas` | Primary page background |
| Graphite | `#161616` | `--color-graphite` | Elevated surface |
| Bone | `#ededed` | `--color-bone` | Primary readable text |
";

#[test]
fn an_imported_guide_is_named_banded_and_marked_as_imported() {
    let _guard = exclusive();
    let imported = import_design_md(TOKEN_TABLE, "d.md").expect("imports");

    let card = style_guide_card(&imported.id).expect("a live pin expands");
    assert_eq!(card.name, "Dimension");
    assert!(card.is_user, "an import must not read as a shipped guide");
    assert!(
        card.swatches.len() >= 3,
        "the token table states three colours: {:?}",
        card.swatches
    );
    assert_eq!(
        card.description.as_deref(),
        Some("A dark, quiet reference system built around one violet accent."),
        "only the opening sentence of the summary section belongs on a card"
    );
}

/// The corpus opens every guide with an identical "Style Scope" paragraph.
/// Taking the first paragraph would describe all of them the same way, which
/// is worse than describing none of them.
#[test]
fn a_corpus_guide_is_described_by_its_summary_not_its_boilerplate() {
    let _guard = exclusive();
    let guide = crate::style_guide::style_guide_registry()
        .first()
        .expect("the corpus ships guides");

    let card = style_guide_card(&guide.name).expect("a corpus name expands");
    assert!(!card.is_user, "a corpus guide must not read as an import");
    let description = card.description.as_deref().unwrap_or_default();
    assert!(
        !description.contains("self-contained"),
        "the shared Style Scope boilerplate leaked into the card: {description}"
    );
    assert!(!card.swatches.is_empty(), "{}", guide.name);
    assert!(
        card.swatches.len() <= STYLE_CARD_SWATCH_CAP,
        "{:?}",
        card.swatches
    );
}

/// Every shipped guide names its fonts and its palette. A card that silently
/// dropped either would look like a guide that stated neither.
#[test]
fn every_corpus_guide_expands_with_a_palette() {
    let _guard = exclusive();
    for guide in crate::style_guide::style_guide_registry() {
        let card = style_guide_card(&guide.name).expect("expands");
        assert!(
            !card.swatches.is_empty(),
            "{} expanded with no colours at all",
            guide.name
        );
        // Duplicates would read as a smaller palette painted twice.
        for (index, hex) in card.swatches.iter().enumerate() {
            assert!(
                !card.swatches[..index]
                    .iter()
                    .any(|held| held.eq_ignore_ascii_case(hex)),
                "{} repeated {hex}",
                guide.name
            );
        }
    }
}

/// A guide that states nothing gets empty fields. Filling them with a default
/// palette would be indistinguishable from a guide that really said that.
#[test]
fn a_prose_only_guide_carries_no_invented_values() {
    let _guard = exclusive();
    let imported = import_design_md("# Prose Only\n\nQuiet and plain.\n", "p.md").expect("imports");

    let card = style_guide_card(&imported.id).expect("expands");
    assert!(card.swatches.is_empty(), "{:?}", card.swatches);
    assert_eq!(card.display_font, None);
    assert_eq!(card.body_font, None);
}

#[test]
fn a_stale_pin_expands_to_nothing() {
    let _guard = exclusive();
    assert_eq!(style_guide_card("user:deleted-last-week"), None);
    assert_eq!(style_guide_card("   "), None);
}

/// Deleting an import must not leave its card behind: the memo is keyed by id,
/// and a re-import under the same slug would otherwise show the old file.
#[test]
fn removing_an_import_drops_its_memoized_card() {
    let _guard = exclusive();
    let imported = import_design_md(TOKEN_TABLE, "d.md").expect("imports");
    assert!(style_guide_card(&imported.id).is_some());

    crate::style_guide::user_registry::remove_user_style_guide(&imported.id);
    assert_eq!(
        style_guide_card(&imported.id),
        None,
        "the card outlived the guide it describes"
    );
}

#[test]
fn a_version_number_does_not_end_the_opening_sentence() {
    assert_eq!(
        first_sentence("Built on v1.2 of the token spec. Then more."),
        Some("Built on v1.2 of the token spec.".to_string())
    );
}

#[test]
fn a_long_description_is_cut_rather_than_carried_whole() {
    let long = format!("{}.", "word ".repeat(120));
    let cut = first_sentence(&long).expect("a sentence");
    assert!(cut.chars().count() <= DESCRIPTION_CAP + 1, "{cut}");
    assert!(cut.ends_with('…'), "{cut}");
}
