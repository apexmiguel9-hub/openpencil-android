//! Open the hub portal's per-account MCP-token page from the signed-in
//! account dropdown.
//!
//! The online web editor is served BY the hub at the hub's own origin, so
//! the portal's `/mcp-tokens` page is reachable relative to
//! `window.location.origin`. This mirrors the sign-in loading popup in
//! `web_auth_sync`: a `window.open(url, "_blank")` issued synchronously
//! inside the click's user-activation window is not popup-blocked.

/// Portal path (relative to the hub origin) that lists / generates
/// per-account MCP access tokens.
const MCP_TOKENS_PATH: &str = "/mcp-tokens";

/// Open the hub portal's MCP-token page in a new browser tab.
///
/// The URL is `<origin>/mcp-tokens`, built from the current page origin so
/// it always addresses the hub that served this editor. Only reached from
/// the online/hub-served host (the row is gated by
/// `EditorUiState::account_mcp_tokens_entry`).
pub(crate) fn open_mcp_tokens_page() {
    let Some(window) = web_sys::window() else {
        return;
    };
    // Prefer the explicit `<origin>/mcp-tokens`; fall back to the bare
    // relative path (the browser resolves it against the origin anyway) if
    // the origin can't be read.
    let url = window
        .location()
        .origin()
        .ok()
        .filter(|origin| !origin.is_empty())
        .map(|origin| format!("{origin}{MCP_TOKENS_PATH}"))
        .unwrap_or_else(|| MCP_TOKENS_PATH.to_string());
    let _ = window.open_with_url_and_target(&url, "_blank");
}
