//! `DESIGN.md` import tests.
//!
//! Two halves, and the second is the point. The first checks that the shapes
//! found in real community files yield a usable guide. The second feeds the
//! parser what a user can actually hand it — a picked file is untrusted input
//! — and asserts only that it comes back with an answer instead of a panic.

use super::*;

#[test]
fn front_matter_names_tags_and_platform_are_lifted() {
    let raw = "---\nname: \"Nordic Frost\"\ntags: [minimal, light-mode]\nplatform: mobile\n---\n\n\
               # Nordic Frost\n\nBackground `#F8FAFC`, accent `#2563EB`.\n";
    let parsed = parse_design_md(raw, "whatever").expect("parses");
    assert_eq!(parsed.name, "Nordic Frost");
    assert_eq!(parsed.tags, vec!["minimal", "light-mode"]);
    assert_eq!(parsed.platform, Platform::Mobile);
    assert_eq!(parsed.swatches, vec!["#F8FAFC", "#2563EB"]);
    // The document is injected into prompts verbatim, so it must survive
    // import byte for byte.
    assert_eq!(parsed.content, raw);
}

#[test]
fn a_leading_yaml_fence_counts_as_the_header() {
    let raw = "```yaml\ntitle: Bauhaus Poster\ntags:\n  - bold-typography\n  - geometric\n```\n\n\
               Primary #E63946.\n";
    let parsed = parse_design_md(raw, "fallback").expect("parses");
    assert_eq!(parsed.name, "Bauhaus Poster");
    assert_eq!(parsed.tags, vec!["bold-typography", "geometric"]);
    assert_eq!(parsed.swatches, vec!["#E63946"]);
}

/// A `yaml` example further down a document is documentation, not the guide's
/// own header — lifting a name out of it would rename the style after one of
/// its own code samples.
#[test]
fn a_yaml_fence_below_the_prose_is_not_the_header() {
    let raw = "# Real Name\n\nSome prose.\n\n```yaml\nname: Example From The Docs\n```\n";
    let parsed = parse_design_md(raw, "fallback").expect("parses");
    assert_eq!(parsed.name, "Real Name");
}

#[test]
fn the_first_heading_names_a_file_with_no_header() {
    let parsed =
        parse_design_md("## Warm Kitchen\n\nSoft cream surfaces.\n", "kitchen").expect("parses");
    assert_eq!(parsed.name, "Warm Kitchen");
    assert!(parsed.tags.is_empty());
    assert_eq!(parsed.platform, Platform::Webapp);
}

#[test]
fn a_file_that_names_itself_nowhere_falls_back_to_the_caller() {
    let parsed = parse_design_md("just prose about a look and feel", "my-styles").expect("parses");
    assert_eq!(parsed.name, "my-styles");

    // …and when even the fallback is blank, to something showable. A card
    // with no name is a card the user cannot pick.
    let parsed = parse_design_md("just prose about a look and feel", "   ").expect("parses");
    assert_eq!(parsed.name, "imported style");
}

#[test]
fn swatches_stop_at_five_and_ignore_non_colours() {
    let raw = "# P\n#111 #222222 #333333 #44444444 #555555 #666666\n\
               issue #1234567890 and #GGGGGG and #12\n";
    let parsed = parse_design_md(raw, "p").expect("parses");
    assert_eq!(
        parsed.swatches,
        vec!["#111", "#222222", "#333333", "#44444444", "#555555"]
    );
}

#[test]
fn repeated_colours_are_kept_once() {
    let parsed =
        parse_design_md("# P\n#ABCDEF then #abcdef again then #123456\n", "p").expect("parses");
    assert_eq!(parsed.swatches, vec!["#ABCDEF", "#123456"]);
}

#[test]
fn a_typography_only_guide_imports_without_colours() {
    let parsed =
        parse_design_md("# Type Only\n\nDisplay: Inter. Body: Inter.\n", "t").expect("parses");
    assert!(parsed.swatches.is_empty());
}

// ─── Malformed input ───────────────────────────────────────────────────
//
// Everything below is a file a user can pick or paste. None of it may panic,
// and each either imports or reports a reason.

