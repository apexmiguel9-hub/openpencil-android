use std::{
    fmt,
    num::NonZeroU64,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use op_auth_bridge::{CollabJwksFetcher, CollabTicketVerifier, CollabVerifierConfigError};
use op_collab_relay_protocol::{
    LocatorKeyId, RelayChallengeKeyId, RelayChallengeProofV2, RelayClientHello, RelayHelloAuthMode,
    RelayLocatorVerifier, RelayProtocolError, RelayRegion, RelayRejectCode, RelayRole,
    RelayServerChallengeV1, RouteMapKey, MAX_PAIRING_LIFETIME_SECS,
};
use subtle::ConstantTimeEq as _;
use zeroize::Zeroize;

use crate::RelayServerX25519ProofBoundary;

/// A strictly parsed HTTP `Authorization: Bearer` credential.
///
/// The owned bytes are wiped on drop and intentionally neither cloneable nor
/// printable.
pub struct RelayBearerCredential {
    bytes: Vec<u8>,
}

impl RelayBearerCredential {
    pub(crate) fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl fmt::Debug for RelayBearerCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RelayBearerCredential([REDACTED])")
    }
}

impl Drop for RelayBearerCredential {
    fn drop(&mut self) {
        self.bytes.zeroize();
    }
}

/// The opaque route, role, and authentication expiry accepted by the relay.
#[derive(Clone, Copy)]
pub struct AuthenticatedRoute {
    pub(crate) route: RouteMapKey,
    pub(crate) role: RelayRole,
    pub(crate) expires_at_unix: NonZeroU64,
}

impl AuthenticatedRoute {
    pub fn new(route: RouteMapKey, role: RelayRole, expires_at_unix: NonZeroU64) -> Self {
        Self {
            route,
            role,
            expires_at_unix,
        }
    }

    pub const fn role(&self) -> RelayRole {
        self.role
    }

    pub const fn expires_at_unix(&self) -> NonZeroU64 {
        self.expires_at_unix
    }
}

impl fmt::Debug for AuthenticatedRoute {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedRoute")
            .field("route", &"[REDACTED]")
            .field("role", &self.role)
            .field("expires_at_unix", &self.expires_at_unix)
            .finish()
    }
}

/// Fresh challenge retained only by one upgraded WebSocket connection.
///
/// This type is intentionally not cloneable. The connection passes it by
/// value into exactly one authentication call, which consumes it even when
/// verification fails.
pub struct RelayUpgradeChallenge {
    challenge: RelayServerChallengeV1,
    expires_at: Instant,
}

impl RelayUpgradeChallenge {
    pub(crate) fn generate(
        key_id: RelayChallengeKeyId,
        expires_at: Instant,
    ) -> Result<Self, RelayRejectCode> {
        let challenge = RelayServerChallengeV1::generate(key_id)
            .map_err(|_| RelayRejectCode::RelayUnavailable)?;
        Ok(Self {
            challenge,
            expires_at,
        })
    }

    pub(crate) fn challenge(&self) -> &RelayServerChallengeV1 {
        &self.challenge
    }

    fn consume_if_fresh(self, now: Instant) -> Option<RelayServerChallengeV1> {
        (now < self.expires_at).then_some(self.challenge)
    }
}

impl fmt::Debug for RelayUpgradeChallenge {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RelayUpgradeChallenge([REDACTED])")
    }
}

/// Authentication boundary for a relay ClientHello.
pub trait RelayAuthenticator: Send + Sync {
    /// Selects the key id for a fresh 101-response challenge.
    ///
    /// `None` is accepted only by explicit reduced/development policies.
    fn challenge_key_id(&self) -> Result<Option<RelayChallengeKeyId>, RelayRejectCode> {
        Ok(None)
    }

    fn authenticate(
        &self,
        hello: &RelayClientHello,
        credential: Option<&RelayBearerCredential>,
        challenge: Option<RelayUpgradeChallenge>,
    ) -> Result<AuthenticatedRoute, RelayRejectCode>;
}

/// Full production policy backed by an external/HSM-capable X25519 boundary.
pub struct RequireRelayChallengeProof<B> {
    boundary: B,
}

