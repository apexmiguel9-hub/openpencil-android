//! The account surface: regions, hub origins, hub URLs, session parsing.
//!
//! Signing in is **optional** and changes nothing about the local import
//! path. What it adds is an identity the popup can show and — once the Hub
//! grows an inbox API (see `docs/hub-inbox-api-proposal.md`) — a second
//! delivery target. See [`crate::delivery`].
//!
//! # The trust model this module encodes
//!
//! The extension is a **public client**. It holds no SSO client secret, no
//! authorization code, no refresh token, and no session token. The whole
//! flow is:
//!
//! 1. the popup opens `GET <hub>/api/v1/auth/login?return_to=/account` in a
//!    normal tab, and the user signs in on the Hub's own origin;
//! 2. the Hub sets its first-party `op_hub_session` cookie (`HttpOnly`,
//!    `Secure`, `SameSite=Lax`, `Path=/`) — a cookie the extension cannot
//!    read, and deliberately does not ask to;
//! 3. the popup reads `GET <hub>/api/v1/session` with `credentials:
//!    'include'` and renders the JSON it gets back.
//!
//! So the only credential material that exists anywhere is the cookie, and it
//! lives in the browser's cookie jar under the Hub's origin. The one value
//! that crosses into extension memory is the session's `csrf_token`, which is
//! required on the Hub's mutating endpoints (`POST /api/v1/auth/logout`);
//! [`parse_session`] therefore validates it hard enough to be safe as an HTTP
//! header value, and the popup keeps it in memory only — never in
//! `chrome.storage`.
//!
//! # Why the origins are compiled in rather than typed
//!
//! Chrome resolves `host_permissions` from the manifest at install time, so a
//! hub origin the user could type would not be reachable anyway without a
//! runtime `chrome.permissions.request`. Pinning the two published origins
//! here — and asserting in the tests that `manifest.json` grants exactly them
//! — makes the reachable set of hosts a compile-time fact instead of a
//! storage value an attacker-influenced code path could steer.

use crate::endpoint::normalize_endpoint;
use crate::js_text::{js_trim, truncate_utf16};

/// Consumer portal origin of the China region.
///
/// The public hostname is the one the Hub's own ICP filing check keys on
/// (`op-hub/frontend/src/components/domestic-filing-link.tsx`), and it is the
/// `op.` sibling of the SSO service's `sso.zseven.cn`.
pub const HUB_ORIGIN_CN: &str = "https://op.zseven.cn";

/// Consumer portal origin of the Global region — the `.tech` sibling, exactly
/// as `zseven-sso` pairs `sso.zseven.cn` with `sso.zseven.tech`.
pub const HUB_ORIGIN_GLOBAL: &str = "https://op.zseven.tech";

/// Session probe. Answers `200` with the session envelope, or `401`.
const SESSION_PATH: &str = "/api/v1/session";
/// Login entry point. Opened in a TAB, never fetched.
const LOGIN_PATH: &str = "/api/v1/auth/login";
/// Session teardown. `POST`, needs the CSRF header.
const LOGOUT_PATH: &str = "/api/v1/auth/logout";
/// The portal page a user lands on, and the only `return_to` we ask for.
const ACCOUNT_PATH: &str = "/account";

/// Longest display name rendered in the popup's 340 px header, in UTF-16
/// code units. A display name is user-chosen text from another user's
/// account; it must not be able to push the sign-out control off-screen.
const MAX_NAME_UNITS: usize = 48;

/// Longest `avatar_url` accepted. Chrome would not fetch a longer one
/// usefully, and the value is rendered into an `<img src>`.
const MAX_AVATAR_BYTES: usize = 2048;

/// Longest `csrf_token` accepted. The Hub mints 43-character base64url
/// tokens; the cap is generous so a rotation to a longer token does not
/// break the client, and the charset check below is what actually matters.
const MAX_CSRF_BYTES: usize = 256;

/// Longest failure text echoed into the popup's status line.
const MAX_DETAIL_UNITS: usize = 200;

/// Milliseconds before a hub request is abandoned.
///
/// Deliberately shorter than the snapshot import's 15 s: a session probe
/// carries no payload and runs while the user is looking at an empty account
/// row, so a hub that is slow or unreachable has to fall back to "signed out
/// / unavailable" quickly rather than holding the row blank. A capture, by
/// contrast, is worth waiting for.
pub const REQUEST_TIMEOUT_MS: u32 = 8_000;

/// Which regional deployment the account lives in.
///
/// Mirrors the product's own built-in dual-region design
/// (`CollabRelayRegion` in `op-editor-core`, `RelayRegion` on the collab
/// wire) down to the persisted spelling, so a user who picked "China" for
/// collaboration sees the same word here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Region {
    /// China. The product's default, matching `CollabRelayRegion::default()`.
    #[default]
    Cn,
    /// Everywhere else.
    Global,
}

