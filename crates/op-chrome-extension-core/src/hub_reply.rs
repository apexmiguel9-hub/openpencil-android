//! What the Hub said about a create, and what the popup should do about it.
//!
//! Every status the inbox route can answer with is enumerated here, because
//! each one wants different advice from the user:
//!
//! | Status | Hub source | Outcome |
//! | --- | --- | --- |
//! | `201` | `snapshot_routes.go:180-186` | [`CreateReply::Created`] |
//! | `400 invalid_request` | `snapshot_routes.go:166-172` | [`CreateFailure::Rejected`] |
//! | `401 authentication_required` | `auth/middleware.go:35-39` | [`CreateFailure::SignedOut`] |
//! | `403 forbidden` | `snapshot_routes.go:294-302` (Origin or CSRF) | [`CreateFailure::Forbidden`] |
//! | `409 quota_exceeded` | `snapshot_routes.go:464-473` | [`CreateFailure::Quota`] |
//! | `413 request_too_large` | `snapshot_routes.go:159-162`, `security.go:41-47` | [`CreateFailure::TooLarge`] |
//! | `429 rate_limited` + `Retry-After` | `snapshot_routes.go:139-147` | [`CreateFailure::RateLimited`] |
//! | `503 service_unavailable` (+ `Retry-After: 1`) | `snapshot_routes.go:148-153,494-502` | [`CreateFailure::Unavailable`] |
//! | `404` / `405` | route absent — a Hub with no inbox configured | [`CreateFailure::Unavailable`] |
//!
//! # Two decisions worth stating
//!
//! **A `409` does not say which ceiling was hit.** The Hub answers both the
//! item quota and the byte quota with the same `quota_exceeded` code and
//! differs only in the English `message`
//! (`snapshot_routes.go:464-473`). Sniffing that prose would break the day it
//! is reworded, so this module reports one quota outcome and lets the popup
//! name both ceilings. The server's own sentence still travels as the detail.
//!
//! **A `2xx` that carries no `id` is a failure.** The success body is pinned
//! (`{id,name,created_at,bytes,expires_at}`), so a `200` with an HTML page in
//! it is a proxy or a captive portal, not a filed snapshot. Reporting success
//! there would tell the user their capture is safe when it is nowhere; the
//! opposite error costs them a retry and a duplicate they can delete.

use serde_json::Value;

use crate::js_text::{js_trim, truncate_utf16};

/// Longest server-chosen text echoed into the popup's status line, in UTF-16
/// code units. Same reasoning as [`crate::ingress`]: this lands in a 340 px
/// popup and must not push the buttons off-screen.
const MAX_DETAIL_UNITS: usize = 200;

/// Longest snapshot name rendered back to the user. The Hub bounds it at 200
/// runes and strips control characters, but the value is re-checked here
/// because it is being rendered, and a reply is not a place to start trusting.
const MAX_NAME_UNITS: usize = 200;

/// Largest `Retry-After` honoured, in seconds. A day is already far past the
/// point where "try again later" is the only useful thing to say.
const MAX_RETRY_AFTER_SECONDS: u32 = 86_400;

/// What `POST /api/v1/snapshots` said.
#[derive(Debug, Clone, PartialEq)]
pub enum CreateReply {
    /// Filed. Everything here is safe to render as *text*.
    Created {
        id: String,
        name: String,
        bytes: f64,
        expires_at: String,
    },
    /// Refused. `code` is the popup's message selector.
    Failed {
        code: CreateFailure,
        detail: String,
        /// Seconds to wait, when the Hub said so.
        retry_after_seconds: Option<u32>,
    },
}

/// Failure kinds the popup renders differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateFailure {
    /// The session is gone. The popup clears the account row and asks for a
    /// fresh sign-in.
    SignedOut,
    /// Origin or CSRF check. A distinct message, because "sign in again" is
    /// the fix for one of those and not the other.
    Forbidden,
    /// The inbox is full — items or bytes, the Hub does not distinguish.
    Quota,
    /// Over the 32 MiB request cap.
    TooLarge,
    /// Too many uploads this hour.
    RateLimited,
    /// The Hub accepted the request and refused the snapshot.
    Rejected,
    /// No inbox on this Hub, or the Hub cannot serve it right now.
    Unavailable,
}