impl<B> RequireRelayChallengeProof<B> {
    pub fn new(boundary: B) -> Self {
        Self { boundary }
    }
}

impl<B: RelayServerX25519ProofBoundary> RelayPossessionPolicy for RequireRelayChallengeProof<B> {
    fn challenge_key_id(&self) -> Result<Option<RelayChallengeKeyId>, RelayRejectCode> {
        self.boundary
            .active_key_id()
            .map(Some)
            .map_err(|_| RelayRejectCode::RelayUnavailable)
    }

    fn expected_auth_mode(&self) -> RelayHelloAuthMode {
        RelayHelloAuthMode::ChallengeBoundX25519V2
    }

    fn verify(
        &self,
        hello: &RelayClientHello,
        credential: &RelayBearerCredential,
        challenge: Option<RelayUpgradeChallenge>,
    ) -> bool {
        let Some(challenge) = challenge.and_then(|value| value.consume_if_fresh(Instant::now()))
        else {
            return false;
        };
        let Some(proof) = hello.auth_extension().possession_proof() else {
            return false;
        };
        let Ok(proof) = RelayChallengeProofV2::decode(proof) else {
            return false;
        };
        self.boundary
            .verify(
                &challenge,
                hello.auth_extension().caller_device_dh_pub_x25519(),
                credential.as_bytes(),
                hello,
                &proof,
            )
            .unwrap_or(false)
    }
}

impl<B> fmt::Debug for RequireRelayChallengeProof<B> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RequireRelayChallengeProof")
            .field("boundary", &"[EXTERNAL]")
            .finish()
    }
}

/// Explicit reduced-assurance policy relying only on ticket-to-DH binding.
#[derive(Clone, Copy, Debug, Default)]
pub struct TicketDhBindingOnly;

impl RelayPossessionPolicy for TicketDhBindingOnly {
    fn expected_auth_mode(&self) -> RelayHelloAuthMode {
        RelayHelloAuthMode::SignedLocatorAndBearerTicketV1
    }

    fn verify(
        &self,
        hello: &RelayClientHello,
        _credential: &RelayBearerCredential,
        challenge: Option<RelayUpgradeChallenge>,
    ) -> bool {
        challenge.is_none() && hello.auth_extension().possession_proof().is_none()
    }
}

pub trait RelayPossessionPolicy: Send + Sync {
    fn challenge_key_id(&self) -> Result<Option<RelayChallengeKeyId>, RelayRejectCode> {
        Ok(None)
    }

    fn expected_auth_mode(&self) -> RelayHelloAuthMode;

    fn verify(
        &self,
        hello: &RelayClientHello,
        credential: &RelayBearerCredential,
        challenge: Option<RelayUpgradeChallenge>,
    ) -> bool;
}

/// Signed-bearer relay authentication with strict or explicit reduced policy.
///
/// The bearer is a *claim-minimized relay token*: it proves a live issuer
/// session bound to the caller's X25519 key and nothing else. The relay reads
/// exactly one field out of it — the signed expiry it clamps the session
/// deadline to. Route and role come from the Ed25519-signed locator plus the
/// `RouteCapability` in the hello, never from bearer claims.
///
/// During migration the relay also accepts the legacy full collaboration
/// ticket. That path is gated by [`Self::with_full_collab_ticket_accepted`]
/// and still yields only an expiry, so the relay cannot read the account
/// subject or device id even from a legacy bearer.
pub struct CollabTicketRelayAuthenticator<F, V, P> {
    ticket_verifier: CollabTicketVerifier<F>,
    expected_home_region: RelayRegion,
    locator_verifier: V,
    possession_policy: P,
    accept_full_collab_ticket: bool,
}

impl<F, V, B> CollabTicketRelayAuthenticator<F, V, RequireRelayChallengeProof<B>>
where
    F: CollabJwksFetcher,
{
    pub fn production(
        fetcher: F,
        expected_home_region: RelayRegion,
        locator_verifier: V,
        proof_boundary: B,
    ) -> Result<Self, CollabVerifierConfigError> {
        Ok(Self::new(
            CollabTicketVerifier::production(fetcher)?,
            expected_home_region,
            locator_verifier,
            proof_boundary,
        ))
    }

    pub fn new(
        ticket_verifier: CollabTicketVerifier<F>,
        expected_home_region: RelayRegion,
        locator_verifier: V,
        proof_boundary: B,
    ) -> Self {
        Self {
            ticket_verifier,
            expected_home_region,
            locator_verifier,
            possession_policy: RequireRelayChallengeProof::new(proof_boundary),
            accept_full_collab_ticket: true,
        }
    }
}

