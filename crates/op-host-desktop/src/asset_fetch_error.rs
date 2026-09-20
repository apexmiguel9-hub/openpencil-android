//! Typed failures for the desktop host's remote-asset workers — one enum for
//! the three background fetchers that pull an asset over the network and drop
//! the outcome into an editor-state field: `iconify_host.rs` (icon-set search
//! pages), `remote_image_host.rs` (`http(s)` image `src` values the canvas
//! painter recorded as cache misses), and the Generate half of
//! `image_panel_host.rs`.
//!
//! They are one failure domain on purpose: all three run the same
//! `block_on_anywhere` + `reqwest` shape on a worker thread, all three deliver
//! through an `mpsc` channel the per-frame pump drains, and all three end in
//! "the asset did not arrive" rather than anything the document must roll back.
//! Splitting them would have produced three enums with the same two transport
//! variants.
//!
//! Style follows `op_orchestrator::OrchestratorError`: a plain enum plus a
//! hand-written `Display`, no `thiserror` and no new dependency. `Display`
//! reproduces the exact sentence the stringly code produced — the icon
//! picker's inline error row and the image panel's error state (truncated to
//! 200 chars, TS parity) render it directly.
//!
//! What the enum adds is that a REFUSAL by our own guards (size cap, empty
//! body, not-an-image sniff) is now distinct from a transport failure, so the
//! remote-image negative cache could learn to treat them differently without
//! matching on prose.
//!
//! Two seams carry `String` payloads: `reqwest`'s errors, and the message
//! from `image_generate_host::run_generate_blocking`. Both are adapted with
//! `e.to_string()` so they survive those sources later typing their own
//! errors.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AssetFetchError {
    /// The HTTP client could not be built (TLS backend / timeout config).
    /// Never a server fault — the request was not sent.
    HttpClient(String),
    /// The request failed, returned a non-success status, or its body did not
    /// decode. Carries `reqwest`'s message verbatim.
    Request(String),
    /// The response is larger than the per-entry cap the paint-side byte
    /// cache is sized against. Detected from `Content-Length` before the body
    /// is read when the server declares one, from the body length otherwise.
    TooLarge,
    /// The response body is empty, so there is nothing to decode.
    EmptyBody,
    /// The response is neither typed `image/*` nor recognised by the
    /// magic-byte sniff — usually an HTML error page served with 200.
    NotAnImage,
    /// An image-generation provider refused. Carries the message from
    /// `image_generate_host::run_generate_blocking` verbatim.
    Generate(String),
    /// The generate worker dropped its sender without reporting.
    GenerateWorkerVanished,
}

impl fmt::Display for AssetFetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AssetFetchError::HttpClient(message)
            | AssetFetchError::Request(message)
            | AssetFetchError::Generate(message) => f.write_str(message),
            AssetFetchError::TooLarge => f.write_str("image exceeds the size cap"),
            AssetFetchError::EmptyBody => f.write_str("empty response body"),
            AssetFetchError::NotAnImage => f.write_str("response is not an image"),
            AssetFetchError::GenerateWorkerVanished => {
                f.write_str("image generation worker vanished")
            }
        }
    }
}

impl std::error::Error for AssetFetchError {}
