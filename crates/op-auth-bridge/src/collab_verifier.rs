//! Compact-JWS parser and typed collaboration-ticket verifier.

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use ed25519_dalek::{Signature, VerifyingKey};

use crate::{
    collab_claims::{
        valid_key_id, validate_claims, CollabJwsHeader, CollabKeySource, UnverifiedCollabClaims,
        COLLAB_JWS_ALGORITHM, COLLAB_JWS_TYPE,
    },
    CollabJwksCache, CollabJwksCacheLimits, CollabJwksError, CollabJwksFetchError,
    CollabJwksFetcher, CollabVerifierConfig, CollabVerifierConfigError, CollabVerifyError,
    VerifiedCollabClaims, MAX_COLLAB_TICKET_BYTES,
};

pub const MAX_COLLAB_JWS_HEADER_BYTES: usize = 1_024;
pub const MAX_COLLAB_JWS_CLAIMS_BYTES: usize = 8 * 1_024;
const MAX_COLLAB_JWS_SIGNATURE_BYTES: usize = 64;

/// Signature, claims, timing, and channel-binding verifier.
pub struct CollabTicketVerifier<F> {
    config: CollabVerifierConfig,
    cache: CollabJwksCache<F>,
}

impl<F: CollabJwksFetcher> CollabTicketVerifier<F> {
    pub fn production(fetcher: F) -> Result<Self, CollabVerifierConfigError> {
        Self::new(
            CollabVerifierConfig::production(),
            fetcher,
            CollabJwksCacheLimits::default(),
        )
    }

    pub fn new(
        config: CollabVerifierConfig,
        fetcher: F,
        cache_limits: CollabJwksCacheLimits,
    ) -> Result<Self, CollabVerifierConfigError> {
        let cache = match config.key_source() {
            CollabKeySource::SignedPolicy => CollabJwksCache::new_signed_policy(
                config.keyset_endpoint(),
                config.issuer(),
                fetcher,
                cache_limits,
            )?,
            CollabKeySource::LegacyJwks => {
                CollabJwksCache::new(config.keyset_endpoint(), fetcher, cache_limits)?
            }
        };
        Ok(Self { config, cache })
    }

    pub fn config(&self) -> &CollabVerifierConfig {
        &self.config
    }

    /// Verify with explicit wall and monotonic clocks for deterministic hosts.
    pub fn verify_at(
        &self,
        ticket: &[u8],
        expected_dh_pub_x25519: &[u8; 32],
        now_unix_seconds: u64,
        cache_now: Instant,
    ) -> Result<VerifiedCollabClaims, CollabVerifyError> {
        self.verify_at_inner(
            ticket,
            expected_dh_pub_x25519,
            now_unix_seconds,
            cache_now,
            &never_cancelled,
            false,
        )
    }

