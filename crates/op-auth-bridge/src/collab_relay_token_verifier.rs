//! Relay-bearer verification: the minimized relay token, plus the bounded
//! dual-accept path that keeps pre-migration clients working.
//!
//! See [`crate::collab_relay_token`] for the credential contract and the
//! threat model this addresses (third-party/regional relay operator; **not**
//! first-party issuer↔relay collusion).

use std::time::Instant;

use crate::{
    collab_claims::COLLAB_JWS_TYPE,
    collab_relay_token::{
        validate_relay_token_claims, UnverifiedRelayTokenClaims, MAX_COLLAB_RELAY_TOKEN_BYTES,
        RELAY_TOKEN_JWS_TYPE,
    },
    collab_verifier::{
        decode_channel_binding, decode_segment, ensure_not_cancelled, parse_compact_jws,
    },
    CollabJwksFetcher, CollabTicketVerifier, CollabVerifyError, VerifiedRelayTokenClaims,
    MAX_COLLAB_JWS_CLAIMS_BYTES, MAX_COLLAB_TICKET_BYTES,
};

/// Which credential shape a relay bearer turned out to be.
///
/// Carried for migration telemetry and tests only. It deliberately carries no
/// payload, so observing the kind can never become a path to reading claims.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelayBearerKind {
    /// The claim-minimized relay token.
    MinimizedRelayToken,
    /// The legacy full collaboration ticket, accepted only during migration.
    FullCollabTicket,
}

/// Everything a relay is allowed to learn from an accepted bearer.
///
/// Both credential shapes collapse to this, so the relay's authorization code
/// cannot read identity claims even while it still dual-accepts full tickets.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VerifiedRelayBearer {
    kind: RelayBearerKind,
    expires_at_unix_seconds: u64,
}

impl VerifiedRelayBearer {
    pub const fn kind(&self) -> RelayBearerKind {
        self.kind
    }

    pub const fn expires_at_unix_seconds(&self) -> u64 {
        self.expires_at_unix_seconds
    }
}

/// Failures of the relay-bearer boundary.
///
/// Kept separate from [`CollabVerifyError`] so adding the dual-accept policy
/// does not widen the exhaustive classification the desktop ticket path does.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RelayBearerVerifyError {
    #[error("relay bearer credential type is not accepted by this relay")]
    UnacceptedCredentialType,
    #[error(transparent)]
    Verify(#[from] CollabVerifyError),
}

impl<F: CollabJwksFetcher> CollabTicketVerifier<F> {
    /// Verify a claim-minimized relay token against the pinned trust root.
    pub fn verify_relay_token_at(
        &self,
        token: &[u8],
        expected_dh_pub_x25519: &[u8; 32],
        now_unix_seconds: u64,
        cache_now: Instant,
    ) -> Result<VerifiedRelayTokenClaims, CollabVerifyError> {
        self.verify_relay_token_at_cancellable(
            token,
            expected_dh_pub_x25519,
            now_unix_seconds,
            cache_now,
            &|| false,
            false,
        )
    }

    pub fn verify_relay_token_at_cancellable(
        &self,
        token: &[u8],
        expected_dh_pub_x25519: &[u8; 32],
        now_unix_seconds: u64,
        cache_now: Instant,
        cancelled: &dyn Fn() -> bool,
        cancellation_enabled: bool,
    ) -> Result<VerifiedRelayTokenClaims, CollabVerifyError> {
        ensure_not_cancelled(cancelled)?;
        if token.is_empty() || token.len() > MAX_COLLAB_RELAY_TOKEN_BYTES {
            return Err(CollabVerifyError::InvalidTicketSize {
                maximum: MAX_COLLAB_RELAY_TOKEN_BYTES,
            });
        }
        if expected_dh_pub_x25519.iter().all(|byte| *byte == 0) {
            return Err(CollabVerifyError::InvalidChannelBinding);
        }
        let parsed = parse_compact_jws(token)?;
        if parsed.header.typ != RELAY_TOKEN_JWS_TYPE {
            return Err(CollabVerifyError::WrongType);
        }
        self.verify_relay_token_parsed(
            &parsed,
            expected_dh_pub_x25519,
            now_unix_seconds,
            cache_now,
            cancelled,
            cancellation_enabled,
        )
    }

