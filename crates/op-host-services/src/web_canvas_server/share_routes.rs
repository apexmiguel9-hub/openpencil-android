//! `/api/share/*` handlers — who else may open this account's document.
//!
//! Online only. The local and managed daemons have one document and one
//! operator, so there is nothing to share and these routes are not mounted.
//!
//! ## The one invariant
//!
//! The grantor is always the request's VERIFIED identity. The body names the
//! account being granted, never the account doing the granting — otherwise
//! any caller could add themselves to any document's access list, which is
//! the whole security property inverted.
//!
//! Grants run on the connection thread rather than under the document lock:
//! the access list is its own mutex on the tenant, so sharing is answerable
//! while a large document push is in flight.

use op_editor_core::share_routes;

use super::tenant::{AclChange, TenantLease, TenantRegistry};
use super::tenant_auth::ResolvedIdentity;
use super::WebReply;

/// Longest body either POST accepts. Both are a single short account id.
const MAX_SHARE_BODY_BYTES: usize = 4 * 1024;

/// Longest account id accepted in a body.
const MAX_ACCOUNT_ID_CHARS: usize = 256;

/// Why a share request was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShareError {
    BodyTooLarge,
    MalformedRequest,
    /// The body named the caller's own account.
    SelfShare,
}

impl ShareError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::BodyTooLarge => "payload-too-large",
            Self::MalformedRequest => "malformed-share-request",
            Self::SelfShare => "cannot-share-with-self",
        }
    }

    pub const fn http_status(self) -> &'static str {
        match self {
            Self::BodyTooLarge => "413 Payload Too Large",
            Self::MalformedRequest | Self::SelfShare => "400 Bad Request",
        }
    }
}

impl std::fmt::Display for ShareError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::BodyTooLarge => "body too large",
            Self::MalformedRequest => "malformed share request",
            Self::SelfShare => "an account already has access to its own document",
        })
    }
}

impl std::error::Error for ShareError {}

/// Whether `path` is one of the share routes.
pub(super) fn is_share_route(path: &str) -> bool {
    matches!(
        path,
        share_routes::GRANT | share_routes::REVOKE | share_routes::LIST
    )
}

/// Dispatch one `/api/share/*` request.
///
/// `lease` is the CALLER's own tenant: grant and revoke edit the caller's
/// access list, never the tenant a `?tenant=` parameter pointed at. A visitor
/// cannot re-share a document they were merely given access to.
pub(super) fn handle(
    method: &str,
    path: &str,
    body: &str,
    identity: &ResolvedIdentity,
    lease: &TenantLease,
    registry: &TenantRegistry,
) -> WebReply {
    match (method, path) {
        ("POST", share_routes::GRANT) => mutate(body, identity, lease, registry, true),
        ("POST", share_routes::REVOKE) => mutate(body, identity, lease, registry, false),
        ("GET", share_routes::LIST) => list(identity, lease, registry),
        _ => WebReply {
            status: "405 Method Not Allowed",
            body: crate::mcp_serve::rest_error_body("method not allowed for this share route"),
        },
    }
}

fn mutate(
    body: &str,
    identity: &ResolvedIdentity,
    lease: &TenantLease,
    registry: &TenantRegistry,
    granting: bool,
) -> WebReply {
    let account = match parse_account(body, &identity.user_id) {
        Ok(account) => account,
        Err(error) => return error_reply(error),
    };
    let change = if granting {
        AclChange::Grant(account)
    } else {
        AclChange::Revoke(account)
    };
    // The edit and its write are one serialised operation. Persisted
    // immediately rather than at eviction: a share the user was told had
    // succeeded must survive a restart, and the document it applies to may not
    // be written for another half hour.
    match registry.update_acl(lease.owner_id(), lease.tenant(), change) {
        Ok(update) => WebReply {
            status: "200 OK",
            body: serde_json::json!({
                "ok": true,
                "changed": update.changed,
                "sharedWith": update.shared_with.into_iter().collect::<Vec<_>>(),
            })
            .to_string(),
        },
        // A full access list is the caller's problem, not the server's: the
        // store writes a bounded list, so accepting the grant would report a
        // success that vanishes on the next save.
        Err(super::tenant_store::TenantStoreError::ShareLimitReached(limit)) => WebReply {
            status: "400 Bad Request",
            body: serde_json::json!({
                "ok": false,
                "error": "share-limit-reached",
                "limit": limit,
                "message": format!("this document is already shared with {limit} accounts"),
            })
            .to_string(),
        },
        // The change has been rolled back, so memory and disk agree and a
        // retry starts from a known state. Reporting 200 here — as the
        // previous code did — told the user a share had succeeded that would
        // vanish on the next restart.
        Err(error) => WebReply {
            status: "500 Internal Server Error",
            body: serde_json::json!({
                "ok": false,
                "error": "share-not-persisted",
                "message": error.to_string(),
            })
            .to_string(),
        },
    }
}

fn list(identity: &ResolvedIdentity, lease: &TenantLease, registry: &TenantRegistry) -> WebReply {
    WebReply {
        status: "200 OK",
        body: serde_json::json!({
            "ok": true,
            // Who may open this account's document…
            "sharedWith": lease.tenant().shared_with().into_iter().collect::<Vec<_>>(),
            // …and whose documents this account may open. Resident owners
            // only; see `TenantRegistry::shared_with_visitor`.
            "sharedWithMe": registry.shared_with_visitor(&identity.user_id),
        })
        .to_string(),
    }
}

/// Pull the target account out of a share body.
///
/// The account id is not validated against the hub: this deployment has no
/// user-lookup endpoint yet, so an id that belongs to nobody simply grants
/// access to nobody. M5 should resolve it through the hub so a typo is
/// reported at grant time instead of silently doing nothing.
fn parse_account(body: &str, caller: &str) -> Result<String, ShareError> {
    if body.len() > MAX_SHARE_BODY_BYTES {
        return Err(ShareError::BodyTooLarge);
    }
    let parsed: serde_json::Value =
        serde_json::from_str(body).map_err(|_| ShareError::MalformedRequest)?;
    let account = parsed
        .get("userId")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(ShareError::MalformedRequest)?;
    if account.chars().count() > MAX_ACCOUNT_ID_CHARS {
        return Err(ShareError::MalformedRequest);
    }
    if account == caller {
        return Err(ShareError::SelfShare);
    }
    Ok(account.to_string())
}

fn error_reply(error: ShareError) -> WebReply {
    WebReply {
        status: error.http_status(),
        body: serde_json::json!({
            "ok": false,
            "error": error.code(),
            "message": error.to_string(),
        })
        .to_string(),
    }
}

#[cfg(test)]
#[path = "share_routes_tests.rs"]
mod tests;
