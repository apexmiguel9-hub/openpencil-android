//! Claim-minimized relay bearer credential.
//!
//! The public collaboration relay authenticates a WSS connection with an
//! `Authorization: Bearer` credential and needs exactly one fact out of it:
//! the signed expiry it clamps the session deadline to. Route authorization
//! is proven separately by the Ed25519-signed locator plus the 32-byte
//! `RouteCapability` in the ClientHello, and peer admission is proven inside
//! the Noise channel by the full collaboration ticket. So the relay bearer
//! carries no account subject, no device id, no ticket id, and no profile.
//!
//! # Threat model
//!
//! This defends against a **third-party or regional relay operator** who
//! would otherwise be able to reconstruct a genuine social graph — which
//! accounts collaborate with whom, from which devices, when — purely from
//! the credentials it is handed. It deliberately does **not** defend against
//! the first-party issuer colluding with the relay: the token is still a
//! conventional signed JWS minted per session by the same SSO that knows the
//! account, so an issuer↔relay join on timing or on the signing key remains
//! possible. Unlinkable-credential machinery (blind signatures, per-connection
//! minting, rotating device statics) is explicitly out of scope; do not read
//! this module as providing that guarantee.
//!
//! # Non-confusability
//!
//! The relay token and the collaboration ticket are domain-separated twice
//! over — a distinct protected-header `typ` and a distinct `aud`, both
//! compared strictly — and both claim structs carry `deny_unknown_fields`, so
//! each credential is *structurally* invalid against the other's parser even
//! before the strict comparisons run. That property is what makes reusing one
//! issuer signing key for both defensible, and it is covered by negative
//! tests in both directions.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{
    collab_claims::{COLLAB_TICKET_CLOCK_SKEW_SECONDS, COLLAB_TICKET_MAX_LIFETIME_SECONDS},
    CollabVerifierConfig, CollabVerifyError,
};

/// Required protected-header `typ` for the minimized relay token.
pub const RELAY_TOKEN_JWS_TYPE: &str = "openpencil-relay+jwt";
/// Required `aud` claim for the minimized relay token.
pub const RELAY_TOKEN_AUDIENCE: &str = "openpencil-relay";
/// Required authorization scope for the minimized relay token.
pub const RELAY_TOKEN_SCOPE: &str = "relay:connect";
/// Supported relay-token claims revision.
pub const RELAY_TOKEN_VERSION: u32 = 1;

/// Maximum opaque relay-token size accepted from a provider or a peer.
///
/// The minimized claim set is a few hundred bytes; the ceiling only has to
/// bound a hostile input. It is deliberately far below
/// [`crate::MAX_COLLAB_TICKET_BYTES`], which must still accommodate a signed
/// display name and avatar URL.
pub const MAX_COLLAB_RELAY_TOKEN_BYTES: usize = 4 * 1024;

/// Compile-time proof that the two credential type strings really differ.
const _: () = assert!(
    RELAY_TOKEN_JWS_TYPE.len() != crate::collab_claims::COLLAB_JWS_TYPE.len()
        || !const_str_eq(RELAY_TOKEN_JWS_TYPE, crate::collab_claims::COLLAB_JWS_TYPE)
);
const _: () = assert!(
    RELAY_TOKEN_AUDIENCE.len() != crate::collab_claims::COLLAB_TICKET_AUDIENCE.len()
        || !const_str_eq(
            RELAY_TOKEN_AUDIENCE,
            crate::collab_claims::COLLAB_TICKET_AUDIENCE
        )
);

const fn const_str_eq(left: &str, right: &str) -> bool {
    let (left, right) = (left.as_bytes(), right.as_bytes());
    if left.len() != right.len() {
        return false;
    }
    let mut index = 0;
    while index < left.len() {
        if left[index] != right[index] {
            return false;
        }
        index += 1;
    }
    true
}