    fn verify_relay_token_parsed(
        &self,
        parsed: &crate::collab_verifier::ParsedCompactJws<'_>,
        expected_dh_pub_x25519: &[u8; 32],
        now_unix_seconds: u64,
        cache_now: Instant,
        cancelled: &dyn Fn() -> bool,
        cancellation_enabled: bool,
    ) -> Result<VerifiedRelayTokenClaims, CollabVerifyError> {
        self.verify_parsed_signature(
            parsed,
            now_unix_seconds,
            cache_now,
            cancelled,
            cancellation_enabled,
        )?;
        let claims_bytes =
            decode_segment(parsed.claims_segment, "claims", MAX_COLLAB_JWS_CLAIMS_BYTES)?;
        let claims: UnverifiedRelayTokenClaims = serde_json::from_slice(&claims_bytes)
            .map_err(|_| CollabVerifyError::MalformedJson { part: "claims" })?;
        let decoded_dh_pub_x25519 = decode_channel_binding(&claims.dh_pub_x25519)?;
        ensure_not_cancelled(cancelled)?;
        let verified = validate_relay_token_claims(
            claims,
            self.config(),
            expected_dh_pub_x25519,
            decoded_dh_pub_x25519,
            now_unix_seconds,
        )?;
        ensure_not_cancelled(cancelled)?;
        Ok(verified)
    }

    /// Verify an `Authorization: Bearer` relay credential and return **only**
    /// the authenticated expiry.
    ///
    /// The credential shape is discriminated on the protected-header `typ`
    /// *before* any claim parsing, so a full ticket is never fed to the
    /// minimized parser and vice versa. When `accept_full_collab_ticket` is
    /// false, a legacy ticket is refused outright rather than verified and
    /// discarded — the relay must not learn identity claims it has been
    /// configured to stop accepting.
    ///
    /// Note for future maintainers: even in the dual-accept case the caller
    /// receives no identity, because a relay that could read `sub` and
    /// `device_id` off a legacy bearer would keep the exact social-graph
    /// exposure this credential exists to remove.
    pub fn verify_relay_bearer_at(
        &self,
        bearer: &[u8],
        expected_dh_pub_x25519: &[u8; 32],
        now_unix_seconds: u64,
        cache_now: Instant,
        accept_full_collab_ticket: bool,
    ) -> Result<VerifiedRelayBearer, RelayBearerVerifyError> {
        if bearer.is_empty() || bearer.len() > MAX_COLLAB_TICKET_BYTES {
            return Err(CollabVerifyError::InvalidTicketSize {
                maximum: MAX_COLLAB_TICKET_BYTES,
            }
            .into());
        }
        let parsed = parse_compact_jws(bearer)?;
        if parsed.header.typ == RELAY_TOKEN_JWS_TYPE {
            if bearer.len() > MAX_COLLAB_RELAY_TOKEN_BYTES {
                return Err(CollabVerifyError::InvalidTicketSize {
                    maximum: MAX_COLLAB_RELAY_TOKEN_BYTES,
                }
                .into());
            }
            if expected_dh_pub_x25519.iter().all(|byte| *byte == 0) {
                return Err(CollabVerifyError::InvalidChannelBinding.into());
            }
            let verified = self.verify_relay_token_parsed(
                &parsed,
                expected_dh_pub_x25519,
                now_unix_seconds,
                cache_now,
                &|| false,
                false,
            )?;
            return Ok(VerifiedRelayBearer {
                kind: RelayBearerKind::MinimizedRelayToken,
                expires_at_unix_seconds: verified.expires_at_unix_seconds(),
            });
        }
        if parsed.header.typ == COLLAB_JWS_TYPE && accept_full_collab_ticket {
            let verified =
                self.verify_at(bearer, expected_dh_pub_x25519, now_unix_seconds, cache_now)?;
            return Ok(VerifiedRelayBearer {
                kind: RelayBearerKind::FullCollabTicket,
                expires_at_unix_seconds: verified.expires_at_unix_seconds(),
            });
        }
        Err(RelayBearerVerifyError::UnacceptedCredentialType)
    }
}

#[cfg(test)]
#[path = "collab_relay_token_tests.rs"]
mod tests;