impl CreateFailure {
    /// Wire name shared with the popup's error switch.
    pub fn as_str(self) -> &'static str {
        match self {
            CreateFailure::SignedOut => "signedOut",
            CreateFailure::Forbidden => "forbidden",
            CreateFailure::Quota => "quota",
            CreateFailure::TooLarge => "tooLarge",
            CreateFailure::RateLimited => "rateLimited",
            CreateFailure::Rejected => "rejected",
            CreateFailure::Unavailable => "unavailable",
        }
    }
}

/// Classify a reply from `POST /api/v1/snapshots`.
///
/// `retry_after` is the raw `Retry-After` header, or the empty string.
pub fn classify_create_reply(status: u16, text: &str, retry_after: &str) -> CreateReply {
    let json = parse_reply(text);
    if (200..300).contains(&status) {
        return created(json.as_ref()).unwrap_or_else(|| CreateReply::Failed {
            code: CreateFailure::Unavailable,
            detail: detail_text(status, text, json.as_ref()),
            retry_after_seconds: None,
        });
    }
    let code = match status {
        401 => CreateFailure::SignedOut,
        403 => CreateFailure::Forbidden,
        // A 409 that is not `quota_exceeded` is some other conflict this
        // client does not know about; it is a refusal of the snapshot, not a
        // full inbox, and must not be reported as one.
        409 if error_code(json.as_ref()) == "quota_exceeded" => CreateFailure::Quota,
        400 | 409 => CreateFailure::Rejected,
        413 => CreateFailure::TooLarge,
        429 => CreateFailure::RateLimited,
        // 404 / 405: this Hub registers no inbox (`routes.snapshots == nil`),
        // so the request fell through to the static site. Same advice as a
        // 5xx — it is not something the user did.
        _ => CreateFailure::Unavailable,
    };
    CreateReply::Failed {
        code,
        detail: detail_text(status, text, json.as_ref()),
        retry_after_seconds: retry_after_seconds(retry_after),
    }
}

/// Read the created-snapshot body, or `None` when this is not one.
fn created(json: Option<&Value>) -> Option<CreateReply> {
    let value = json?;
    let id = bounded(value["id"].as_str(), 64)?;
    Some(CreateReply::Created {
        // A missing name is not a reason to disbelieve the reply — it is one
        // field of a confirmation the user has already earned.
        name: bounded(value["name"].as_str(), MAX_NAME_UNITS).unwrap_or_default(),
        id,
        bytes: value["bytes"]
            .as_f64()
            .filter(|n| n.is_finite())
            .unwrap_or(0.0),
        expires_at: bounded(value["expires_at"].as_str(), 64).unwrap_or_default(),
    })
}

/// The Hub's `{"error":{"code":…}}` selector, or the empty string.
fn error_code(json: Option<&Value>) -> &str {
    json.and_then(|value| value["error"]["code"].as_str())
        .unwrap_or_default()
}

/// A bounded, control-free string field, or `None` when it is absent or
/// unusable as display text.
fn bounded(value: Option<&str>, max_units: usize) -> Option<String> {
    let trimmed = js_trim(value?);
    if trimmed.is_empty() || trimmed.chars().any(char::is_control) {
        return None;
    }
    Some(truncate_utf16(trimmed, max_units).to_owned())
}

/// A human-readable failure line: the Hub's own `message` when there is one,
/// else the status.
fn detail_text(status: u16, text: &str, json: Option<&Value>) -> String {
    if let Some(value) = json {
        if let Some(message) = value["error"]["message"].as_str() {
            let capped = truncate_utf16(js_trim(message), MAX_DETAIL_UNITS);
            if !capped.is_empty() {
                return capped.to_owned();
            }
        }
    }
    let head = truncate_utf16(js_trim(text), MAX_DETAIL_UNITS);
    if head.is_empty() {
        format!("HTTP {status}")
    } else {
        head.to_owned()
    }
}

/// Parse `Retry-After`.
///
/// The Hub only ever emits delta-seconds (`retryAfterSeconds`, an integer ≥ 1).
/// The HTTP-date form is legal in the header's grammar and is deliberately not
/// supported: turning it into a wait needs the current time, which this crate
/// does not have, and a wrong "try again in N minutes" is worse than none.
fn retry_after_seconds(raw: &str) -> Option<u32> {
    let trimmed = js_trim(raw);
    if trimmed.is_empty() || !trimmed.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let seconds: u32 = trimmed.parse().ok()?;
    Some(seconds.clamp(1, MAX_RETRY_AFTER_SECONDS))
}

/// Parse a reply body, tolerating an empty or non-JSON one.
fn parse_reply(text: &str) -> Option<Value> {
    if text.is_empty() {
        return None;
    }
    serde_json::from_str(text).ok()
}