/// The frozen minimized claim set.
///
/// `deny_unknown_fields` is load-bearing: it makes a full collaboration
/// ticket — which additionally carries `sub`, `device_id`, `jti`, and the
/// optional profile claims — structurally invalid here. Do not add a route,
/// role, or session claim. Route authorization already comes from the signed
/// locator plus the capability secret; putting route ids in a token minted by
/// the SSO would move the collaboration graph to the party that also knows
/// the account, which is exactly what this credential exists to avoid.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UnverifiedRelayTokenClaims {
    pub iss: String,
    pub aud: String,
    pub ver: u32,
    pub dh_pub_x25519: String,
    pub scope: String,
    pub iat: u64,
    pub nbf: u64,
    pub exp: u64,
}

/// Relay-token output produced only after signature, claim, and
/// channel-binding checks.
///
/// This type intentionally exposes **no identity accessor at all** — not the
/// subject, device, ticket id, or profile, because the token does not carry
/// them, and not the issuer or the channel-binding key either, so a future
/// relay change cannot quietly start reading a stable per-caller value off
/// the authenticated credential. The only thing a relay may learn from the
/// bearer is when the authenticated session must end.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct VerifiedRelayTokenClaims {
    expires_at_unix_seconds: u64,
    expires_at_unix_ms: u64,
}

impl VerifiedRelayTokenClaims {
    pub fn expires_at_unix_seconds(&self) -> u64 {
        self.expires_at_unix_seconds
    }

    pub fn expires_at_unix_ms(&self) -> u64 {
        self.expires_at_unix_ms
    }
}

impl fmt::Debug for VerifiedRelayTokenClaims {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedRelayTokenClaims")
            .field("expires_at_unix_seconds", &self.expires_at_unix_seconds)
            .finish()
    }
}

/// Validate the minimized claim set under the same trust root, clock skew,
/// and maximum-lifetime policy as the full collaboration ticket.
pub(crate) fn validate_relay_token_claims(
    claims: UnverifiedRelayTokenClaims,
    config: &CollabVerifierConfig,
    expected_dh_pub_x25519: &[u8; 32],
    decoded_dh_pub_x25519: [u8; 32],
    now_unix_seconds: u64,
) -> Result<VerifiedRelayTokenClaims, CollabVerifyError> {
    if claims.iss != config.issuer() {
        return Err(CollabVerifyError::InvalidIssuer);
    }
    if claims.aud != RELAY_TOKEN_AUDIENCE {
        return Err(CollabVerifyError::InvalidAudience);
    }
    if claims.ver != RELAY_TOKEN_VERSION {
        return Err(CollabVerifyError::InvalidVersion);
    }
    if claims.scope != RELAY_TOKEN_SCOPE {
        return Err(CollabVerifyError::InvalidScope);
    }
    if decoded_dh_pub_x25519 != *expected_dh_pub_x25519 {
        return Err(CollabVerifyError::ChannelBindingMismatch);
    }
    if claims.iat > claims.nbf || claims.nbf >= claims.exp {
        return Err(CollabVerifyError::InvalidTimestamps);
    }
    let latest_accepted_future = now_unix_seconds.saturating_add(COLLAB_TICKET_CLOCK_SKEW_SECONDS);
    if claims.iat > latest_accepted_future {
        return Err(CollabVerifyError::InvalidTimestamps);
    }
    if claims.nbf > latest_accepted_future {
        return Err(CollabVerifyError::NotYetValid);
    }
    let earliest_accepted_expiry =
        now_unix_seconds.saturating_sub(COLLAB_TICKET_CLOCK_SKEW_SECONDS);
    if claims.exp <= earliest_accepted_expiry {
        return Err(CollabVerifyError::Expired);
    }
    let lifetime = claims
        .exp
        .checked_sub(claims.iat)
        .ok_or(CollabVerifyError::InvalidTimestamps)?;
    if lifetime > COLLAB_TICKET_MAX_LIFETIME_SECONDS {
        return Err(CollabVerifyError::LifetimeTooLong);
    }
    let expires_at_unix_ms = claims
        .exp
        .checked_mul(1_000)
        .ok_or(CollabVerifyError::ExpiryOverflow)?;
    Ok(VerifiedRelayTokenClaims {
        expires_at_unix_seconds: claims.exp,
        expires_at_unix_ms,
    })
}
