//! Production collaboration authentication adapters.
//!
//! The private bridge only returns an opaque ticket. Identity is derived here
//! from the open verifier and pinned signed-policy authority; account display
//! state and peer payloads never become an authentication fallback.

mod relay;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use op_auth_bridge::{
    CollabJwksCacheLimits, CollabJwksError, CollabJwksFetchError, CollabTicketPoll,
    CollabTicketProvider, CollabTicketRequest, CollabTicketRequestId, CollabTicketVerifier,
    CollabVerifierConfig, CollabVerifyError, OpaqueCollabRelayToken, OpaqueCollabTicket,
};
use op_collab::{OpaqueTicket, VerifiedAuthMetadata};
use op_collab_transport::{
    AdmissionError, AdmissionHello, JoinIntent, TicketVerifier, VerifiedTicketClaims,
};

use crate::jwks::NativeCollabJwksFetcher;

use super::network::TERMINAL_FLUSH_TIMEOUT;
use super::types::{CollabRuntimeError, CollabRuntimeFailure};

const TICKET_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const TICKET_POLL_INTERVAL: Duration = Duration::from_millis(10);
const INITIAL_RENEWAL_RETRY_BACKOFF: Duration = Duration::from_secs(1);
const MAX_RENEWAL_RETRY_BACKOFF: Duration = Duration::from_secs(30);

