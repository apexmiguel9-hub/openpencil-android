//! Typed failures for the in-process live-GUI MCP server (`mcp_live.rs`):
//! start-up, the per-connection handlers, and every UI-thread round-trip the
//! connection threads make.
//!
//! Style follows `op_orchestrator::OrchestratorError`: a plain enum plus a
//! hand-written `Display`, no `thiserror` and no new dependency. `Display` is
//! transparent — each variant reproduces the exact sentence the stringly
//! code produced, so the stderr lines (`openpencil-desktop mcp: …`), the
//! `{"error":…}` HTTP bodies, and the `Renderer reported failure: …`
//! screenshot envelope are unchanged byte for byte.
//!
//! What the enum adds is the classification this module never had: a UI-ack
//! timeout, a refused MCP message, and a failed raster capture used to be
//! indistinguishable `String`s that the accept loop dumped to stderr and the
//! 500 handler echoed. Two of them now arrive pre-classified from the shared
//! layers ([`McpServeError`], [`ExportError`]) through the `From` impls
//! below, which is what let `mcp_serve`'s three `String` seams
//! (`write_mcp_http_response`, `process_tool_message_with_registry`,
//! `doc_sync`'s validation) and `export`'s `From<ExportError> for String`
//! bridge be deleted — this module was their last stringly consumer.

use std::fmt;

use crate::export::ExportError;
use crate::mcp_serve::McpServeError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpLiveError {
    /// Start-up failed: the listener would not bind, its bound port was
    /// unreadable, or the server thread would not spawn. Never becomes an
    /// HTTP response — it aborts `McpLiveServer::start_with_wake`, and the
    /// desktop host falls back to another port or gives up.
    Startup(String),
    /// A UI-thread round-trip failed: the request channel is closed (the
    /// editor is shutting down) or the ack did not arrive inside the
    /// per-request budget (`UI_ACK_TIMEOUT`, or the caller's own budget for
    /// screenshots). A server fault — the wire answers 500.
    UiThread(String),
    /// The shared MCP transport refused the message or the socket write
    /// failed. Carries [`McpServeError`] so the classification the transport
    /// already made (client fault vs dead connection) survives the hop.
    Transport(McpServeError),
    /// The live raster capture behind `debug_screenshot` failed. Carries
    /// [`ExportError`] so the screenshot glue can report the renderer's own
    /// reason inside its `Renderer reported failure: …` envelope.
    Screenshot(ExportError),
}

impl fmt::Display for McpLiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            McpLiveError::Startup(m) | McpLiveError::UiThread(m) => f.write_str(m),
            McpLiveError::Transport(e) => e.fmt(f),
            McpLiveError::Screenshot(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for McpLiveError {}

/// Lets every `mcp_serve` primitive the connection handlers reach for
/// (`read_http_request`, `write_mcp_http_response`,
/// `process_message_with_applier`, `process_tool_message_with_registry`,
/// `parse_document_sync_request`, `file_path::process_message_for_file_path_arg`)
/// be used with a plain `?`. Both `Display` impls are transparent, so the
/// resulting log line / response body is byte-identical to the
/// pre-conversion text.
impl From<McpServeError> for McpLiveError {
    fn from(error: McpServeError) -> McpLiveError {
        McpLiveError::Transport(error)
    }
}

/// Same for the UI pump's raster capture, so `request_screenshot` can hand
/// the export core's reason straight to the screenshot wire glue.
impl From<ExportError> for McpLiveError {
    fn from(error: ExportError) -> McpLiveError {
        McpLiveError::Screenshot(error)
    }
}
