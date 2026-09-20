//! Failure modes of the hub identity calls.
//!
//! Deliberately coarse on the wire side: a browser or an MCP client learns
//! only whether it is authenticated, never why the hub said no or what the
//! hub replied. The distinction that DOES matter is kept — a definitive "not
//! authenticated" is cacheable and an upstream failure is not — because
//! caching an outage would extend it.

use std::fmt;

/// Why the daemon could not turn a credential into a hub account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HubAuthError {
    /// No hub base URL is configured, so this client cannot verify anything.
    NotConfigured,
    /// The introspection endpoint needs the shared internal secret and none
    /// is configured. Fails closed rather than calling without it.
    MissingInternalAuth,
    /// The credential is not a well-formed one (empty, oversized, or not
    /// ASCII). Rejected at the boundary without reaching the hub.
    InvalidCredential,
    /// The hub gave a definitive negative: no session, or an inactive token.
    /// The only variant worth caching negatively.
    Unauthenticated,
    /// The hub could not be reached, timed out, or answered 5xx. Fails
    /// closed and is NOT cached — see the module docs.
    Upstream,
    /// The hub answered 2xx with a body this daemon cannot trust.
    MalformedResponse,
}

impl HubAuthError {
    /// Whether a negative cache entry may be written for this outcome.
    ///
    /// Only a definitive verdict qualifies. Caching a transport failure or a
    /// 5xx would turn a momentary hub blip into 15 seconds of guaranteed
    /// rejection for every account that happened to retry inside the window.
    pub const fn is_cacheable(self) -> bool {
        matches!(self, Self::Unauthenticated)
    }

    /// Whether this outcome means the hub itself is in trouble, as opposed
    /// to the caller's credential being bad.
    pub const fn is_upstream_failure(self) -> bool {
        matches!(
            self,
            Self::Upstream
                | Self::MalformedResponse
                | Self::NotConfigured
                | Self::MissingInternalAuth
        )
    }
}

impl fmt::Display for HubAuthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NotConfigured => "no hub base URL is configured",
            Self::MissingInternalAuth => "no hub internal auth secret is configured",
            Self::InvalidCredential => "malformed credential",
            Self::Unauthenticated => "unauthorized",
            Self::Upstream => "the hub could not be reached",
            Self::MalformedResponse => "the hub returned an unusable response",
        })
    }
}

impl std::error::Error for HubAuthError {}
