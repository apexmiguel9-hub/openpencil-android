//! Typed failures for the desktop MCP transports (`mcp_serve.rs`): the
//! stdio server, the `--mcp-http` server, and the shared HTTP
//! request/response primitives every host reuses.
//!
//! Style follows `op_orchestrator::OrchestratorError`: a plain enum plus a
//! hand-written `Display`, no `thiserror` and no new dependency. `Display`
//! is transparent — each variant carries the exact sentence the
//! stringly-typed code produced, so stderr lines, HTTP error bodies and the
//! messages tests assert on are unchanged byte for byte.
//!
//! What the enum adds is a transport-independent classification. Its main
//! consumer is the `--serve-web` daemon, which used to re-label every
//! `mcp_serve` string by hand at the call site
//! (`.map_err(WebCanvasError::Transport)` / `::Document` / `::BadRequest`).
//! With [`From<McpServeError> for WebCanvasError`] that mapping lives in one
//! table and the call sites are plain `?`.
//!
//! The whole module surface is typed now. The three seams that used to keep
//! a `String` for `mcp_live.rs`'s sake — [`super::write_mcp_http_response`]
//! (the permissive-CORS wrapper), [`super::process_tool_message_with_registry`]
//! (the lightweight-tool dispatch), and `doc_sync`'s document-sync validation
//! — all report this enum, because `mcp_live` converted to its own
//! [`crate::mcp_live::McpLiveError`] and absorbs them through
//! `From<McpServeError>`. The `From<McpServeError> for String` bridge those
//! seams needed is gone with them.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpServeError {
    /// Loading, saving, or seeding the `.op` file this transport is backed
    /// by failed. A client fault when the path came from a tool call's
    /// `filePath`, a start-up fault when it came from argv.
    Document(String),
    /// The MCP wire pipeline (parser → registry → serializer) refused the
    /// message. The document was not touched.
    Dispatch(String),
    /// The peer's HTTP framing is malformed, truncated, or over a declared
    /// size cap — a client fault detected before any handler ran.
    Protocol(String),
    /// A route's own framing precondition refused the request BEFORE the
    /// body was read: `413` when the declared `Content-Length` exceeds that
    /// route's cap, `411` when a route that requires the header did not get
    /// one. Distinct from [`McpServeError::Protocol`] because nothing was
    /// consumed off the socket — the caller answers with `status` instead of
    /// logging a 500. Carries the status line so the transport does not have
    /// to re-derive it from the message.
    Framing {
        status: &'static str,
        message: String,
    },
    /// A REST route's own body validation refused the payload: the framing
    /// parsed, but the JSON does not describe what the route needs (today
    /// only `doc_sync`'s `/api/mcp/document` envelope check). A client fault
    /// answered with 400, distinct from [`McpServeError::Protocol`] — which
    /// means the CONNECTION is unusable and is only ever logged.
    Validation(String),
    /// A socket or stdio read/write failed. The connection is already
    /// unusable; callers log it rather than answering with it.
    Io(String),
    /// Start-up configuration failed: the listener would not bind. Never
    /// becomes a response — it aborts the server.
    Config(String),
}

impl fmt::Display for McpServeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            McpServeError::Document(m)
            | McpServeError::Dispatch(m)
            | McpServeError::Protocol(m)
            | McpServeError::Validation(m)
            | McpServeError::Io(m)
            | McpServeError::Config(m)
            | McpServeError::Framing { message: m, .. } => f.write_str(m),
        }
    }
}

impl std::error::Error for McpServeError {}
