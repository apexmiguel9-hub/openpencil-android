//! Typed failures for the headless `.op` / `.pen` document IO core
//! (`doc_io.rs` and its `atomic_file` / `canonical_save` / `clean_copy` /
//! `load` siblings).
//!
//! Style follows `op_orchestrator::OrchestratorError`: a plain enum plus a
//! hand-written `Display`, no `thiserror` and no new dependency. The
//! filesystem variants carry STRUCTURED fields and `Display` re-formats the
//! sentence, so the text the desktop error dialog and the daemon's 400 bodies
//! show is reproduced byte for byte while callers can match on the reason
//! instead of the prose.
//!
//! What the enum adds is a phase classification the flat strings could not
//! express: whether a failure is a FILESYSTEM fault on a path the user named
//! (`Open` / `Stat` / `Map` / `CreateTemp` / `TempAllocExhausted` / `Replace` /
//! `Io`), a CONTENT fault in the bytes themselves (`SourceEmpty` /
//! `SourceNotUtf8` / `InvalidUtf8Document` / `Schema` / `LegacyFormat`), or a
//! SERIALIZER fault while writing (`Serialize`). That middle group is the
//! client-fault / server-fault split a request boundary needs — a daemon route
//! handed a bad document answers `400`, while a disk that would not open
//! answers `500` — and it is now a `match` on the variant rather than a
//! substring test on the prose. `From<DocIoError> for WebCanvasError` in
//! `web_canvas_server_error.rs` is where that split is consumed.
//!
//! ## No `String` bridge
//!
//! There is none, and nothing is left that would want one. Every entry point
//! in this module reports [`DocIoError`], and every consumer — this crate's
//! own routes and all of `op-host-desktop` — carries a typed error of its own,
//! so nothing reaches these functions from a `Result<_, String>` body any
//! more. An `impl From<DocIoError> for String` lived here transitionally while
//! `op-host-desktop` was still stringly typed, and was removed once that crate
//! converted. Do not reintroduce it: a caller that genuinely needs a `String`
//! wants `error.to_string()` at ITS boundary, where the erasure is visible,
//! rather than a blanket conversion that lets `?` silently discard the
//! classification above.
//!
//! Three inbound seams belong to crates this pass does not own, and their
//! messages are carried verbatim rather than re-worded: `op_pen_loader`'s
//! source writers and canonical loader (`Serialize` / `Schema`),
//! `jian_ops_schema::image_table`'s streaming writer (`Serialize`), and
//! `op_i18n`'s localised open-failure sentences (`LegacyFormat` /
//! `InvalidUtf8Document`), which are end-user copy rather than diagnostics.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocIoError {
    /// A sibling temporary file could not be created next to the save target.
    CreateTemp { path: String, detail: String },
    /// 128 candidate sibling names were all taken — a stuck directory rather
    /// than a transient collision between overlapping saves.
    TempAllocExhausted { path: String },
    /// The completed temporary file could not be installed at its
    /// destination. The previous file is still intact.
    Replace {
        destination: String,
        temp: String,
        detail: String,
    },
    /// Opening a source document for reading failed.
    Open { path: String, detail: String },
    /// Reading a source document's metadata failed.
    Stat { path: String, detail: String },
    /// A zero-length source document — the clean-copy Save As path refuses it
    /// rather than publishing an empty file over a good one.
    SourceEmpty { path: String },
    /// Memory-mapping a source document failed.
    Map { path: String, detail: String },
    /// A clean-copy source is not UTF-8, so it cannot be a `.op` file. Worded
    /// as a diagnostic — [`DocIoError::InvalidUtf8Document`] is the localised
    /// end-user twin on the interactive open path.
    SourceNotUtf8 { path: String, detail: String },
    /// A bare `std::io` failure on the document path (open / stat / map /
    /// flush) whose message is the OS's own. Carried verbatim because the
    /// pre-conversion code emitted exactly `error.to_string()` here.
    Io(String),
    /// The streaming writer (`jian_ops_schema::image_table`) or one of
    /// `op_pen_loader`'s source writers refused to serialize the document.
    /// Both live in crates this pass does not own, so the message is carried
    /// verbatim.
    Serialize(String),
    /// The canonical loader rejected the document's schema. Message carried
    /// verbatim from `op_pen_loader`.
    Schema(String),
    /// The file is a pre-canonical private `DocPayload`, which this build has
    /// no converter for. Carries the LOCALISED `dialog.loadErrorOldVersion`
    /// sentence — end-user copy, asserted on by `doc_io`'s own tests.
    LegacyFormat(String),
    /// The opened document is not UTF-8. Carries the LOCALISED
    /// `dialog.loadErrorInvalidUtf8` sentence with its `{{detail}}` already
    /// substituted.
    InvalidUtf8Document(String),
}

impl fmt::Display for DocIoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocIoError::CreateTemp { path, detail } => write!(f, "create {path}: {detail}"),
            DocIoError::TempAllocExhausted { path } => {
                write!(f, "could not create a unique save file beside {path}")
            }
            DocIoError::Replace {
                destination,
                temp,
                detail,
            } => write!(f, "replace {destination} with {temp}: {detail}"),
            DocIoError::Open { path, detail } => write!(f, "open {path}: {detail}"),
            DocIoError::Stat { path, detail } => write!(f, "stat {path}: {detail}"),
            DocIoError::SourceEmpty { path } => write!(f, "source document {path} is empty"),
            DocIoError::Map { path, detail } => write!(f, "map {path}: {detail}"),
            DocIoError::SourceNotUtf8 { path, detail } => {
                write!(f, "{path} is not UTF-8 JSON: {detail}")
            }
            DocIoError::Io(message)
            | DocIoError::Serialize(message)
            | DocIoError::Schema(message)
            | DocIoError::LegacyFormat(message)
            | DocIoError::InvalidUtf8Document(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for DocIoError {}
