//! Typed failures for the client-side live-sync protocol in
//! [`crate::web_sync`].
//!
//! Split into its own module so `web_sync.rs` stays under the repository's
//! 800-line ceiling. The `Display` strings are byte-identical to the ad-hoc
//! `String` messages this enum replaced, so any log or transcript that
//! surfaced them reads exactly the same.

/// A malformed `/api/mcp/document` exchange.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebSyncError {
    /// The response body is not valid JSON.
    ResponseParse(String),
    /// The response wrapper carries no numeric `version` field.
    MissingVersion,
    /// The response wrapper carries no `document` field.
    MissingDocument,
    /// The `document` field is not a canonical `PenDocument`.
    DocumentParse(String),
    /// The local document could not be serialized for a push.
    SerializeDocument(String),
}

impl std::fmt::Display for WebSyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebSyncError::ResponseParse(e) => write!(f, "sync response parse: {e}"),
            WebSyncError::MissingVersion => write!(f, "sync response missing numeric `version`"),
            WebSyncError::MissingDocument => write!(f, "sync response missing `document`"),
            WebSyncError::DocumentParse(e) => write!(f, "sync response document parse: {e}"),
            WebSyncError::SerializeDocument(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for WebSyncError {}

impl From<WebSyncError> for String {
    fn from(error: WebSyncError) -> String {
        error.to_string()
    }
}
