//! Typed failure for imported-UIKit persistence (`kit_persistence.rs`) —
//! `<config>/openpencil/uikits.json`.
//!
//! Style follows `op_orchestrator::OrchestratorError`: a plain enum plus a
//! hand-written `Display`, no `thiserror` and no new dependency. `Display` is
//! transparent — the stringly code carried the bare `serde_json` / `std::io`
//! message with no envelope of its own, and `kit_persistence::save` prints it
//! inside `openpencil-desktop: uikits.json save failed: {e}`, so the line is
//! unchanged byte for byte.
//!
//! Saving here is deliberately best-effort (the retired TS `persist()`
//! swallowed quota errors the same way), which is exactly why the enum is
//! worth having: the caller logs and moves on, so the only way to ever tell
//! an unserializable kit from an unwritable config directory is for the
//! failure to say which step it was. All three variants adapt crates this
//! pass does not own with `e.to_string()`.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum KitStoreError {
    /// The reconstituted `PersistedState` would not serialize to JSON.
    Encode(String),
    /// The config directory could not be created.
    CreateDir(String),
    /// Writing `uikits.json` failed.
    Write(String),
}

impl fmt::Display for KitStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KitStoreError::Encode(message)
            | KitStoreError::CreateDir(message)
            | KitStoreError::Write(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for KitStoreError {}