    /// Verify with cancellation propagated through any in-flight fetch.
    pub fn verify_at_cancellable(
        &self,
        ticket: &[u8],
        expected_dh_pub_x25519: &[u8; 32],
        now_unix_seconds: u64,
        cache_now: Instant,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<VerifiedCollabClaims, CollabVerifyError> {
        self.verify_at_inner(
            ticket,
            expected_dh_pub_x25519,
            now_unix_seconds,
            cache_now,
            cancelled,
            true,
        )
    }

    fn verify_at_inner(
        &self,
        ticket: &[u8],
        expected_dh_pub_x25519: &[u8; 32],
        now_unix_seconds: u64,
        cache_now: Instant,
        cancelled: &dyn Fn() -> bool,
        cancellation_enabled: bool,
    ) -> Result<VerifiedCollabClaims, CollabVerifyError> {
        ensure_not_cancelled(cancelled)?;
        if ticket.is_empty() || ticket.len() > MAX_COLLAB_TICKET_BYTES {
            return Err(CollabVerifyError::InvalidTicketSize {
                maximum: MAX_COLLAB_TICKET_BYTES,
            });
        }
        if expected_dh_pub_x25519.iter().all(|byte| *byte == 0) {
            return Err(CollabVerifyError::InvalidChannelBinding);
        }

        let parsed = parse_compact_jws(ticket)?;
        if parsed.header.typ != COLLAB_JWS_TYPE {
            return Err(CollabVerifyError::WrongType);
        }
        self.verify_parsed_signature(
            &parsed,
            now_unix_seconds,
            cache_now,
            cancelled,
            cancellation_enabled,
        )?;

        let claims_bytes =
            decode_segment(parsed.claims_segment, "claims", MAX_COLLAB_JWS_CLAIMS_BYTES)?;
        let claims: UnverifiedCollabClaims = serde_json::from_slice(&claims_bytes)
            .map_err(|_| CollabVerifyError::MalformedJson { part: "claims" })?;
        let decoded_dh_pub_x25519 = decode_channel_binding(&claims.dh_pub_x25519)?;
        ensure_not_cancelled(cancelled)?;
        let verified = validate_claims(
            claims,
            &self.config,
            expected_dh_pub_x25519,
            decoded_dh_pub_x25519,
            now_unix_seconds,
        )?;
        ensure_not_cancelled(cancelled)?;
        Ok(verified)
    }

    /// Resolve the pinned verification key for the parsed `kid` and check the
    /// detached Ed25519 signature over the compact signing input.
    ///
    /// Shared by every credential shape this crate verifies, so the key
    /// authority, cancellation points, and strict-signature policy cannot
    /// diverge between the collaboration ticket and the relay token.
    pub(crate) fn verify_parsed_signature(
        &self,
        parsed: &ParsedCompactJws<'_>,
        now_unix_seconds: u64,
        cache_now: Instant,
        cancelled: &dyn Fn() -> bool,
        cancellation_enabled: bool,
    ) -> Result<(), CollabVerifyError> {
        ensure_not_cancelled(cancelled)?;
        let key_bytes = match (self.config.key_source(), cancellation_enabled) {
            (CollabKeySource::SignedPolicy, true) => {
                self.cache.policy_verification_key_cancellable(
                    &parsed.header.kid,
                    cache_now,
                    now_unix_seconds,
                    cancelled,
                )
            }
            (CollabKeySource::SignedPolicy, false) => {
                self.cache
                    .policy_verification_key(&parsed.header.kid, cache_now, now_unix_seconds)
            }
            (CollabKeySource::LegacyJwks, true) => {
                self.cache
                    .verification_key_cancellable(&parsed.header.kid, cache_now, cancelled)
            }
            (CollabKeySource::LegacyJwks, false) => {
                self.cache.verification_key(&parsed.header.kid, cache_now)
            }
        }
        .map_err(map_jwks_error)?;
        ensure_not_cancelled(cancelled)?;
        let verifying_key = VerifyingKey::from_bytes(&key_bytes)
            .map_err(|_| CollabVerifyError::InvalidSignature)?;
        let signature = Signature::from_bytes(&parsed.signature);
        verifying_key
            .verify_strict(parsed.signing_input, &signature)
            .map_err(|_| CollabVerifyError::InvalidSignature)?;
        ensure_not_cancelled(cancelled)
    }

    /// Verify using the process wall and monotonic clocks.
    pub fn verify(
        &self,
        ticket: &[u8],
        expected_dh_pub_x25519: &[u8; 32],
    ) -> Result<VerifiedCollabClaims, CollabVerifyError> {
        let now_unix_seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| CollabVerifyError::InvalidTimestamps)?
            .as_secs();
        self.verify_at(
            ticket,
            expected_dh_pub_x25519,
            now_unix_seconds,
            Instant::now(),
        )
    }

    pub fn cached_key_count(&self) -> Result<usize, crate::CollabJwksError> {
        self.cache.cached_key_count()
    }
}

fn never_cancelled() -> bool {
    false
}

pub(crate) fn ensure_not_cancelled(cancelled: &dyn Fn() -> bool) -> Result<(), CollabVerifyError> {
    if cancelled() {
        Err(CollabVerifyError::Cancelled)
    } else {
        Ok(())
    }
}

fn map_jwks_error(error: CollabJwksError) -> CollabVerifyError {
    if matches!(
        error,
        CollabJwksError::Fetch(CollabJwksFetchError::Cancelled)
    ) {
        CollabVerifyError::Cancelled
    } else {
        CollabVerifyError::Jwks(error)
    }
}

impl<F> std::fmt::Debug for CollabTicketVerifier<F> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CollabTicketVerifier")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

pub(crate) struct ParsedCompactJws<'a> {
    pub(crate) header: CollabJwsHeader,
    pub(crate) claims_segment: &'a str,
    pub(crate) signature: [u8; 64],
    pub(crate) signing_input: &'a [u8],
}

