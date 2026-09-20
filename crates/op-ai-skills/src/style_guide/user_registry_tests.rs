//! Imported-catalogue tests.
//!
//! The registry is process-global by design — one editor, one set of imported
//! styles — so every case here takes [`exclusive`] and starts from an empty
//! store. Without that, cargo's parallel threads would each see whatever the
//! others had just imported.

use super::exclusive_registry_for_tests as exclusive;
use super::*;

const NORDIC: &str =
    "---\nname: My Nordic\ntags: [minimal]\n---\n\n# My Nordic\n\nAccent #2563EB.\n";

#[test]
fn an_import_becomes_findable_by_id_and_by_name() {
    let _guard = exclusive();
    let imported = import_design_md(NORDIC, "nordic.md").expect("imports");
    assert_eq!(imported.id, "user:my-nordic");
    assert_eq!(imported.slug(), "my-nordic");
    assert_eq!(imported.swatches, vec!["#2563EB"]);

    let by_id = find_style_guide("user:my-nordic").expect("found by id");
    assert!(by_id.is_user());
    assert_eq!(by_id.id(), "user:my-nordic");
    // Deref reaches the markdown the prompt injects.
    assert!(by_id.content.contains("Accent #2563EB"));

    // A plan that echoed the display name instead of the id still resolves.
    let by_name = find_style_guide("my nordic").expect("found by name");
    assert_eq!(by_name.id(), "user:my-nordic");
}

#[test]
fn the_corpus_still_answers_its_own_names() {
    let _guard = exclusive();
    let builtin = find_style_guide("zen-paper-light").expect("the corpus ships this guide");
    assert!(!builtin.is_user());
    assert_eq!(builtin.id(), "zen-paper-light");
    assert!(find_style_guide("zzz-no-such-style").is_none());
    assert!(find_style_guide("   ").is_none());
}

/// A user file that names itself after a shipped guide must not shadow it —
/// which is the entire reason ids carry a prefix.
#[test]
fn a_user_guide_cannot_shadow_a_corpus_name() {
    let _guard = exclusive();
    import_design_md("---\nname: zen-paper-light\n---\n\nMy own take.\n", "x").expect("imports");

    let resolved = find_style_guide("zen-paper-light").expect("resolves");
    assert!(
        !resolved.is_user(),
        "the bare corpus name must still reach the corpus guide"
    );
    // The import is reachable, just under its own id.
    assert!(find_style_guide("user:zen-paper-light")
        .expect("the import is still there")
        .is_user());
}

#[test]
fn same_named_imports_are_numbered_rather_than_overwritten() {
    let _guard = exclusive();
    let first = import_design_md(NORDIC, "a.md").expect("imports");
    let second = import_design_md(NORDIC, "b.md").expect("imports");
    assert_eq!(first.id, "user:my-nordic");
    assert_eq!(second.id, "user:my-nordic-2");
    assert_eq!(user_style_guides().len(), 2);
}

#[test]
fn re_loading_the_same_id_replaces_it_so_a_rescan_is_idempotent() {
    let _guard = exclusive();
    let entry = |content: &str| UserStyleGuide {
        id: "user:disk".to_string(),
        guide: ParsedStyleGuide {
            name: "Disk".to_string(),
            tags: Vec::new(),
            platform: super::super::Platform::Webapp,
            content: content.to_string(),
        },
        swatches: Vec::new(),
    };
    load_user_style_guide(entry("first"));
    load_user_style_guide(entry("second"));
    assert_eq!(user_style_guides().len(), 1);
    assert_eq!(
        find_style_guide("user:disk").expect("found").content,
        "second"
    );
}

#[test]
fn removing_returns_the_entry_so_the_host_can_delete_its_file() {
    let _guard = exclusive();
    import_design_md(NORDIC, "x").expect("imports");
    assert!(has_user_style_guides());

    let removed = remove_user_style_guide("user:my-nordic").expect("removed");
    assert_eq!(removed.slug(), "my-nordic");
    assert!(!has_user_style_guides());
    assert!(find_style_guide("user:my-nordic").is_none());
    assert!(remove_user_style_guide("user:my-nordic").is_none());
}

#[test]
fn a_malformed_import_registers_nothing() {
    let _guard = exclusive();
    assert!(import_design_md("", "x").is_err());
    assert!(import_design_md("\0\0\0 binary junk that decoded", "x").is_err());
    assert!(user_style_guides().is_empty());
}
