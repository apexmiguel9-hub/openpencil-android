//! `~/.openpencil/styles/` round-trip tests.
//!
//! These touch two process-global things — the config root and the runtime
//! style catalogue — so they run under one lock and rebuild both from scratch.
//! The config root is redirected once per process by the test harness, which
//! is what keeps them off the developer's real `~/.openpencil`.

use super::*;
use std::sync::{Mutex, MutexGuard};

fn exclusive() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    let guard = LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    crate::test_config_root::guard_user_config();
    set_user_style_guides(Vec::new());
    if let Ok(dir) = styles_dir() {
        let _ = std::fs::remove_dir_all(&dir);
    }
    guard
}

const OCHRE: &str =
    "---\nname: Studio Ochre\n---\n\n# Studio Ochre\n\nWarm ochre. Accent #C77D3A.\n";

fn write_style(stem: &str, body: &str) -> PathBuf {
    let dir = styles_dir().expect("styles dir");
    let path = dir.join(format!("{stem}.md"));
    std::fs::write(&path, body).expect("write");
    path
}

/// The file is the artifact. Re-emitting a normalized form would leave the
/// user with a document their other tools no longer recognize.
///
/// Walks the whole desktop file route as it now runs: the host reads bytes,
/// the shared flow registers them, this module writes them down.
#[test]
fn an_imported_file_is_stored_verbatim_and_reloads_with_the_same_id() {
    let _guard = exclusive();
    let source = write_style("source-file", OCHRE);
    let raw = std::fs::read_to_string(&source).expect("read");
    let id = op_ai_skills::style_guide::import_design_md(&raw, "source-file")
        .expect("imports")
        .id
        .clone();
    assert_eq!(id, "user:studio-ochre");
    let stored = persist_user_style_guide(&id).expect("persists");

    assert_eq!(stored.file_name().unwrap(), "studio-ochre.md");
    assert_eq!(std::fs::read_to_string(&stored).expect("stored"), OCHRE);

    // A fresh session sees the same guide under the same id, which is what
    // makes a pin survive a restart.
    set_user_style_guides(Vec::new());
    load_user_style_guides();
    let loaded = op_ai_skills::style_guide::find_style_guide("user:studio-ochre")
        .expect("the scan found it");
    assert_eq!(loaded.name, "Studio Ochre");
    assert!(loaded.content.contains("Warm ochre"));
}

#[test]
fn persist_writes_a_guide_the_shared_flow_registered() {
    let _guard = exclusive();
    let imported = op_ai_skills::style_guide::import_design_md(OCHRE, "pasted").expect("imports");
    let path = persist_user_style_guide(&imported.id).expect("persists");
    assert_eq!(path.file_name().unwrap(), "studio-ochre.md");
    assert_eq!(std::fs::read_to_string(&path).expect("written"), OCHRE);

    assert!(
        persist_user_style_guide("user:never-registered").is_err(),
        "an unknown id has no content to write"
    );
}

#[test]
fn delete_removes_the_file_and_is_idempotent() {
    let _guard = exclusive();
    let imported = op_ai_skills::style_guide::import_design_md(OCHRE, "x").expect("imports");
    let path = persist_user_style_guide(&imported.id).expect("persists");
    assert!(path.exists());

    delete_user_style_guide(&imported.id).expect("deletes");
    assert!(!path.exists());
    // The user asked for it gone and it is; a second pass must not error.
    delete_user_style_guide(&imported.id).expect("a missing file is success");
}

/// The id reaches this through editor state, so it is re-validated rather
/// than trusted — a delete must not be able to walk out of the directory.
#[test]
fn delete_refuses_a_path_traversing_id() {
    let _guard = exclusive();
    let victim = styles_dir().expect("dir").join("keep-me.md");
    std::fs::write(&victim, OCHRE).expect("write");

    for id in [
        "user:../keep-me",
        "user:..",
        "user:sub/keep-me",
        "user:",
        "user:keep me",
    ] {
        assert!(delete_user_style_guide(id).is_err(), "{id} must be refused");
    }
    assert!(victim.exists(), "nothing outside the directory was touched");
}

/// The directory is user-editable, so one unreadable file must not cost them
/// the rest of the collection.
#[test]
fn a_broken_file_is_skipped_and_the_rest_of_the_scan_survives() {
    let _guard = exclusive();
    write_style("good-one", OCHRE);
    write_style("empty-one", "   ");
    write_style("not-markdown", "\0\0\0binary");
    std::fs::write(styles_dir().expect("dir").join("notes.txt"), OCHRE).expect("write");

    load_user_style_guides();
    let loaded = op_ai_skills::style_guide::user_style_guides();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].id, "user:good-one");
}

/// Two files that name themselves the same thing is ordinary — a user
/// collecting variants of one aesthetic does it on the first afternoon.
#[test]
fn a_second_import_of_the_same_name_is_numbered_not_overwritten() {
    let _guard = exclusive();
    let source = write_style("source-file", OCHRE);
    let raw = std::fs::read_to_string(&source).expect("read");
    let first = op_ai_skills::style_guide::import_design_md(&raw, "source-file")
        .expect("imports")
        .id
        .clone();
    let second = op_ai_skills::style_guide::import_design_md(&raw, "source-file")
        .expect("imports")
        .id
        .clone();
    assert_eq!(first, "user:studio-ochre");
    assert_eq!(second, "user:studio-ochre-2");
    persist_user_style_guide(&first).expect("persists");
    persist_user_style_guide(&second).expect("persists");

    let dir = styles_dir().expect("dir");
    assert!(dir.join("studio-ochre.md").exists());
    assert!(dir.join("studio-ochre-2.md").exists());
}