impl<F, V> CollabTicketRelayAuthenticator<F, V, TicketDhBindingOnly>
where
    F: CollabJwksFetcher,
{
    pub fn reduced_assurance_ticket_binding_only(
        fetcher: F,
        expected_home_region: RelayRegion,
        locator_verifier: V,
    ) -> Result<Self, CollabVerifierConfigError> {
        Ok(Self::new_ticket_binding_only(
            CollabTicketVerifier::production(fetcher)?,
            expected_home_region,
            locator_verifier,
        ))
    }

    pub fn new_ticket_binding_only(
        ticket_verifier: CollabTicketVerifier<F>,
        expected_home_region: RelayRegion,
        locator_verifier: V,
    ) -> Self {
        Self {
            ticket_verifier,
            expected_home_region,
            locator_verifier,
            possession_policy: TicketDhBindingOnly,
            accept_full_collab_ticket: true,
        }
    }
}

impl<F, V, P> CollabTicketRelayAuthenticator<F, V, P> {
    /// Select whether the legacy full collaboration ticket is still accepted
    /// as a relay bearer.
    ///
    /// Dual-accept defaults to on so pre-migration clients keep connecting.
    /// Turning it off is a one-way narrowing of what identity ever reaches the
    /// relay operator and should only happen once the fleet has moved.
    ///
    /// There is deliberately no client-side "try minimized, retry with the
    /// full ticket on rejection" fallback anywhere in this system: a curious
    /// relay could trigger that downgrade at will simply by rejecting the
    /// minimized token.
    pub fn with_full_collab_ticket_accepted(mut self, accepted: bool) -> Self {
        self.accept_full_collab_ticket = accepted;
        self
    }
}

impl<F, V, P> RelayAuthenticator for CollabTicketRelayAuthenticator<F, V, P>
where
    F: CollabJwksFetcher,
    V: RelayLocatorVerifier + Send + Sync,
    P: RelayPossessionPolicy,
{
    fn challenge_key_id(&self) -> Result<Option<RelayChallengeKeyId>, RelayRejectCode> {
        self.possession_policy.challenge_key_id()
    }

    fn authenticate(
        &self,
        hello: &RelayClientHello,
        credential: Option<&RelayBearerCredential>,
        challenge: Option<RelayUpgradeChallenge>,
    ) -> Result<AuthenticatedRoute, RelayRejectCode> {
        let now_unix = unix_now().ok_or(RelayRejectCode::AuthenticationFailed)?;
        self.authenticate_at(hello, credential, challenge, now_unix, Instant::now())
    }
}

impl<F, V, P> CollabTicketRelayAuthenticator<F, V, P>
where
    F: CollabJwksFetcher,
    V: RelayLocatorVerifier,
    P: RelayPossessionPolicy,
{
    fn authenticate_at(
        &self,
        hello: &RelayClientHello,
        credential: Option<&RelayBearerCredential>,
        challenge: Option<RelayUpgradeChallenge>,
        now_unix: u64,
        cache_now: Instant,
    ) -> Result<AuthenticatedRoute, RelayRejectCode> {
        let fail = || RelayRejectCode::AuthenticationFailed;
        if hello.auth_mode() != self.possession_policy.expected_auth_mode() {
            return Err(fail());
        }
        let credential = credential.ok_or_else(fail)?;
        let verified_hello = hello
            .verify_locator(&self.locator_verifier, now_unix)
            .map_err(|_| fail())?;
        if verified_hello.route().locator().claims().home_region() != self.expected_home_region {
            return Err(fail());
        }
        let caller_dh = verified_hello
            .auth_extension()
            .caller_device_dh_pub_x25519()
            .as_bytes();
        if verified_hello.role() == RelayRole::Owner
            && !bool::from(
                caller_dh.ct_eq(
                    verified_hello
                        .route()
                        .locator()
                        .claims()
                        .owner_noise_static()
                        .as_bytes(),
                ),
            )
        {
            return Err(fail());
        }
        // The only fact taken from the bearer is its signed expiry. The
        // credential kind is discriminated on the JWS `typ` before any claim
        // parsing, and both shapes collapse to an expiry, so no identity
        // claim is reachable from here even on the legacy dual-accept path.
        let verified_bearer = self
            .ticket_verifier
            .verify_relay_bearer_at(
                credential.as_bytes(),
                caller_dh,
                now_unix,
                cache_now,
                self.accept_full_collab_ticket,
            )
            .map_err(|_| fail())?;
        if !self.possession_policy.verify(hello, credential, challenge) {
            return Err(fail());
        }
        let expires_at_unix = verified_bearer
            .expires_at_unix_seconds()
            .min(hello.expires_at_unix());
        let expires_at_unix = NonZeroU64::new(expires_at_unix).ok_or_else(fail)?;
        Ok(AuthenticatedRoute::new(
            verified_hello.route_map_key(),
            verified_hello.role(),
            expires_at_unix,
        ))
    }
}

