use std::{
    fmt,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use op_auth_bridge::{
    CollabJwksFetcher, CollabTicketVerifier, CollabVerifierConfigError, VerifiedCollabClaims,
};
use op_collab_relay_protocol::RelayRegion;

use crate::{
    OwnerPublishRequest, RelayLocatorIssueError, RelayLocatorIssuer, RelayLocatorSigner,
    SignedLocatorResponse, TicketVerifiedOwnerBinding,
};

#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
pub enum RelayLocatorPublishServiceError {
    #[error("owner publish request was rejected")]
    RequestRejected,
    #[error("owner publish authentication failed")]
    AuthenticationFailed,
    #[error("locator issuance is unavailable")]
    Unavailable,
}

pub struct RelayOwnerPublishPolicyContext<'a> {
    request: &'a OwnerPublishRequest,
    ticket: &'a VerifiedCollabClaims,
}

impl<'a> RelayOwnerPublishPolicyContext<'a> {
    fn new(request: &'a OwnerPublishRequest, ticket: &'a VerifiedCollabClaims) -> Self {
        Self { request, ticket }
    }

    pub const fn request(&self) -> &OwnerPublishRequest {
        self.request
    }

    pub const fn ticket(&self) -> &VerifiedCollabClaims {
        self.ticket
    }
}

impl fmt::Debug for RelayOwnerPublishPolicyContext<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RelayOwnerPublishPolicyContext")
            .field("home_region", &self.request.home_region())
            .field("ticket", &"[REDACTED]")
            .finish()
    }
}

/// Mandatory deployment authorization policy for owner publication.
///
/// A deployment with account entitlements should inspect the already verified
/// ticket through the context. Rejection is deliberately collapsed into the
/// same authentication failure as an invalid ticket.
pub trait RelayOwnerPublishPolicy: Send + Sync {
    fn authorize(&self, context: &RelayOwnerPublishPolicyContext<'_>) -> bool;
}

/// Explicit policy for deployments where every authenticated account may host
/// in exactly one configured residency.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionBoundOwnerPublishPolicy {
    home_region: RelayRegion,
}

impl RegionBoundOwnerPublishPolicy {
    pub const fn new(home_region: RelayRegion) -> Self {
        Self { home_region }
    }
}

impl RelayOwnerPublishPolicy for RegionBoundOwnerPublishPolicy {
    fn authorize(&self, context: &RelayOwnerPublishPolicyContext<'_>) -> bool {
        context.request().home_region() == self.home_region
    }
}

/// Transport-neutral handler core for `POST /v1/locator`.
///
/// The HTTP wrapper is responsible only for strict method/path/content-type,
/// request-size, and status mapping. Ticket validation and locator issuance
/// stay single-sourced here.
pub struct RelayLocatorPublishService<F, S, P> {
    ticket_verifier: CollabTicketVerifier<F>,
    issuer: RelayLocatorIssuer<S>,
    policy: P,
}

impl<F, S, P> RelayLocatorPublishService<F, S, P>
where
    F: CollabJwksFetcher,
    S: RelayLocatorSigner,
    P: RelayOwnerPublishPolicy,
{
    pub fn production(fetcher: F, signer: S, policy: P) -> Result<Self, CollabVerifierConfigError> {
        Ok(Self::new(
            CollabTicketVerifier::production(fetcher)?,
            signer,
            policy,
        ))
    }

    pub const fn new(ticket_verifier: CollabTicketVerifier<F>, signer: S, policy: P) -> Self {
        Self {
            ticket_verifier,
            issuer: RelayLocatorIssuer::new(signer),
            policy,
        }
    }

    pub fn publish(
        &self,
        request_body: &[u8],
        opaque_ticket: &[u8],
    ) -> Result<SignedLocatorResponse, RelayLocatorPublishServiceError> {
        let now_unix = unix_now().ok_or(RelayLocatorPublishServiceError::Unavailable)?;
        self.publish_at(request_body, opaque_ticket, now_unix, Instant::now())
    }

    pub fn publish_at(
        &self,
        request_body: &[u8],
        opaque_ticket: &[u8],
        now_unix: u64,
        cache_now: Instant,
    ) -> Result<SignedLocatorResponse, RelayLocatorPublishServiceError> {
        let request = OwnerPublishRequest::decode_binary(request_body)
            .map_err(|_| RelayLocatorPublishServiceError::RequestRejected)?;
        let verified_ticket = self
            .ticket_verifier
            .verify_at(
                opaque_ticket,
                request.owner_noise_static().as_bytes(),
                now_unix,
                cache_now,
            )
            .map_err(|_| RelayLocatorPublishServiceError::AuthenticationFailed)?;
        let policy_context = RelayOwnerPublishPolicyContext::new(&request, &verified_ticket);
        if !self.policy.authorize(&policy_context) {
            return Err(RelayLocatorPublishServiceError::AuthenticationFailed);
        }
        self.issuer
            .issue_at(
                &request,
                &TicketVerifiedOwnerBinding::from_verified_ticket(&verified_ticket),
                now_unix,
            )
            .map_err(map_issue_error)
    }
}

impl<F, S, P> fmt::Debug for RelayLocatorPublishService<F, S, P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RelayLocatorPublishService")
            .field("ticket_verifier", &"[CONFIGURED]")
            .field("signer", &"[EXTERNAL]")
            .field("policy", &"[CONFIGURED]")
            .finish()
    }
}

fn map_issue_error(error: RelayLocatorIssueError) -> RelayLocatorPublishServiceError {
    match error {
        RelayLocatorIssueError::OwnerBindingRejected => {
            RelayLocatorPublishServiceError::AuthenticationFailed
        }
        RelayLocatorIssueError::Protocol(_)
        | RelayLocatorIssueError::ZeroLifetime
        | RelayLocatorIssueError::LifetimeTooLong { .. }
        | RelayLocatorIssueError::InvalidRequestLength { .. }
        | RelayLocatorIssueError::UnsupportedRequestVersion { .. }
        | RelayLocatorIssueError::NonZeroRequestPadding
        | RelayLocatorIssueError::InvalidRequestUtf8 => {
            RelayLocatorPublishServiceError::RequestRejected
        }
        _ => RelayLocatorPublishServiceError::Unavailable,
    }
}

fn unix_now() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
        .filter(|now| *now != 0)
}
