//! Pinned-guide summary tests.

use super::*;
use crate::style_guide::user_registry::exclusive_registry_for_tests as exclusive;
use crate::style_guide::{
    import_design_md, remove_user_style_guide, set_user_style_guides, ParsedStyleGuide,
    UserStyleGuide,
};

/// A token-table guide, the shape whose values only the dialect can read.
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

#[test]
fn a_summary_shows_the_authors_name_not_the_id() {
    let _guard = exclusive();
    let imported = import_design_md(IMPORTED, "dimension.md").expect("imports");
    let summary = style_guide_summary(&imported.id).expect("resolves");

    assert_eq!(summary.id, "user:dimension");
    // `user:dimension` is how the pipeline refers to it; "Dimension" is what
    // its author called it, and the receipt is for a person to read.
    assert_eq!(summary.name, "Dimension");
    assert_eq!(
        summary.swatches,
        vec!["#0A0A0A", "#161616", "#6B62F2", "#EDEDED"]
    );
}

#[test]
fn a_corpus_guide_summarizes_under_its_own_name() {
    let _guard = exclusive();
    let summary = style_guide_summary("zen-paper-light").expect("the corpus ships this guide");
    assert_eq!(summary.id, "zen-paper-light");
    assert_eq!(summary.name, "zen-paper-light");
    assert!(!summary.swatches.is_empty());
}

/// The stale-pin case the receipt must stay silent for: generation has already
/// fallen back to choosing its own style, so naming this guide would be a new
/// lie on top of the old one.
#[test]
fn a_pin_naming_nothing_summarizes_to_nothing() {
    let _guard = exclusive();
    assert!(style_guide_summary("user:deleted-last-week").is_none());
    assert!(style_guide_summary("").is_none());
    assert!(style_guide_summary("   ").is_none());
}

/// A guide with no readable values summarizes with an empty band rather than
/// failing — an empty band is the visible form of "this file's colours could
/// not be read", which is exactly what used to be invisible.
#[test]
fn an_unreadable_guide_summarizes_with_no_swatches() {
    let _guard = exclusive();
    let imported =
        import_design_md("# Prose Only\n\nQuiet, generous, unadorned.\n", "p.md").expect("imports");
    let summary = style_guide_summary(&imported.id).expect("resolves");
    assert_eq!(summary.name, "Prose Only");
    assert!(summary.swatches.is_empty());
}

// ─── Memo invalidation ─────────────────────────────────────────────────
//
// The memo exists because this runs on the chrome paint path. It is only safe
// while every way a guide's content can change also drops it.

#[test]
fn deleting_a_guide_drops_its_memoized_summary() {
    let _guard = exclusive();
    let imported = import_design_md(IMPORTED, "d.md").expect("imports");
    assert!(style_guide_summary(&imported.id).is_some());

    remove_user_style_guide(&imported.id).expect("removed");
    assert!(
        style_guide_summary(&imported.id).is_none(),
        "a deleted guide must not keep answering from the memo"
    );
}

#[test]
fn replacing_a_guide_under_the_same_id_resummarizes_it() {
    let _guard = exclusive();
    let entry = |name: &str, body: &str| UserStyleGuide {
        id: "user:same".to_string(),
        guide: ParsedStyleGuide {
            name: name.to_string(),
            tags: Vec::new(),
            platform: crate::style_guide::Platform::Webapp,
            content: body.to_string(),
        },
        swatches: Vec::new(),
    };
    crate::style_guide::load_user_style_guide(entry("First", "# First\n"));
    assert_eq!(
        style_guide_summary("user:same").expect("resolves").name,
        "First"
    );

    crate::style_guide::load_user_style_guide(entry("Second", "# Second\n"));
    assert_eq!(
        style_guide_summary("user:same").expect("resolves").name,
        "Second",
        "a boot rescan replaces content under a live id"
    );
}

#[test]
fn a_wholesale_reload_drops_everything_memoized() {
    let _guard = exclusive();
    let imported = import_design_md(IMPORTED, "d.md").expect("imports");
    assert!(style_guide_summary(&imported.id).is_some());

    set_user_style_guides(Vec::new());
    assert!(style_guide_summary(&imported.id).is_none());
}

/// Repeated calls must agree — the memo is an optimization, not a behaviour.
#[test]
fn the_memo_returns_the_same_answer_it_computed() {
    let _guard = exclusive();
    let imported = import_design_md(IMPORTED, "d.md").expect("imports");
    let first = style_guide_summary(&imported.id).expect("resolves");
    let second = style_guide_summary(&imported.id).expect("resolves");
    assert_eq!(first, second);
}