#[test]
fn empty_and_near_empty_files_are_refused() {
    assert_eq!(parse_design_md("", "x"), Err(DesignMdImportError::Empty));
    assert_eq!(
        parse_design_md("   \n\t\n  ", "x"),
        Err(DesignMdImportError::Empty)
    );
    assert_eq!(
        parse_design_md("# hi", "x"),
        Err(DesignMdImportError::Empty)
    );
}

#[test]
fn binary_content_is_refused_rather_than_imported_as_prose() {
    let nul = "PK\u{3}\u{4}\0\0\0\0some bytes that decoded fine";
    assert_eq!(parse_design_md(nul, "x"), Err(DesignMdImportError::NotText));

    let control_soup = "\u{7}".repeat(200);
    assert_eq!(
        parse_design_md(&control_soup, "x"),
        Err(DesignMdImportError::NotText)
    );
}

/// A stray control character in an otherwise real document is a quirk of
/// whatever exported it, not a reason to reject the guide.
#[test]
fn a_single_control_character_does_not_condemn_a_real_document() {
    let raw = format!("# Guide\u{7}\n\n{}\n", "prose ".repeat(50));
    assert!(parse_design_md(&raw, "x").is_ok());
}

#[test]
fn oversized_files_are_refused_before_anything_is_scanned() {
    let huge = "a".repeat(MAX_DESIGN_MD_BYTES + 1);
    assert_eq!(
        parse_design_md(&huge, "x"),
        Err(DesignMdImportError::TooLarge)
    );
}

#[test]
fn one_enormous_line_imports_without_scanning_off_the_end() {
    let raw = format!("# Long\n{}#ABCDEF\n", "x".repeat(200_000));
    let parsed = parse_design_md(&raw, "x").expect("a long line is still text");
    assert_eq!(parsed.name, "Long");
    assert_eq!(parsed.swatches, vec!["#ABCDEF"]);
}

#[test]
fn unterminated_front_matter_and_fences_still_import() {
    // Front matter with no closing rule: `split_frontmatter` rejects it, so
    // the H1 below has to be what names the guide.
    let parsed = parse_design_md("---\nname: Never Closed\n\n# Actual Heading\nprose\n", "x")
        .expect("parses");
    assert_eq!(parsed.name, "Actual Heading");

    // An unterminated yaml fence is the header the author was mid-way
    // through writing; failing the import over three missing backticks
    // would be the parser being pedantic about a format with no spec.
    let parsed =
        parse_design_md("```yaml\nname: Half Written\ntags: [calm]\n", "x").expect("parses");
    assert_eq!(parsed.name, "Half Written");
    assert_eq!(parsed.tags, vec!["calm"]);
}

#[test]
fn multibyte_text_is_walked_by_character_not_by_byte() {
    let raw = "# 温暖厨房 🍳\n\ntags 说明:主色 #E07A5F,背景 #FFF8F0。\n";
    let parsed = parse_design_md(raw, "x").expect("parses");
    assert_eq!(parsed.name, "温暖厨房 🍳");
    assert_eq!(parsed.swatches, vec!["#E07A5F", "#FFF8F0"]);
}

#[test]
fn a_hash_at_the_very_end_does_not_read_past_the_document() {
    assert!(parse_design_md("# Guide\nsome prose and a trailing #", "x").is_ok());
    assert!(parse_design_md("# Guide\nsome prose and a trailing #AB", "x").is_ok());
}

#[test]
fn slugs_are_file_safe_and_never_empty() {
    assert_eq!(slugify("Nordic Frost / Light"), "nordic-frost-light");
    assert_eq!(slugify("  ...  "), "style");
    assert_eq!(slugify(""), "style");
    assert_eq!(slugify("温暖厨房"), "温暖厨房");
    assert_eq!(slugify("a".repeat(200).as_str()).chars().count(), 48);
    // No leading or trailing separator, whatever the input punctuation.
    let slug = slugify("--Hello, World!--");
    assert_eq!(slug, "hello-world");
}
