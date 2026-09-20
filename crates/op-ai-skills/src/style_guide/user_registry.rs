//! The runtime half of the style-guide catalogue: guides the user imported.
//!
//! The shipped corpus is baked into the binary and never changes, so it can be
//! a `&'static` slice. A user's `DESIGN.md` arrives while the editor is
//! running, can be deleted while it is running, and on the web has nowhere to
//! be written down at all — so it lives here instead, behind a process-global
//! lock, and every consumer that used to reach straight for
//! [`style_guide_registry`](super::style_guide_registry) reaches for
//! [`find_style_guide`] instead.
//!
//! Two sources, one lookup. That is the whole design: pinning, planning, and
//! sub-agent injection all resolve a *name* to markdown, and none of them
//! should have to know which half of the catalogue the answer came from.
//!
//! This module holds no files. Persistence is a host capability (the desktop
//! writes `~/.openpencil/styles/*.md`; the browser has no disk), so hosts call
//! [`load_user_style_guide`] on boot and [`import_design_md`] on demand, and
//! this stays a pure in-memory catalogue that compiles for wasm32 unchanged.

use std::sync::{Arc, RwLock};

use super::design_md_import::{parse_design_md, slugify, DesignMdImportError};
use super::types::ParsedStyleGuide;

/// Prefix separating imported ids from corpus names.
///
/// Corpus guides are pinned by their bare name, so a user file called
/// `zen-paper-light` would otherwise silently shadow (or be shadowed by) the
/// shipped guide of that name depending on lookup order.
pub const USER_STYLE_ID_PREFIX: &str = "user:";

/// A style guide the user imported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserStyleGuide {
    /// Stable id, `user:<slug>`. This is what pinning stores and what the
    /// host derives the on-disk file name from.
    pub id: String,
    /// Name, tags, platform, and the verbatim markdown.
    pub guide: ParsedStyleGuide,
    /// Hex colours for the card's band, in document order.
    pub swatches: Vec<String>,
}

impl UserStyleGuide {
    /// The `<slug>` half of [`Self::id`] — the host's file stem.
    pub fn slug(&self) -> &str {
        self.id
            .strip_prefix(USER_STYLE_ID_PREFIX)
            .unwrap_or(&self.id)
    }
}

/// A guide from either half of the catalogue.
///
/// Derefs to [`ParsedStyleGuide`] so callers that only want `name` / `tags` /
/// `content` — which is all of them but the Asset Center — read the same as
/// they did when the corpus was the only source.
#[derive(Debug, Clone)]
pub enum StyleGuideRef {
    /// Baked into the binary.
    Builtin(&'static ParsedStyleGuide),
    /// Imported at runtime.
    User(Arc<UserStyleGuide>),
}

impl StyleGuideRef {
    /// The id to pin: the bare name for corpus guides, `user:<slug>` for
    /// imported ones.
    pub fn id(&self) -> &str {
        match self {
            StyleGuideRef::Builtin(guide) => guide.name.as_str(),
            StyleGuideRef::User(user) => user.id.as_str(),
        }
    }

    /// Whether this came from an import.
    pub fn is_user(&self) -> bool {
        matches!(self, StyleGuideRef::User(_))
    }
}

impl std::ops::Deref for StyleGuideRef {
    type Target = ParsedStyleGuide;

    fn deref(&self) -> &ParsedStyleGuide {
        match self {
            StyleGuideRef::Builtin(guide) => guide,
            StyleGuideRef::User(user) => &user.guide,
        }
    }
}

fn store() -> &'static RwLock<Vec<Arc<UserStyleGuide>>> {
    static STORE: RwLock<Vec<Arc<UserStyleGuide>>> = RwLock::new(Vec::new());
    &STORE
}

/// Every imported guide, in import order.
///
/// A poisoned lock returns an empty catalogue rather than propagating the
/// panic: this is read on the paint path, and a design tool that stops drawing
/// because a style list could not be locked is a worse outcome than one that
/// shows no user styles for the rest of the session.
pub fn user_style_guides() -> Vec<Arc<UserStyleGuide>> {
    store()
        .read()
        .map(|guides| guides.clone())
        .unwrap_or_default()
}

/// Whether anything has been imported.
pub fn has_user_style_guides() -> bool {
    store().read().map(|g| !g.is_empty()).unwrap_or(false)
}

/// Replace the whole imported set — the host's boot-time scan.
pub fn set_user_style_guides(guides: Vec<UserStyleGuide>) {
    if let Ok(mut store) = store().write() {
        *store = guides.into_iter().map(Arc::new).collect();
    }
    super::summary::invalidate_summaries();
}