impl<F, V, P> fmt::Debug for CollabTicketRelayAuthenticator<F, V, P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CollabTicketRelayAuthenticator")
            .field("ticket_verifier", &"[CONFIGURED]")
            .field("expected_home_region", &self.expected_home_region)
            .field("locator_verifier", &"[CONFIGURED]")
            .field("possession_policy", &"[CONFIGURED]")
            .field("accept_full_collab_ticket", &self.accept_full_collab_ticket)
            .finish()
    }
}

pub(crate) struct UnauthenticatedDevAuthenticator<N> {
    now: N,
}

impl<N> UnauthenticatedDevAuthenticator<N> {
    pub(crate) fn new(now: N) -> Self {
        Self { now }
    }
}

impl<N> RelayAuthenticator for UnauthenticatedDevAuthenticator<N>
where
    N: Fn() -> SystemTime + Send + Sync,
{
    fn authenticate(
        &self,
        hello: &RelayClientHello,
        _credential: Option<&RelayBearerCredential>,
        challenge: Option<RelayUpgradeChallenge>,
    ) -> Result<AuthenticatedRoute, RelayRejectCode> {
        if challenge.is_some()
            || hello.auth_mode() != RelayHelloAuthMode::SignedLocatorAndBearerTicketV1
        {
            return Err(RelayRejectCode::AuthenticationFailed);
        }
        let now = (self.now)()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| RelayRejectCode::Internal)?
            .as_secs();
        if hello.expires_at_unix().saturating_sub(now) > MAX_PAIRING_LIFETIME_SECS {
            return Err(RelayRejectCode::ExpiryTooFarFuture);
        }
        let verified = hello
            .verify_locator(&DevelopmentLocatorVerifier, now)
            .map_err(locator_reject_code)?;
        Ok(AuthenticatedRoute {
            route: verified.route_map_key(),
            role: verified.role(),
            expires_at_unix: NonZeroU64::new(verified.route().locator().claims().expires_at_unix())
                .ok_or(RelayRejectCode::Internal)?,
        })
    }
}

struct DevelopmentLocatorVerifier;

impl RelayLocatorVerifier for DevelopmentLocatorVerifier {
    fn verify(
        &self,
        _key_id: &LocatorKeyId,
        _canonical_signing_bytes: &[u8],
        _signature: &[u8; 64],
    ) -> bool {
        true
    }
}

fn locator_reject_code(error: RelayProtocolError) -> RelayRejectCode {
    match error {
        RelayProtocolError::NotYetValid => RelayRejectCode::LocatorNotYetValid,
        RelayProtocolError::Expired => RelayRejectCode::LocatorExpired,
        RelayProtocolError::ExpiryTooFarFuture | RelayProtocolError::ValidityWindowTooLong => {
            RelayRejectCode::ExpiryTooFarFuture
        }
        RelayProtocolError::UnsupportedVersion { .. } => RelayRejectCode::UnsupportedVersion,
        _ => RelayRejectCode::AuthenticationFailed,
    }
}

fn unix_now() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}