impl Region {
    /// Persisted spelling, shared with the collaboration bootstrap document.
    pub fn as_str(self) -> &'static str {
        match self {
            Region::Cn => "cn",
            Region::Global => "global",
        }
    }

    /// Parse a stored value. Anything unrecognised — including a value left
    /// by a future version — falls back to the default rather than failing,
    /// because a popup that will not open is a worse outcome than a popup
    /// pointing at the default region.
    pub fn from_stored(raw: &str) -> Region {
        match js_trim(raw) {
            "global" => Region::Global,
            _ => Region::Cn,
        }
    }

    /// The region's published hub origin.
    pub fn hub_origin(self) -> &'static str {
        match self {
            Region::Cn => HUB_ORIGIN_CN,
            Region::Global => HUB_ORIGIN_GLOBAL,
        }
    }
}

/// The hub origin to talk to: the region's published one, unless a loopback
/// development origin has been stored.
///
/// The override exists so `OP_HUB_PUBLIC_URL=http://127.0.0.1:18081` (the
/// Hub's documented development invocation) can be driven from the same
/// popup. It is accepted **only** when it normalizes to a loopback
/// `host:port` — the exact rule [`crate::endpoint`] applies to the local
/// editor, and the reason no new host permission is needed for it: the
/// manifest already grants `http://127.0.0.1/*`.
///
/// Anything else — a public hostname, an `https://` URL, a path — is ignored
/// in favour of the region's origin. There is no code path in which a stored
/// value can point the session probe at an arbitrary host.
pub fn hub_origin(region: Region, override_raw: &str) -> String {
    if let Some(dev) = dev_origin(override_raw) {
        return dev;
    }
    region.hub_origin().to_owned()
}

/// A loopback development hub origin, or `None`.
fn dev_origin(raw: &str) -> Option<String> {
    let trimmed = js_trim(raw);
    if trimmed.is_empty() {
        return None;
    }
    // `https://127.0.0.1:8443` is not a development Hub — the Hub only
    // serves plaintext when `OP_HUB_PUBLIC_URL` is `http://`, and accepting
    // both would mean two spellings of one thing. `get` rather than a slice
    // index: byte 8 can land inside a multi-byte character.
    if trimmed
        .get(..8)
        .is_some_and(|head| head.eq_ignore_ascii_case("https://"))
    {
        return None;
    }
    normalize_endpoint(trimmed).map(|endpoint| format!("http://{endpoint}"))
}

/// The URL to OPEN IN A TAB to sign in.
///
/// `return_to` must be one of the Hub's exact allowlisted portal routes and
/// the request must carry that one query parameter and nothing else — both
/// are enforced server-side, so the string is built here rather than
/// assembled by the caller.
pub fn login_url(origin: &str) -> String {
    format!("{origin}{LOGIN_PATH}?return_to={ACCOUNT_PATH}")
}

/// The session probe URL.
pub fn session_url(origin: &str) -> String {
    format!("{origin}{SESSION_PATH}")
}

/// The logout URL.
pub fn logout_url(origin: &str) -> String {
    format!("{origin}{LOGOUT_PATH}")
}

/// The portal page, opened in a tab as the sign-out fallback.
pub fn account_url(origin: &str) -> String {
    format!("{origin}{ACCOUNT_PATH}")
}

/// The `host_permissions` match pattern a hub origin needs.
///
/// Kept here so the manifest and this module cannot drift apart — the test
/// module asserts the shipped `manifest.json` grants exactly these two.
pub fn host_permission(origin: &str) -> String {
    format!("{origin}/*")
}

/// What `GET /api/v1/session` said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionView {
    /// A usable session. Everything here is safe to render as *text*.
    SignedIn(Account),
    /// No session — the ordinary state, not an error.
    SignedOut,
    /// The Hub answered something we cannot act on.
    Unavailable { detail: String },
}

/// The renderable half of a Hub session.
///
/// Deliberately does NOT carry `primary_email` or `roles`. The popup shows
/// who you are, not what you may do, and an email address is the one field
/// in the envelope whose leak into a screenshot would matter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    /// Stable SSO subject. Used only to notice that the account changed.
    pub user_id: String,
    /// The name to render: `display_name` when it has one, else `username`.
    pub display_name: String,
    /// An `https://` avatar, or `None`. Never a `data:` or relative URL.
    pub avatar_url: Option<String>,
    /// Session CSRF token, required by the Hub's mutating endpoints.
    pub csrf_token: String,
}

