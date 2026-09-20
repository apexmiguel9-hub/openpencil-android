//! Scope enforcement for the REST tier.
//!
//! The MCP dispatch has enforced scopes since M3, but `/api/*` did not — so a
//! read-only API token could simply `POST /api/mcp/document` and replace the
//! whole document, which is strictly more damage than any single tool call
//! could do. Same credential, same deployment, two different answers.
//!
//! This closes that: an API-token identity is held to the same scopes on REST
//! that it is on `/mcp`. A browser session is not — it IS the account, and
//! scopes exist to narrow a token below the account's own authority.

use crate::mcp_serve::tool_profile::{McpScopes, MCP_READ_SCOPE, MCP_WRITE_SCOPE};

use super::tenant_auth::IdentityVia;

/// Which scope a REST request needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestScope {
    Read,
    Write,
}

impl RestScope {
    const fn name(self) -> &'static str {
        match self {
            Self::Read => MCP_READ_SCOPE,
            Self::Write => MCP_WRITE_SCOPE,
        }
    }

    const fn allowed_by(self, scopes: McpScopes) -> bool {
        match self {
            Self::Read => scopes.can_read(),
            Self::Write => scopes.can_write(),
        }
    }
}

/// Why a REST request was refused on scope grounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestScopeRefusal {
    required: RestScope,
}

impl RestScopeRefusal {
    pub const fn code(self) -> &'static str {
        "scope-insufficient"
    }

    pub const fn http_status(self) -> &'static str {
        "403 Forbidden"
    }
}

impl std::fmt::Display for RestScopeRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "this credential lacks the '{}' scope",
            self.required.name()
        )
    }
}

impl std::error::Error for RestScopeRefusal {}

/// Decide whether a REST request may proceed.
///
/// `None` when it may. A session-cookie identity is never refused here.
pub(super) fn check_rest_scope(
    via: IdentityVia,
    scopes: McpScopes,
    method: &str,
    path: &str,
) -> Option<RestScopeRefusal> {
    if via == IdentityVia::SessionCookie {
        return None;
    }
    let required = super::online_policy::rest_scope_required(method, path)?;
    (!required.allowed_by(scopes)).then_some(RestScopeRefusal { required })
}

#[cfg(test)]
#[path = "tool_scopes_tests.rs"]
mod tests;
