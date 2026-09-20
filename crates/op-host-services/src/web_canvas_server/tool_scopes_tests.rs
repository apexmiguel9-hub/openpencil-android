//! Tests for REST scope enforcement.

use super::*;

const READ_ONLY: McpScopes = McpScopes::READ_ONLY;
const FULL: McpScopes = McpScopes::FULL;
const NONE: McpScopes = McpScopes::NONE;

fn refused(scopes: McpScopes, method: &str, path: &str) -> bool {
    check_rest_scope(IdentityVia::ApiToken, scopes, method, path).is_some()
}

#[test]
fn a_read_only_token_cannot_write_the_document_over_rest() {
    // The bypass this closes: the same credential is refused `add_page` on
    // /mcp but could replace the entire document here.
    assert!(refused(READ_ONLY, "POST", "/api/mcp/document"));
    assert!(refused(READ_ONLY, "POST", "/api/mcp/selection"));
    assert!(refused(READ_ONLY, "POST", "/api/settings/credentials"));
    assert!(refused(READ_ONLY, "DELETE", "/api/mcp/document"));
    assert!(refused(READ_ONLY, "PUT", "/api/mcp/document"));
}

#[test]
fn a_read_only_token_may_still_read() {
    assert!(!refused(READ_ONLY, "GET", "/api/mcp/document"));
    assert!(!refused(READ_ONLY, "GET", "/api/mcp/version"));
    assert!(!refused(READ_ONLY, "GET", "/api/mcp/events"));
}

#[test]
fn a_full_scope_token_is_unrestricted() {
    for (method, path) in [
        ("GET", "/api/mcp/document"),
        ("POST", "/api/mcp/document"),
        ("DELETE", "/api/share/grant"),
    ] {
        assert!(!refused(FULL, method, path), "{method} {path}");
    }
}

#[test]
fn a_token_with_no_scopes_is_refused_everything_but_the_health_probe() {
    // Fail-closed: op-hub issues no tokens yet, so tightening this costs
    // nothing and removes a default that would be hard to tighten later.
    assert!(refused(NONE, "GET", "/api/mcp/document"));
    assert!(refused(NONE, "POST", "/api/mcp/document"));
    // …except the probe, so a client can discover the daemon and be told why.
    assert!(!refused(NONE, "GET", "/api/mcp/server"));
    // …and except JSON-RPC, where the MCP dispatch enforces per-tool scopes
    // and would otherwise be refused wholesale by the coarse method rule.
    assert!(!refused(NONE, "POST", "/mcp"));
    assert!(!refused(READ_ONLY, "POST", "/mcp"));
}

#[test]
fn a_browser_session_is_never_scope_limited() {
    // A session IS the account; scopes exist to narrow a token below it.
    for scopes in [NONE, READ_ONLY, FULL] {
        for (method, path) in [
            ("GET", "/api/mcp/document"),
            ("POST", "/api/mcp/document"),
            ("POST", "/api/share/grant"),
        ] {
            assert!(
                check_rest_scope(IdentityVia::SessionCookie, scopes, method, path).is_none(),
                "{method} {path}"
            );
        }
    }
}

#[test]
fn an_unknown_method_needs_write() {
    // A route or verb added later is covered by default, in the strict
    // direction.
    assert!(refused(READ_ONLY, "PATCH", "/api/anything/new"));
    assert!(refused(READ_ONLY, "PROPFIND", "/api/anything/new"));
}

#[test]
fn a_refusal_is_a_typed_403_naming_the_missing_scope() {
    let refusal = check_rest_scope(
        IdentityVia::ApiToken,
        READ_ONLY,
        "POST",
        "/api/mcp/document",
    )
    .expect("refused");
    assert_eq!(refusal.http_status(), "403 Forbidden");
    assert_eq!(refusal.code(), "scope-insufficient");
    assert!(refusal.to_string().contains("mcp:write"), "{refusal}");

    let read =
        check_rest_scope(IdentityVia::ApiToken, NONE, "GET", "/api/mcp/document").expect("refused");
    assert!(read.to_string().contains("mcp:read"), "{read}");
}
