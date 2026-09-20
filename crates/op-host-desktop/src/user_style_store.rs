//! `~/.openpencil/styles/` — the desktop's home for imported style guides.
//!
//! The runtime catalogue (`op_ai_skills::style_guide::user_registry`) is pure
//! memory so it can also serve the browser, which has no disk. This is the
//! half only a desktop can do: scan that directory at boot, write a file when
//! the user imports one, delete it when they remove it.
//!
//! Deliberately **only the disk half**. Deciding what an imported guide is —
//! parsing, allocating its id, registering it — belongs to the shared import
//! flow both hosts and both routes (paste and file) go through. This module
//! once carried a second copy of that decision for the file route, which meant
//! picking a file and pasting the same file could disagree.
//!
//! One `.md` per guide, stored **verbatim**. Not a database, not a re-emitted
//! normalization of what was parsed: the file the user picked is the artifact,
//! and keeping it byte-for-byte means it stays readable by every other tool
//! that speaks `DESIGN.md` — including the editor they wrote it in.
//!
//! The file stem is the id. `user:studio-ochre` lives in `studio-ochre.md`,
//! which is what lets a boot scan re-derive the same ids a previous session
//! pinned against, and what lets a delete find the file from the id alone.

use std::path::PathBuf;

use op_ai_skills::style_guide::{
    parse_design_md, set_user_style_guides, slugify, ParsedStyleGuide, UserStyleGuide,
    USER_STYLE_ID_PREFIX,
};

/// Directory under `~/.openpencil` holding imported style guides.
const STYLES_DIR: &str = "styles";

/// Resolve the styles directory, creating it if this is the first import.
fn styles_dir() -> std::io::Result<PathBuf> {
    crate::test_config_root::guard_user_config();
    let dir = op_config_store::openpencil_dir()?.join(STYLES_DIR);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Load the styles directory into the runtime catalogue, once per process.
///
/// The catalogue is process-global, so scanning it is boot work, not
/// per-document work — and `app_state` is built more than once in a process.
/// Rescanning on each build was measured doing real filesystem work on every
/// construction, and it would also discard any guide imported since boot by
/// replacing the whole catalogue with what happens to be on disk.
pub(crate) fn load_user_style_guides_once() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(load_user_style_guides);
}

/// Load every `.md` in the styles directory into the runtime catalogue.
///
/// A file that no longer parses is skipped with a note rather than failing
/// the scan: the directory is user-editable, and one bad file must not cost
/// them the rest of their collection.
pub(crate) fn load_user_style_guides() {
    let Ok(dir) = styles_dir() else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    let mut guides: Vec<UserStyleGuide> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "md") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let raw = match std::fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(error) => {
                eprintln!("[styles] {}: {error}", path.display());
                continue;
            }
        };
        match parse_design_md(&raw, stem) {
            Ok(parsed) => guides.push(UserStyleGuide {
                id: format!("{USER_STYLE_ID_PREFIX}{stem}"),
                guide: ParsedStyleGuide {
                    name: parsed.name,
                    tags: parsed.tags,
                    platform: parsed.platform,
                    content: parsed.content,
                },
                swatches: parsed.swatches,
            }),
            Err(error) => eprintln!("[styles] {}: {error}", path.display()),
        }
    }
    // Newest last is the order they were added in; the directory hands them
    // back in whatever order the filesystem likes, so sort for a stable list.
    guides.sort_by(|a, b| a.id.cmp(&b.id));
    set_user_style_guides(guides);
}

/// Write an imported guide to disk, by id.
///
/// The guide is already in the runtime catalogue — the shared press flow put
/// it there so the card appears the instant the user confirms — so this only
/// has to make it survive a restart.
pub(crate) fn persist_user_style_guide(id: &str) -> std::io::Result<PathBuf> {
    let guide = op_ai_skills::style_guide::user_style_guides()
        .into_iter()
        .find(|guide| guide.id == id)
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("no imported style guide with id {id}"),
            )
        })?;
    let path = styles_dir()?.join(format!("{}.md", guide.slug()));
    std::fs::write(&path, guide.guide.content.as_bytes())?;
    Ok(path)
}

/// Delete an imported guide's file. A missing file is success — the user
/// asked for it gone, and it is.
pub(crate) fn delete_user_style_guide(id: &str) -> std::io::Result<()> {
    let slug = id.strip_prefix(USER_STYLE_ID_PREFIX).unwrap_or(id);
    // The id is derived from a file name we wrote, but it arrives here through
    // editor state, so it is re-validated rather than trusted: a slug that
    // survives `slugify` unchanged cannot contain a separator or `..`, which
    // is what keeps a delete inside the styles directory.
    if slug.is_empty() || slugify(slug) != slug {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("refusing to delete an unsafe style id: {id}"),
        ));
    }
    let path = styles_dir()?.join(format!("{slug}.md"));
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
#[path = "user_style_store_tests.rs"]
mod user_style_store_tests;