struct PendingRenewal {
    receiver: Option<Receiver<Result<LocalAdmission, CollabRuntimeError>>>,
    cancelled: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Drop for PendingRenewal {
    fn drop(&mut self) {
        // Retire the publication channel before waking the worker so even a
        // result racing with cancellation cannot re-enter the active session.
        self.receiver.take();
        self.cancelled.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub(super) struct ProductionTicketVerifier {
    inner: CollabTicketVerifier<NativeCollabJwksFetcher>,
}

impl ProductionTicketVerifier {
    pub(super) fn new() -> Result<Self, CollabRuntimeError> {
        let config = op_auth_bridge::desktop_collab_verifier_config().map_err(|_| {
            CollabRuntimeError::new(CollabRuntimeFailure::AuthenticationUnavailable)
        })?;
        Self::new_with_config(config)
    }

    fn new_with_config(config: CollabVerifierConfig) -> Result<Self, CollabRuntimeError> {
        let fetcher = NativeCollabJwksFetcher::new().map_err(|_| {
            CollabRuntimeError::new(CollabRuntimeFailure::AuthenticationUnavailable)
        })?;
        let inner = CollabTicketVerifier::new(config, fetcher, CollabJwksCacheLimits::default())
            .map_err(|_| {
                CollabRuntimeError::new(CollabRuntimeFailure::AuthenticationUnavailable)
            })?;
        Ok(Self { inner })
    }

    fn expected_issuer(&self) -> &str {
        self.inner.config().issuer()
    }

    fn verify_cancellable(
        &self,
        opaque_ticket: &[u8],
        expected_dh_pub_x25519: &[u8; 32],
        now_unix_ms: u64,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<VerifiedTicketClaims, CollabRuntimeError> {
        if cancelled() {
            return Err(authentication_unavailable());
        }
        let verified = self
            .inner
            .verify_at_cancellable(
                opaque_ticket,
                expected_dh_pub_x25519,
                now_unix_ms / 1_000,
                Instant::now(),
                cancelled,
            )
            .map_err(|error| verification_runtime_error(&error, cancelled()))?;
        if cancelled() {
            return Err(authentication_unavailable());
        }
        VerifiedTicketClaims::new_with_profile(
            verified.issuer().to_owned(),
            verified.subject().to_owned(),
            verified.device_id().to_owned(),
            *verified.dh_pub_x25519(),
            verified.expires_at_unix_ms(),
            verified.display_name().map(str::to_owned),
            verified.avatar_url().map(str::to_owned),
        )
        .map_err(|_| ticket_rejected())
    }

    /// Locally verify a freshly minted relay token before it is ever offered
    /// to a relay, so a provider fault surfaces here rather than as an opaque
    /// authentication rejection on the socket.
    ///
    /// The verified output carries no identity, so this only proves that the
    /// pinned issuer signed a live token bound to this device's X25519 key.
    fn verify_relay_token_cancellable(
        &self,
        token: &[u8],
        expected_dh_pub_x25519: &[u8; 32],
        now_unix_ms: u64,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<u64, CollabRuntimeError> {
        if cancelled() {
            return Err(authentication_unavailable());
        }
        let verified = self
            .inner
            .verify_relay_token_at_cancellable(
                token,
                expected_dh_pub_x25519,
                now_unix_ms / 1_000,
                Instant::now(),
                cancelled,
                true,
            )
            .map_err(|error| verification_runtime_error(&error, cancelled()))?;
        Ok(verified.expires_at_unix_ms())
    }
}

impl TicketVerifier for ProductionTicketVerifier {
    fn verify(
        &self,
        opaque_ticket: &[u8],
        expected_dh_pub_x25519: &[u8; 32],
        now_unix_ms: u64,
    ) -> Result<VerifiedTicketClaims, AdmissionError> {
        self.verify_cancellable(
            opaque_ticket,
            expected_dh_pub_x25519,
            now_unix_ms,
            &never_cancelled,
        )
        .map_err(|_| AdmissionError::Verification)
    }
}

/// The two credentials one signed-in device session holds at once.
///
/// * `ticket` — the full collaboration ticket. It stays inside the Noise
///   channel: it is what the owner's approval UI authenticates a joining peer
///   with, and it is what a renewal must re-prove the same principal against.
/// * `relay_bearer` — what the WSS relay sees. Normally the claim-minimized
///   relay token, which carries no account subject, device id, ticket id, or
///   profile at all, so a third-party or regional relay operator cannot
///   reconstruct who collaborates with whom from the credentials it handles.
///
/// When the linked auth ABI predates the relay-token capability, the bearer
/// falls back to the full ticket at *session start* — a static build-time
/// property, not a runtime downgrade. There is deliberately no "connect with
/// the minimized token, retry with the ticket if the relay rejects it" path:
/// a curious relay could trigger that downgrade at will just by rejecting.
pub(super) struct LocalAdmission {
    ticket: OpaqueCollabTicket,
    relay_bearer: RelayBearerCredentialSource,
    auth: VerifiedAuthMetadata,
}

pub(super) enum RelayBearerCredentialSource {
    /// The claim-minimized relay token minted alongside the ticket.
    MinimizedRelayToken(OpaqueCollabRelayToken),
    /// Pre-capability fallback: the relay still dual-accepts the full ticket.
    LegacyFullTicket,
}

impl LocalAdmission {
    pub(super) fn request_cancellable(
        local_static: &[u8; 32],
        verifier: &ProductionTicketVerifier,
        cancelled: impl Fn() -> bool,
    ) -> Result<Self, CollabRuntimeError> {
        let deadline = Instant::now()
            .checked_add(TICKET_REQUEST_TIMEOUT)
            .ok_or_else(authentication_unavailable)?;
        Self::request_cancellable_until(local_static, verifier, deadline, cancelled)
    }

    fn request_cancellable_until(
        local_static: &[u8; 32],
        verifier: &ProductionTicketVerifier,
        deadline: Instant,
        cancelled: impl Fn() -> bool,
    ) -> Result<Self, CollabRuntimeError> {
        if cancelled() {
            return Err(authentication_unavailable());
        }
        let provider = op_auth_bridge::collab_ticket_provider();
        if !provider.available() {
            return Err(authentication_unavailable());
        }
        let request =
            CollabTicketRequest::new(*local_static).map_err(|_| authentication_unavailable())?;
        let request_id = provider
            .begin_ticket(request)
            .map_err(|_| authentication_unavailable())?;
        let ticket = await_provider_credential(provider, request_id, deadline, &cancelled)?;
        if cancelled() {
            return Err(authentication_unavailable());
        }
        let now_unix_ms = unix_time_ms()?;
        let claims =
            verifier.verify_cancellable(ticket.expose(), local_static, now_unix_ms, &cancelled)?;
        if cancelled() {
            return Err(authentication_unavailable());
        }
        if claims.issuer() != verifier.expected_issuer() {
            return Err(ticket_rejected());
        }
        let auth = VerifiedAuthMetadata {
            issuer: claims.issuer().to_owned(),
            subject: claims.subject().to_owned(),
            device_id: claims.device_id().to_owned(),
            proof_binding: URL_SAFE_NO_PAD.encode(claims.dh_pub_x25519()),
            expires_at_unix_ms: claims.expires_at_unix_ms(),
            display_name: claims.display_name().map(str::to_owned),
            avatar_url: claims.avatar_url().map(str::to_owned),
        };
        if cancelled() {
            return Err(authentication_unavailable());
        }
        let relay_bearer = Self::request_relay_bearer(
            local_static,
            verifier,
            provider,
            deadline,
            now_unix_ms,
            &cancelled,
        )?;
        if cancelled() {
            return Err(authentication_unavailable());
        }
        Ok(Self {
            ticket,
            relay_bearer,
            auth,
        })
    }

    /// Mint the claim-minimized relay bearer for this session.
    ///
    /// The decision between the minimized token and the legacy full ticket is
    /// made exactly once, here, on a build-time capability of the linked auth
    /// ABI. It is never made in response to a relay's answer.
    fn request_relay_bearer(
        local_static: &[u8; 32],
        verifier: &ProductionTicketVerifier,
        provider: &'static dyn CollabTicketProvider,
        deadline: Instant,
        now_unix_ms: u64,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RelayBearerCredentialSource, CollabRuntimeError> {
        if !provider.relay_token_available() {
            return Ok(RelayBearerCredentialSource::LegacyFullTicket);
        }
        let request =
            CollabTicketRequest::new(*local_static).map_err(|_| authentication_unavailable())?;
        let request_id = provider
            .begin_relay_token(request)
            .map_err(|_| authentication_unavailable())?;
        let payload = await_provider_credential(provider, request_id, deadline, cancelled)?;
        let token = OpaqueCollabRelayToken::from_provider_payload(payload)
            .map_err(|_| ticket_rejected())?;
        verifier.verify_relay_token_cancellable(
            token.expose(),
            local_static,
            now_unix_ms,
            cancelled,
        )?;
        Ok(RelayBearerCredentialSource::MinimizedRelayToken(token))
    }

    pub(super) fn hello(&self, intent: JoinIntent) -> Result<AdmissionHello, CollabRuntimeError> {
        AdmissionHello::from_ticket_bytes(self.ticket.expose(), intent)
            .map_err(|_| CollabRuntimeError::new(CollabRuntimeFailure::TicketRejected))
    }

    pub(super) fn auth(&self) -> &VerifiedAuthMetadata {
        &self.auth
    }

    pub(super) fn expected_issuer(&self) -> &str {
        &self.auth.issuer
    }

    pub(super) fn expected_subject(&self) -> &str {
        &self.auth.subject
    }

    pub(super) fn renewal_ticket(&self) -> Result<OpaqueTicket, CollabRuntimeError> {
        OpaqueTicket::from_utf8_bytes(self.ticket.expose())
            .map_err(|_| CollabRuntimeError::new(CollabRuntimeFailure::TicketRejected))
    }
}

/// Non-blocking local ticket renewal. Provider polling runs on a dedicated
/// worker; socket drivers only poll this receiver and keep heartbeats moving.
/// A renewal is cancelled at the ticket's terminal-shutdown boundary, so a
/// short ticket has a real opportunity to renew when the provider is fast but
/// never extends its old authentication deadline for a slow provider.
pub(super) struct LocalTicketRenewer {
    local_static: [u8; 32],
    verifier: Arc<ProductionTicketVerifier>,
    binding: VerifiedAuthMetadata,
    renew_at: Instant,
    shutdown_at: Instant,
    expires_at: Instant,
    retry_backoff: Duration,
    pending: Option<PendingRenewal>,
}

impl LocalTicketRenewer {
    pub(super) fn new(
        local_static: [u8; 32],
        verifier: Arc<ProductionTicketVerifier>,
        binding: VerifiedAuthMetadata,
    ) -> Result<Self, CollabRuntimeError> {
        let now_unix_ms = unix_time_ms()?;
        let now = Instant::now();
        let (renew_at, expires_at) =
            renewal_schedule(now_unix_ms, binding.expires_at_unix_ms, now)?;
        let shutdown_at = terminal_shutdown_at(now, expires_at);
        Ok(Self {
            local_static,
            verifier,
            binding,
            renew_at,
            shutdown_at,
            expires_at,
            retry_backoff: INITIAL_RENEWAL_RETRY_BACKOFF,
            pending: None,
        })
    }

    pub(super) fn poll(
        &mut self,
        now: Instant,
    ) -> Result<Option<LocalAdmission>, CollabRuntimeError> {
        let renewal_due = renewal_due_before_shutdown(now, self.renew_at, self.shutdown_at)?;
        if self.pending.is_none() && renewal_due {
            let local_static = self.local_static;
            let verifier = Arc::clone(&self.verifier);
            let (sender, receiver) = mpsc::channel();
            let cancelled = Arc::new(AtomicBool::new(false));
            let worker_cancelled = Arc::clone(&cancelled);
            let renewal_deadline = self.shutdown_at;
            let worker = match std::thread::Builder::new()
                .name("op-collab-ticket-renewal".to_string())
                .spawn(move || {
                    let _ = sender.send(LocalAdmission::request_cancellable_until(
                        &local_static,
                        verifier.as_ref(),
                        renewal_deadline,
                        || {
                            worker_cancelled.load(Ordering::Acquire)
                                || Instant::now() >= renewal_deadline
                        },
                    ));
                }) {
                Ok(worker) => worker,
                Err(_) => {
                    self.schedule_transient_retry(now)?;
                    return Ok(None);
                }
            };
            self.pending = Some(PendingRenewal {
                receiver: Some(receiver),
                cancelled,
                worker: Some(worker),
            });
        }
        let Some(pending) = self.pending.as_ref() else {
            return Ok(None);
        };
        let admission = match pending
            .receiver
            .as_ref()
            .expect("pending renewal receiver must exist")
            .try_recv()
        {
            Ok(Ok(admission)) => admission,
            Ok(Err(error)) if error.failure == CollabRuntimeFailure::AuthenticationUnavailable => {
                self.pending = None;
                self.schedule_transient_retry(now)?;
                return Ok(None);
            }
            Ok(Err(error)) => {
                self.pending = None;
                return Err(error);
            }
            Err(TryRecvError::Empty) => return Ok(None),
            Err(TryRecvError::Disconnected) => {
                self.pending = None;
                self.schedule_transient_retry(now)?;
                return Ok(None);
            }
        };
        self.pending = None;
        validate_renewed_binding(&self.binding, admission.auth())?;
        let now_unix_ms = unix_time_ms()?;
        (self.renew_at, self.expires_at) =
            renewal_schedule(now_unix_ms, admission.auth().expires_at_unix_ms, now)?;
        self.shutdown_at = terminal_shutdown_at(now, self.expires_at);
        self.retry_backoff = INITIAL_RENEWAL_RETRY_BACKOFF;
        self.binding = admission.auth().clone();
        Ok(Some(admission))
    }

    fn schedule_transient_retry(&mut self, now: Instant) -> Result<(), CollabRuntimeError> {
        let (retry_at, next_backoff) =
            transient_retry_schedule(now, self.expires_at, self.retry_backoff)?;
        self.renew_at = retry_at;
        self.retry_backoff = next_backoff;
        Ok(())
    }
}

/// Drive one provider request to its terminal state, or fail closed.
///
/// Shared by the collaboration-ticket and relay-token requests: both live in
/// the same provider handle namespace, so a single poll/cancel loop covers
/// them and cannot drift apart on cancellation or deadline handling.
fn await_provider_credential(
    provider: &'static dyn CollabTicketProvider,
    request_id: CollabTicketRequestId,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> Result<OpaqueCollabTicket, CollabRuntimeError> {
    loop {
        if cancelled() {
            provider.cancel_ticket(request_id);
            return Err(authentication_unavailable());
        }
        match provider.poll_ticket(request_id) {
            CollabTicketPoll::Pending if Instant::now() < deadline => {
                std::thread::sleep(
                    TICKET_POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())),
                );
            }
            CollabTicketPoll::Ready { ticket, .. } => return Ok(ticket),
            CollabTicketPoll::Pending => {
                provider.cancel_ticket(request_id);
                return Err(authentication_unavailable());
            }
            CollabTicketPoll::Failed(_) => return Err(authentication_unavailable()),
        }
    }
}

pub(super) fn production_verifier() -> Result<Arc<ProductionTicketVerifier>, CollabRuntimeError> {
    Ok(Arc::new(ProductionTicketVerifier::new()?))
}

fn never_cancelled() -> bool {
    false
}

fn authentication_unavailable() -> CollabRuntimeError {
    CollabRuntimeError::new(CollabRuntimeFailure::AuthenticationUnavailable)
}

fn ticket_rejected() -> CollabRuntimeError {
    CollabRuntimeError::new(CollabRuntimeFailure::TicketRejected)
}

fn verification_runtime_error(
    error: &CollabVerifyError,
    cancellation_requested: bool,
) -> CollabRuntimeError {
    let failure = if cancellation_requested {
        CollabRuntimeFailure::AuthenticationUnavailable
    } else {
        verification_failure(error)
    };
    CollabRuntimeError::new(failure)
}

fn verification_failure(error: &CollabVerifyError) -> CollabRuntimeFailure {
    match error {
        CollabVerifyError::Cancelled => CollabRuntimeFailure::AuthenticationUnavailable,
        CollabVerifyError::Jwks(error) => jwks_verification_failure(error),
        CollabVerifyError::InvalidTicketSize { .. }
        | CollabVerifyError::MalformedCompactJws
        | CollabVerifyError::SegmentTooLarge { .. }
        | CollabVerifyError::InvalidBase64 { .. }
        | CollabVerifyError::MalformedJson { .. }
        | CollabVerifyError::WrongAlgorithm
        | CollabVerifyError::WrongType
        | CollabVerifyError::InvalidKeyId
        | CollabVerifyError::InvalidSignature
        | CollabVerifyError::InvalidIssuer
        | CollabVerifyError::InvalidAudience
        | CollabVerifyError::InvalidVersion
        | CollabVerifyError::InvalidScope
        | CollabVerifyError::InvalidSubject
        | CollabVerifyError::InvalidDeviceId
        | CollabVerifyError::InvalidChannelBinding
        | CollabVerifyError::ChannelBindingMismatch
        | CollabVerifyError::InvalidTicketId
        | CollabVerifyError::InvalidDisplayName
        | CollabVerifyError::InvalidAvatarUrl
        | CollabVerifyError::InvalidTimestamps
        | CollabVerifyError::NotYetValid
        | CollabVerifyError::Expired
        | CollabVerifyError::LifetimeTooLong
        | CollabVerifyError::ExpiryOverflow => CollabRuntimeFailure::TicketRejected,
    }
}

fn jwks_verification_failure(error: &CollabJwksError) -> CollabRuntimeFailure {
    // A bad authority response cannot validate the renewal, but it also does
    // not invalidate the already-verified ticket. Unknown kid is different.
    match error {
        CollabJwksError::UnknownKey => CollabRuntimeFailure::TicketRejected,
        CollabJwksError::InvalidBodySize { .. }
        | CollabJwksError::MalformedJson
        | CollabJwksError::EmptyKeyset
        | CollabJwksError::TooManyKeys { .. }
        | CollabJwksError::DuplicateKeyId
        | CollabJwksError::InvalidKey { .. }
        | CollabJwksError::InvalidEtag { .. }
        | CollabJwksError::RefreshThrottled
        | CollabJwksError::CacheUnavailable
        | CollabJwksError::NotModifiedWithoutCache
        | CollabJwksError::Policy(_) => CollabRuntimeFailure::AuthenticationUnavailable,
        CollabJwksError::Fetch(error) => jwks_fetch_failure(error),
    }
}

fn jwks_fetch_failure(error: &CollabJwksFetchError) -> CollabRuntimeFailure {
    match error {
        CollabJwksFetchError::Cancelled
        | CollabJwksFetchError::Unavailable
        | CollabJwksFetchError::RejectedResponse
        | CollabJwksFetchError::ResponseTooLarge => CollabRuntimeFailure::AuthenticationUnavailable,
    }
}

pub(super) fn unix_time_ms() -> Result<u64, CollabRuntimeError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| CollabRuntimeError::new(CollabRuntimeFailure::ClockUnavailable))?
        .as_millis();
    u64::try_from(millis)
        .map_err(|_| CollabRuntimeError::new(CollabRuntimeFailure::ClockUnavailable))
}

fn renewal_schedule(
    now_unix_ms: u64,
    expires_at_unix_ms: u64,
    now: Instant,
) -> Result<(Instant, Instant), CollabRuntimeError> {
    let ttl = expires_at_unix_ms
        .checked_sub(now_unix_ms)
        .filter(|ttl| *ttl > 0)
        .ok_or_else(|| CollabRuntimeError::new(CollabRuntimeFailure::TicketRejected))?;
    let renewal_ms = ttl / 2;
    let renew_at = now
        .checked_add(Duration::from_millis(renewal_ms))
        .ok_or_else(|| CollabRuntimeError::new(CollabRuntimeFailure::ClockUnavailable))?;
    let expires_at = now
        .checked_add(Duration::from_millis(ttl))
        .ok_or_else(|| CollabRuntimeError::new(CollabRuntimeFailure::ClockUnavailable))?;
    Ok((renew_at, expires_at))
}

fn terminal_shutdown_at(now: Instant, expires_at: Instant) -> Instant {
    let remaining = expires_at.saturating_duration_since(now);
    let reserved = TERMINAL_FLUSH_TIMEOUT.min(remaining / 4);
    expires_at.checked_sub(reserved).unwrap_or(now)
}

fn renewal_due_before_shutdown(
    now: Instant,
    renew_at: Instant,
    shutdown_at: Instant,
) -> Result<bool, CollabRuntimeError> {
    if now >= shutdown_at {
        return Err(CollabRuntimeError::new(
            CollabRuntimeFailure::TicketRejected,
        ));
    }
    Ok(now >= renew_at)
}

fn transient_retry_schedule(
    now: Instant,
    expires_at: Instant,
    backoff: Duration,
) -> Result<(Instant, Duration), CollabRuntimeError> {
    if now >= expires_at {
        return Err(CollabRuntimeError::new(
            CollabRuntimeFailure::TicketRejected,
        ));
    }
    let remaining = expires_at.saturating_duration_since(now);
    let delay = backoff.min(MAX_RENEWAL_RETRY_BACKOFF).min(remaining);
    let retry_at = now
        .checked_add(delay)
        .ok_or_else(|| CollabRuntimeError::new(CollabRuntimeFailure::ClockUnavailable))?;
    let next_backoff = backoff
        .checked_mul(2)
        .unwrap_or(MAX_RENEWAL_RETRY_BACKOFF)
        .min(MAX_RENEWAL_RETRY_BACKOFF);
    Ok((retry_at, next_backoff))
}

fn validate_renewed_binding(
    current: &VerifiedAuthMetadata,
    renewed: &VerifiedAuthMetadata,
) -> Result<(), CollabRuntimeError> {
    if current.issuer != renewed.issuer
        || current.subject != renewed.subject
        || current.device_id != renewed.device_id
        || current.proof_binding != renewed.proof_binding
        || renewed.expires_at_unix_ms <= current.expires_at_unix_ms
    {
        return Err(CollabRuntimeError::new(
            CollabRuntimeFailure::TicketRejected,
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "auth/renewal_schedule_tests.rs"]
mod renewal_schedule_tests;

#[cfg(test)]
#[path = "auth/verifier_tests.rs"]
mod tests;
