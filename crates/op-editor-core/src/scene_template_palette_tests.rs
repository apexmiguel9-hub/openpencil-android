use super::*;
use crate::scene_template_catalog::scene_template_catalogue;

#[test]
fn every_shipped_template_declares_a_readable_palette() {
    for template in scene_template_catalogue() {
        let palette = scene_template_palette(&template.id);
        // Non-empty is the contract the card depends on: every template gets
        // a band, and a band with nothing in it is a stripe of placeholder
        // grey where the design's colours should be. The *length* is not
        // asserted per template — a two-colour design is a legitimate design,
        // and the variables path plus the fill-frequency fallback between
        // them read something out of anything that renders at all.
        assert!(
            !palette.is_empty(),
            "{} yielded no colours at all",
            template.id
        );
        assert!(palette.len() <= TEMPLATE_PALETTE_MAX);
        for hex in palette.iter() {
            assert_eq!(
                hex.len(),
                7,
                "{} emitted a non-canonical {hex}",
                template.id
            );
            assert!(hex.starts_with('#'));
        }
        let mut unique = palette.as_slice().to_vec();
        unique.sort();
        unique.dedup();
        assert_eq!(
            unique.len(),
            palette.len(),
            "{} repeats a colour in its band",
            template.id
        );
    }
}

/// The band is read out of the template's own variables, not invented — so a
/// colour the document declares has to come back verbatim.
#[test]
fn the_palette_comes_from_the_documents_declared_variables() {
    let palette = scene_template_palette("minimal-keynote");
    assert!(
        palette.contains(&"#C7340F".to_string()),
        "the deck's accent must be in its band: {palette:?}"
    );
    assert!(
        palette.contains(&"#FFFFFF".to_string()),
        "so must its ground: {palette:?}"
    );
}

/// Ground before accent before ink, whichever order the file declared them
/// in — the band reads as a palette rather than as a dump of the variable
/// table.
#[test]
fn roles_are_hoisted_ahead_of_declaration_order() {
    let document = r##"{
        "variables": {
            "c-ink": { "type": "color", "value": "#111111" },
            "c-accent": { "type": "color", "value": "#FF0000" },
            "c-bg": { "type": "color", "value": "#FFFFFF" },
            "c-extra": { "type": "color", "value": "#00FF00" }
        },
        "children": []
    }"##;
    assert_eq!(
        extract_palette(document),
        vec!["#FFFFFF", "#FF0000", "#111111", "#00FF00"],
        "bg, accent, ink, then whatever is left"
    );
}

/// Non-colour variables and `$c-accent` references are not colours, and a
/// band that painted them would be painting parse failures.
#[test]
fn only_colour_variables_with_real_hex_values_reach_the_band() {
    let document = r##"{
        "variables": {
            "gap": { "type": "number", "value": "12" },
            "c-alias": { "type": "color", "value": "$c-bg" },
            "c-bg": { "type": "color", "value": "#0A0A0A" },
            "c-accent": { "type": "color", "value": "#2563EB" },
            "c-line": { "type": "color", "value": "#333333" }
        },
        "children": []
    }"##;
    assert_eq!(
        extract_palette(document),
        vec!["#0A0A0A", "#2563EB", "#333333"]
    );
}

/// A document that declares no palette still has one — in its fills. The
/// fallback ranks by how often the file reaches for each colour, so the band
/// shows what the design is actually made of.
#[test]
fn a_document_without_variables_falls_back_to_its_most_used_fills() {
    let document = r##"{
        "variables": { "c-only": { "type": "color", "value": "#ABCDEF" } },
        "children": [
            { "fill": [{ "type": "solid", "color": "#101010" }], "children": [
                { "fill": [{ "type": "solid", "color": "#FAFAFA" }] },
                { "fill": [{ "type": "solid", "color": "#FAFAFA" }] },
                { "fill": [{ "type": "solid", "color": "#FAFAFA" }] },
                { "fill": [{ "type": "solid", "color": "#101010" }] },
                { "fill": [{ "type": "solid", "color": "#CC0000" }] }
            ]}
        ]
    }"##;
    assert_eq!(
        extract_palette(document),
        vec!["#FAFAFA", "#101010", "#CC0000"],
        "most-painted first, then first-appearance order for the rest"
    );
}

#[test]
fn an_unreadable_or_unknown_document_yields_no_band_rather_than_a_guess() {
    assert!(extract_palette("not json at all").is_empty());
    assert!(extract_palette("{}").is_empty());
    assert!(scene_template_palette("no-such-template").is_empty());
}

/// The memo is the reason this module exists: the gallery repaints every
/// frame, and re-reading tens of kilobytes of JSON per card per frame is the
/// cost it was written to avoid. A second call for the same id must parse
/// nothing at all.
#[test]
fn a_second_lookup_of_the_same_template_parses_nothing() {
    // Warm the entry first — the *first* call is allowed to parse.
    let first = scene_template_palette("slide-deck");
    let parses_before = palette_parse_count();

    let second = scene_template_palette("slide-deck");
    let third = scene_template_palette("slide-deck");

    assert_eq!(
        palette_parse_count(),
        parses_before,
        "a memoized id must not re-parse its document"
    );
    assert_eq!(first, second);
    assert_eq!(first, third);
}
