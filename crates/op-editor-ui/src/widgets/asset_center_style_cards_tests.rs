//! Styles-tab card tests.

use super::style_test_support::{exclusive_user_styles, import_style};
use super::*;

#[test]
fn every_registry_guide_becomes_a_card() {
    let _guard = exclusive_user_styles();
    let cards = style_guide_cards();
    assert_eq!(cards.len(), style_guide_registry().len());
    assert!(cards.len() > 20, "the shipped corpus is not this small");
}

#[test]
fn cards_carry_a_name_swatches_and_a_summary() {
    let _guard = exclusive_user_styles();
    // Not a spot check on one guide: a card with an empty band or an
    // empty summary paints as a blank tile, and the tab is only useful
    // if every entry is recognizable.
    for card in style_guide_cards() {
        assert!(!card.name.is_empty());
        assert!(
            !card.swatches.is_empty(),
            "{} has no parseable palette colours",
            card.name
        );
        assert!(
            card.swatches.len() <= STYLE_SWATCH_COUNT,
            "{} overflows the band",
            card.name
        );
        assert!(!card.summary.is_empty(), "{} has no summary", card.name);
    }
}

#[test]
fn a_known_guide_reads_its_own_palette() {
    let _guard = exclusive_user_styles();
    let card = style_guide_cards()
        .into_iter()
        .find(|c| c.name == "nordic-frost-light")
        .expect("the corpus ships this guide");
    assert_eq!(card.platform, Platform::Webapp);
    // The background swatch is the first entry, and a light guide's
    // background must not come back near-black — that would mean the
    // colour parse silently failed and painted a default.
    let background = card.swatches[0];
    assert!(
        background.r > 0.8 && background.g > 0.8 && background.b > 0.8,
        "expected a light background, got {background:?}"
    );
}

#[test]
fn search_matches_names_and_tags() {
    let _guard = exclusive_user_styles();
    assert!(filtered_style_guide_cards("terminal")
        .iter()
        .any(|c| c.name == "developer-terminal-dark"));
    // Tag-only match: "brutalist" is a tag, not a substring of the name.
    assert!(!filtered_style_guide_cards("brutalist").is_empty());
    assert_eq!(
        filtered_style_guide_cards("").len(),
        style_guide_registry().len(),
        "an empty query filters nothing"
    );
    assert!(filtered_style_guide_cards("zzz-no-such-style").is_empty());
}

#[test]
fn pinning_matches_by_exact_name() {
    let _guard = exclusive_user_styles();
    let card = style_guide_cards()
        .into_iter()
        .find(|c| c.name == "zen-paper-light")
        .expect("the corpus ships this guide");
    assert!(card.is_pinned(Some("zen-paper-light")));
    assert!(!card.is_pinned(Some("zen-paper")));
    assert!(!card.is_pinned(None));
}

// ─── Imported guides ───────────────────────────────────────────────────

/// Your own material sorts above fifty shipped entries, and the count tells
/// the grid where to draw the boundary between the two sections.
#[test]
fn imports_lead_the_list_and_are_counted() {
    let _guard = exclusive_user_styles();
    import_style("Studio Ochre");
    import_style("Night Market");

    let cards = style_guide_cards();
    assert_eq!(user_card_count(&cards), 2);
    assert_eq!(cards[0].name, "Studio Ochre");
    assert_eq!(cards[1].name, "Night Market");
    assert!(cards[0].is_user && cards[1].is_user);
    assert!(!cards[2].is_user, "the corpus follows the imports");
    assert_eq!(cards.len(), style_guide_registry().len() + 2);
}

#[test]
fn an_imported_card_pins_by_its_id_not_its_name() {
    let _guard = exclusive_user_styles();
    let id = import_style("Studio Ochre");
    assert_eq!(id, "user:studio-ochre");

    let card = &style_guide_cards()[0];
    assert_eq!(card.id, id);
    assert!(card.is_pinned(Some(&id)));
    assert!(
        !card.is_pinned(Some("Studio Ochre")),
        "pinning by display name would collide with the corpus namespace"
    );
    // The band comes from the document's own hex scan.
    assert_eq!(card.swatches.len(), 1);
}

/// A `DESIGN.md` with no tags — which is most of them — still needs a line
/// under its name, or every import paints as an anonymous tile.
#[test]
fn an_untagged_import_summarises_itself_from_its_prose() {
    let _guard = exclusive_user_styles();
    op_ai_skills::style_guide::import_design_md(
        "# Quiet Type\n\nGenerous leading and no ornament at all.\n",
        "quiet.md",
    )
    .expect("imports");

    let card = &style_guide_cards()[0];
    assert_eq!(card.name, "Quiet Type");
    assert_eq!(card.summary, "Generous leading and no ornament at all.");
    // No colours anywhere in the document is legal; the band just stays empty.
    assert!(card.swatches.is_empty());
}

#[test]
fn search_spans_both_halves_of_the_catalogue() {
    let _guard = exclusive_user_styles();
    import_style("Studio Ochre");

    let hits = filtered_style_guide_cards("ochre");
    assert_eq!(hits.len(), 1);
    assert!(hits[0].is_user);

    // A query that only the corpus answers still finds it with an import
    // sitting above it in the list.
    assert!(filtered_style_guide_cards("terminal")
        .iter()
        .any(|c| !c.is_user));
}
