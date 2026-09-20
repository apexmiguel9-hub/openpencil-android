//! Transport-neutral handler core for the pairing-code endpoints.
//!
//! Mirrors `RelayLocatorPublishService`: the HTTP wrapper does strict
//! method/path/content-type and status mapping; ticket validation and store
//! policy stay single-sourced here. The stored blob is opaque — the server
//! can neither read the invite nor mint a code id that a real code derives
//! to, so the store is a dumb bounded mailbox.

use std::time::Instant;

use op_auth_bridge::{CollabJwksFetcher, CollabTicketVerifier, CollabVerifierConfigError};

use crate::pairing_wire::{PairingClaimRequest, PairingPublishRequest, MAX_PAIRING_CODE_TTL_SECS};

/// Bounded storage the deployable server plugs in.
pub trait PairingCodeStore: Send + Sync {
    /// Store a blob until `expires_at_unix`, attributed to `owner` (the
    /// ticket-verified device key) for per-owner quotas. Re-publishing the
    /// identical `(owner, code_id, sealed)` triple is idempotent; any other
    /// collision with a live id is a refusal — ids come from 128 bits of a
    /// keyed hash, so a foreign collision is an attack, never coincidence.
    /// A live-or-tombstoned entry must never be silently replaced: the
    /// claim-budget tombstone is what stops a code holder from substituting
    /// the owner's sealed invite with their own.
    fn put(
        &self,
        owner: [u8; 32],
        code_id: [u8; 16],
        sealed: Vec<u8>,
        now_unix: u64,
        expires_at_unix: u64,
    ) -> Result<PairingPutOutcome, PairingStoreRejection>;

    /// Fetch a live blob. Implementations enforce their own claim budgets.
    fn claim(&self, code_id: &[u8; 16], now_unix: u64) -> Option<Vec<u8>>;
}

/// Successful put disposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingPutOutcome {
    Stored,
    /// The identical entry already exists — a lost-response retry, not an
    /// error. Mapped to the same success the first publish returned.
    AlreadyStored,
}

/// Storage refusal, split so the HTTP layer can map retryable pressure
/// (503) apart from caller mistakes (400).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingStoreRejection {
    /// A different live (or tombstoned) entry owns this id.
    DuplicateCode,
    /// This owner already holds its maximum number of live codes.
    OwnerQuotaExceeded,
    /// The store is full of live entries.
    CapacityExhausted,
    /// The blob itself is unacceptable (bounds, clock).
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RelayPairingServiceError {
    #[error("pairing request was rejected")]
    RequestRejected,
    #[error("pairing request authentication failed")]
    AuthenticationFailed,
    #[error("pairing code not found or expired")]
    NotFound,
    #[error("pairing service unavailable")]
    Unavailable,
}

pub struct RelayPairingService<F, S> {
    ticket_verifier: CollabTicketVerifier<F>,
    store: S,
}

impl<F, S> RelayPairingService<F, S>
where
    F: CollabJwksFetcher,
    S: PairingCodeStore,
{
    pub fn production(fetcher: F, store: S) -> Result<Self, CollabVerifierConfigError> {
        Ok(Self::new(CollabTicketVerifier::production(fetcher)?, store))
    }

    pub const fn new(ticket_verifier: CollabTicketVerifier<F>, store: S) -> Self {
        Self {
            ticket_verifier,
            store,
        }
    }

    pub fn publish_at(
        &self,
        request_body: &[u8],
        opaque_ticket: &[u8],
        now_unix: u64,
        cache_now: Instant,
    ) -> Result<(), RelayPairingServiceError> {
        let request = PairingPublishRequest::decode_binary(request_body)
            .map_err(|_| RelayPairingServiceError::RequestRejected)?;
        self.verify_ticket(opaque_ticket, request.device_static(), now_unix, cache_now)?;
        let ttl = u64::from(request.ttl_secs().min(MAX_PAIRING_CODE_TTL_SECS));
        let expires_at_unix = now_unix
            .checked_add(ttl)
            .ok_or(RelayPairingServiceError::RequestRejected)?;
        self.store
            .put(
                *request.device_static(),
                *request.code_id(),
                request.sealed().to_vec(),
                now_unix,
                expires_at_unix,
            )
            .map(|_| ())
            .map_err(|rejection| match rejection {
                PairingStoreRejection::DuplicateCode
                | PairingStoreRejection::OwnerQuotaExceeded
                | PairingStoreRejection::Invalid => RelayPairingServiceError::RequestRejected,
                PairingStoreRejection::CapacityExhausted => RelayPairingServiceError::Unavailable,
            })
    }

    pub fn claim_at(
        &self,
        request_body: &[u8],
        opaque_ticket: &[u8],
        now_unix: u64,
        cache_now: Instant,
    ) -> Result<Vec<u8>, RelayPairingServiceError> {
        let request = PairingClaimRequest::decode_binary(request_body)
            .map_err(|_| RelayPairingServiceError::RequestRejected)?;
        self.verify_ticket(opaque_ticket, request.device_static(), now_unix, cache_now)?;
        self.store
            .claim(request.code_id(), now_unix)
            .ok_or(RelayPairingServiceError::NotFound)
    }

    fn verify_ticket(
        &self,
        opaque_ticket: &[u8],
        device_static: &[u8; 32],
        now_unix: u64,
        cache_now: Instant,
    ) -> Result<(), RelayPairingServiceError> {
        self.ticket_verifier
            .verify_at(opaque_ticket, device_static, now_unix, cache_now)
            .map(|_| ())
            .map_err(|_| RelayPairingServiceError::AuthenticationFailed)
    }
}

impl<F, S> std::fmt::Debug for RelayPairingService<F, S> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RelayPairingService")
            .finish_non_exhaustive()
    }
}
