//! Failure domain for the streaming `editorMeta` / `formatVersion` writers.
//!
//! Lives beside [`crate::editor_meta`] rather than inside it: that module is
//! already close to the repo's 800-line ceiling, and the writers are the only
//! fallible surface it exposes.
//!
//! `Display` reproduces the exact sentences the writers used to return as
//! `String`, so host-side Save-As error dialogs keep their wording.

/// Why a metadata-preserving rewrite of a canonical `.op` source failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorMetaWriteError {
    /// The source bytes are not a top-level JSON object, so there is no
    /// `editorMeta` / `formatVersion` slot to replace or append.
    NotTopLevelObject,
    /// Copying source bytes (or a separator) into the destination writer
    /// failed. Payload is the rendered `std::io::Error`.
    Write(String),
    /// Serializing the replacement `editorMeta` / `formatVersion` value
    /// failed. Payload is the rendered `serde_json::Error`.
    Serialize(String),
}

impl std::fmt::Display for EditorMetaWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EditorMetaWriteError::NotTopLevelObject => {
                f.write_str("source document is not a valid top-level JSON object")
            }
            EditorMetaWriteError::Write(error) | EditorMetaWriteError::Serialize(error) => {
                f.write_str(error)
            }
        }
    }
}

impl std::error::Error for EditorMetaWriteError {}

/// `op-host-services`' Save-As path `?`s these writers inside closures that
/// still collect failures as `String`; this keeps those call sites compiling
/// unchanged while the writers themselves report a typed error.
impl From<EditorMetaWriteError> for String {
    fn from(error: EditorMetaWriteError) -> String {
        error.to_string()
    }
}