/// Classify a reply from `GET /api/v1/session`.
///
/// The shape is pinned by the Hub's `auth_routes.go` session handler:
/// `{"user":{"id","username","display_name","avatar_url","primary_email",
/// "roles"},"csrf_token","capabilities"}`, where the three middle user fields
/// may be `null`.
///
/// A `401` is the signed-out answer, and the only status that is not a
/// problem. Everything else — a `503` from a Hub whose Redis is down, an
/// HTML error page from a captive portal, a body that will not parse — is
/// [`SessionView::Unavailable`], because acting on it as "signed out" would
/// silently drop a session the user still has.
pub fn parse_session(status: u16, text: &str) -> SessionView {
    if status == 401 {
        return SessionView::SignedOut;
    }
    if !(200..300).contains(&status) {
        return SessionView::Unavailable {
            detail: detail_text(status, text),
        };
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return SessionView::Unavailable {
            detail: detail_text(status, text),
        };
    };

    let user = &value["user"];
    let Some(user_id) = bounded_field(user["id"].as_str(), 128) else {
        return SessionView::Unavailable {
            detail: "malformed session".to_owned(),
        };
    };
    let Some(username) = bounded_field(user["username"].as_str(), 128) else {
        return SessionView::Unavailable {
            detail: "malformed session".to_owned(),
        };
    };
    let Some(csrf_token) = csrf_token(value["csrf_token"].as_str()) else {
        return SessionView::Unavailable {
            detail: "malformed session".to_owned(),
        };
    };

    // `display_name` is nullable and may be blank; `username` is the
    // documented fallback and is validated above.
    let display_name = user["display_name"]
        .as_str()
        .map(js_trim)
        .filter(|name| !name.is_empty() && !name.chars().any(char::is_control))
        .unwrap_or(&username);
    let display_name = truncate_utf16(display_name, MAX_NAME_UNITS).to_owned();

    SessionView::SignedIn(Account {
        user_id,
        display_name,
        avatar_url: avatar_url(user["avatar_url"].as_str()),
        csrf_token,
    })
}

/// Serialize a [`SessionView`] for the popup's glue.
///
/// * `{"state":"signedIn","userId":…,"displayName":…,"avatarUrl":…|null,"csrfToken":…}`
/// * `{"state":"signedOut"}`
/// * `{"state":"error","detail":…}`
pub fn session_to_json(view: &SessionView) -> String {
    let value = match view {
        SessionView::SignedIn(account) => serde_json::json!({
            "state": "signedIn",
            "userId": account.user_id,
            "displayName": account.display_name,
            "avatarUrl": account.avatar_url,
            "csrfToken": account.csrf_token,
        }),
        SessionView::SignedOut => serde_json::json!({ "state": "signedOut" }),
        SessionView::Unavailable { detail } => serde_json::json!({
            "state": "error",
            "detail": detail,
        }),
    };
    value.to_string()
}

/// A required string field: present, trimmed, non-empty, bounded, and free
/// of control characters (which would corrupt the popup's single-line
/// header as surely as they would corrupt a header value).
fn bounded_field(value: Option<&str>, max_bytes: usize) -> Option<String> {
    let trimmed = js_trim(value?);
    if trimmed.is_empty() || trimmed.len() > max_bytes || trimmed.chars().any(char::is_control) {
        return None;
    }
    Some(trimmed.to_owned())
}

/// The session CSRF token, validated as an HTTP **header value**.
///
/// It is sent as `X-CSRF-Token`. `fetch` would reject a header containing a
/// newline anyway, but rejecting it here means the popup never gets as far as
/// building a request out of a hostile reply, and the rule is testable
/// without a browser.
fn csrf_token(value: Option<&str>) -> Option<String> {
    let raw = value?;
    if raw.is_empty()
        || raw.len() > MAX_CSRF_BYTES
        // Visible ASCII only: no space, no tab, no CR/LF, nothing non-ASCII.
        || !raw.bytes().all(|b| (0x21..=0x7e).contains(&b))
    {
        return None;
    }
    Some(raw.to_owned())
}

/// An avatar URL safe to put in an `<img src>`, or `None`.
///
/// `https://` only. That single rule rules out every scheme that could
/// execute (`javascript:`), embed arbitrary bytes (`data:`), reach the
/// filesystem (`file:`), or leak the popup's requests onto a plaintext
/// connection (`http:`) — and a relative URL, which would otherwise resolve
/// against `chrome-extension://<id>/`.
///
/// Public because the popup re-runs it on the way OUT of `chrome.storage`:
/// the cached account row is writable by any other view of the extension, so
/// the value that reaches an `<img src>` is validated at the point of use,
/// not only at the point it was received.
pub fn avatar_url(value: Option<&str>) -> Option<String> {
    let raw = js_trim(value?);
    if raw.len() < 9
        || raw.len() > MAX_AVATAR_BYTES
        || !raw
            .get(..8)
            .is_some_and(|head| head.eq_ignore_ascii_case("https://"))
    {
        return None;
    }
    // A URL is transmitted as ASCII; anything outside visible ASCII means
    // the value was never a URL, and quotes/backslashes have no business in
    // one either.
    if !raw
        .bytes()
        .all(|b| (0x21..=0x7e).contains(&b) && b != b'"' && b != b'\'' && b != b'\\' && b != b'<')
    {
        return None;
    }
    Some(raw.to_owned())
}

/// A bounded, human-readable failure line for the popup.
fn detail_text(status: u16, text: &str) -> String {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
        // The Hub's error shape: `{"error":{"code","message"}}`.
        if let Some(message) = value["error"]["message"].as_str() {
            let capped = truncate_utf16(js_trim(message), MAX_DETAIL_UNITS);
            if !capped.is_empty() {
                return capped.to_owned();
            }
        }
    }
    format!("HTTP {status}")
}