/// Forget an imported guide. Returns it, so the host knows which file to
/// delete, or `None` when the id was already gone.
pub fn remove_user_style_guide(id: &str) -> Option<Arc<UserStyleGuide>> {
    let removed = {
        let mut store = store().write().ok()?;
        let index = store.iter().position(|guide| guide.id == id)?;
        store.remove(index)
    };
    super::summary::invalidate_summaries();
    Some(removed)
}

/// Register an already-parsed guide under an id the caller chose — the shape
/// the desktop's boot scan needs, where the id must match the file on disk
/// rather than be freshly allocated.
///
/// Re-registering an existing id replaces it in place, so re-scanning a
/// directory is idempotent instead of doubling every card.
pub fn load_user_style_guide(guide: UserStyleGuide) -> Arc<UserStyleGuide> {
    let entry = Arc::new(guide);
    if let Ok(mut store) = store().write() {
        match store.iter().position(|held| held.id == entry.id) {
            Some(index) => store[index] = Arc::clone(&entry),
            None => store.push(Arc::clone(&entry)),
        }
    }
    super::summary::invalidate_summaries();
    entry
}

/// Parse a `DESIGN.md` and register it under a freshly allocated id.
///
/// `fallback_name` names the guide when the document does not name itself —
/// the hosts pass the file stem, or a generic label for a paste.
pub fn import_design_md(
    raw: &str,
    fallback_name: &str,
) -> Result<Arc<UserStyleGuide>, DesignMdImportError> {
    let parsed = parse_design_md(raw, fallback_name)?;
    let id = allocate_id(&slugify(&parsed.name));
    Ok(load_user_style_guide(UserStyleGuide {
        id,
        guide: ParsedStyleGuide {
            name: parsed.name,
            tags: parsed.tags,
            platform: parsed.platform,
            content: parsed.content,
        },
        swatches: parsed.swatches,
    }))
}

/// `user:<slug>`, with `-2`, `-3`… appended until it is free.
///
/// Importing two files that name themselves the same thing is ordinary — a
/// user collecting variants of one aesthetic will do it on the first
/// afternoon — so a collision numbers the newcomer rather than overwriting
/// what is already pinned.
fn allocate_id(slug: &str) -> String {
    let taken = |candidate: &str| {
        store()
            .read()
            .map(|store| store.iter().any(|guide| guide.id == candidate))
            .unwrap_or(false)
    };
    let base = format!("{USER_STYLE_ID_PREFIX}{slug}");
    if !taken(&base) {
        return base;
    }
    for suffix in 2..1000 {
        let candidate = format!("{base}-{suffix}");
        if !taken(&candidate) {
            return candidate;
        }
    }
    format!("{base}-{}", user_style_guides().len() + 1)
}

/// Resolve a pinned id or a plan's `styleGuideName` against both halves of
/// the catalogue.
///
/// Order is deliberate. The `user:` prefix is decisive when present, because
/// that is an id the Asset Center wrote and it must never resolve to a corpus
/// guide. Otherwise the corpus is tried first — its names are the vocabulary
/// planning prompts hand the model — and only then imported guides by display
/// name, which is what makes a plan that echoed a user guide's *name* (rather
/// than its id) still find it.
pub fn find_style_guide(query: &str) -> Option<StyleGuideRef> {
    let query = query.trim();
    if query.is_empty() {
        return None;
    }
    if query.starts_with(USER_STYLE_ID_PREFIX) {
        return user_style_guides()
            .into_iter()
            .find(|guide| guide.id == query)
            .map(StyleGuideRef::User);
    }
    if let Some(guide) = super::style_guide_registry()
        .iter()
        .find(|guide| guide.name == query)
    {
        return Some(StyleGuideRef::Builtin(guide));
    }
    user_style_guides()
        .into_iter()
        .find(|guide| guide.guide.name.eq_ignore_ascii_case(query))
        .map(StyleGuideRef::User)
}

/// Serialized, emptied access to the process-global imported catalogue.
///
/// One lock for the whole crate, deliberately. Every test module that touches
/// the registry shares this store, and giving each its own mutex — which is
/// what the first version did — lets two modules run concurrently and empty
/// the registry out from under each other. It failed exactly that way: a
/// delete in one module could not find the guide another had just cleared.
#[cfg(test)]
pub(crate) fn exclusive_registry_for_tests() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let guard = LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    set_user_style_guides(Vec::new());
    guard
}

#[cfg(test)]
#[path = "user_registry_tests.rs"]
mod user_registry_tests;