/// Parse a compact JWS and enforce the shared protected-header profile.
///
/// The explicit `typ` is deliberately *not* checked here: every credential
/// shape has its own required type and callers must compare it strictly.
/// Returning the parsed header lets the relay discriminate credential kinds
/// before any claim parsing runs.
pub(crate) fn parse_compact_jws(ticket: &[u8]) -> Result<ParsedCompactJws<'_>, CollabVerifyError> {
    let text = std::str::from_utf8(ticket).map_err(|_| CollabVerifyError::MalformedCompactJws)?;
    if !text.is_ascii() {
        return Err(CollabVerifyError::MalformedCompactJws);
    }
    let mut segments = text.split('.');
    let header_segment = segments
        .next()
        .filter(|value| !value.is_empty())
        .ok_or(CollabVerifyError::MalformedCompactJws)?;
    let claims_segment = segments
        .next()
        .filter(|value| !value.is_empty())
        .ok_or(CollabVerifyError::MalformedCompactJws)?;
    let signature_segment = segments
        .next()
        .filter(|value| !value.is_empty())
        .ok_or(CollabVerifyError::MalformedCompactJws)?;
    if segments.next().is_some() {
        return Err(CollabVerifyError::MalformedCompactJws);
    }

    let signing_input_length = header_segment
        .len()
        .checked_add(1)
        .and_then(|length| length.checked_add(claims_segment.len()))
        .ok_or(CollabVerifyError::MalformedCompactJws)?;
    let signing_input = ticket
        .get(..signing_input_length)
        .ok_or(CollabVerifyError::MalformedCompactJws)?;
    let header_bytes = decode_segment(header_segment, "header", MAX_COLLAB_JWS_HEADER_BYTES)?;
    let header: CollabJwsHeader = serde_json::from_slice(&header_bytes)
        .map_err(|_| CollabVerifyError::MalformedJson { part: "header" })?;
    if header.alg != COLLAB_JWS_ALGORITHM {
        return Err(CollabVerifyError::WrongAlgorithm);
    }
    if !valid_key_id(&header.kid) {
        return Err(CollabVerifyError::InvalidKeyId);
    }
    let signature_bytes = decode_segment(
        signature_segment,
        "signature",
        MAX_COLLAB_JWS_SIGNATURE_BYTES,
    )?;
    let signature = signature_bytes
        .try_into()
        .map_err(|_| CollabVerifyError::InvalidSignature)?;
    Ok(ParsedCompactJws {
        header,
        claims_segment,
        signature,
        signing_input,
    })
}

pub(crate) fn decode_segment(
    segment: &str,
    part: &'static str,
    maximum_decoded_bytes: usize,
) -> Result<Vec<u8>, CollabVerifyError> {
    let full_groups = maximum_decoded_bytes
        .checked_div(3)
        .and_then(|value| value.checked_mul(4))
        .ok_or(CollabVerifyError::SegmentTooLarge {
            part,
            maximum: maximum_decoded_bytes,
        })?;
    let tail = match maximum_decoded_bytes % 3 {
        0 => 0,
        1 => 2,
        _ => 3,
    };
    let maximum_encoded_bytes =
        full_groups
            .checked_add(tail)
            .ok_or(CollabVerifyError::SegmentTooLarge {
                part,
                maximum: maximum_decoded_bytes,
            })?;
    if segment.len() > maximum_encoded_bytes {
        return Err(CollabVerifyError::SegmentTooLarge {
            part,
            maximum: maximum_decoded_bytes,
        });
    }
    if segment.contains('=') {
        return Err(CollabVerifyError::InvalidBase64 { part });
    }
    let decoded = URL_SAFE_NO_PAD
        .decode(segment)
        .map_err(|_| CollabVerifyError::InvalidBase64 { part })?;
    if decoded.len() > maximum_decoded_bytes || URL_SAFE_NO_PAD.encode(&decoded) != segment {
        return Err(CollabVerifyError::InvalidBase64 { part });
    }
    Ok(decoded)
}

pub(crate) fn decode_channel_binding(value: &str) -> Result<[u8; 32], CollabVerifyError> {
    if value.len() > 64 || value.contains('=') {
        return Err(CollabVerifyError::InvalidChannelBinding);
    }
    let decoded = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| CollabVerifyError::InvalidChannelBinding)?;
    if URL_SAFE_NO_PAD.encode(&decoded) != value {
        return Err(CollabVerifyError::InvalidChannelBinding);
    }
    let binding: [u8; 32] = decoded
        .try_into()
        .map_err(|_| CollabVerifyError::InvalidChannelBinding)?;
    if binding.iter().all(|byte| *byte == 0) {
        return Err(CollabVerifyError::InvalidChannelBinding);
    }
    Ok(binding)
}

#[cfg(test)]
#[path = "collab_verifier_tests.rs"]
mod tests;
