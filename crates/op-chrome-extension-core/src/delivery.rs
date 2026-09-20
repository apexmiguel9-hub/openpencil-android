//! Where a capture goes: the local editor, or the signed-in account.
//!
//! This is a two-value enum and one resolution rule, and it is in Rust for
//! the same reason the endpoint whitelist is: the *destination of a page
//! capture* is the one decision in this extension that must not be reachable
//! by accident. A stored value from a future build, a session that expired
//! between the popup opening and the button being pressed, a target the
//! server side does not exist for yet — every one of those has to collapse to
//! "the loopback editor the user configured", and [`resolve`] is where that
//! collapse happens, once, with tests.
//!
//! # The account target is wired
//!
//! op-hub grew the snapshot inbox this flag was waiting on:
//! `POST /api/v1/snapshots` (`op-hub/backend/internal/httpapi/snapshot_routes.go`),
//! authenticated by the session cookie plus `X-CSRF-Token`, with the
//! extension's own origin explicitly admitted
//! (`auth.ExtensionCapableMutation`). [`ACCOUNT_AVAILABLE`] is therefore
//! `true`, the popup's delivery row offers the account as a selectable
//! destination, and [`resolve`] returns [`Target::Account`] — but still only
//! for a user who is signed in *right now*.
//!
//! The request, the size ceiling and the reply classification live in
//! [`crate::hub`] and [`crate::hub_reply`]; this module remains what it always
//! was, the single place that decides where a capture is allowed to go.

/// Whether delivery to the signed-in account is implemented end to end.
///
/// A client that offers the target against a hub that 404s it is worse than
/// one that says "coming soon", so this stays true only while BOTH regional
/// hubs answer the inbox route. A `404` from a hub that has the route
/// unconfigured is still handled — it surfaces as
/// [`crate::hub_reply::CreateFailure::Unavailable`] — but that is a fallback,
/// not the plan.
pub const ACCOUNT_AVAILABLE: bool = true;

/// A delivery destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Target {
    /// The OpenPencil listening on the configured loopback endpoint.
    #[default]
    Local,
    /// The signed-in user's OpenPencil account.
    Account,
}

impl Target {
    /// Persisted spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Target::Local => "local",
            Target::Account => "account",
        }
    }

    /// Parse a stored value; anything unrecognised is [`Target::Local`].
    pub fn from_stored(raw: &str) -> Target {
        match raw.trim() {
            "account" => Target::Account,
            _ => Target::Local,
        }
    }
}

/// The target actually used for the next capture.
///
/// `stored` is what the user last chose, `signed_in` whether a Hub session
/// was observed *in this popup session*. The account target survives only
/// when it is implemented and there is an account to deliver to; in every
/// other case the capture goes where it has always gone.
pub fn resolve(stored: &str, signed_in: bool) -> Target {
    match Target::from_stored(stored) {
        Target::Account if ACCOUNT_AVAILABLE && signed_in => Target::Account,
        _ => Target::Local,
    }
}

/// Whether the popup should render the delivery row at all.
///
/// Signed out there is exactly one destination, and a picker with one option
/// is noise. Signed in the row appears with both destinations selectable.
pub fn row_visible(signed_in: bool) -> bool {
    signed_in
}
