use std::{
    fmt,
    time::{SystemTime, UNIX_EPOCH},
};

use op_auth_bridge::VerifiedCollabClaims;
use op_collab_relay_protocol::{
    LocatorKeyId, LocatorSignature, RelayLocatorV1, UnsignedRelayLocatorV1,
    LOCATOR_CANONICAL_SIGNING_BYTES,
};
use subtle::ConstantTimeEq as _;

use crate::{OwnerPublishRequest, RelayLocatorIssueError, RelayLocatorSignerError};

/// An already verified ticket identity accepted at the control-plane boundary.
///
/// It can only be built from [`VerifiedCollabClaims`], whose fields are private
/// to the signed-ticket verifier. Raw bearer bytes or unverified JWT claims
/// cannot cross this API.
pub struct TicketVerifiedOwnerBinding<'a> {
    ticket: &'a VerifiedCollabClaims,
}

impl<'a> TicketVerifiedOwnerBinding<'a> {
    pub const fn from_verified_ticket(ticket: &'a VerifiedCollabClaims) -> Self {
        Self { ticket }
    }
}

impl fmt::Debug for TicketVerifiedOwnerBinding<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TicketVerifiedOwnerBinding([REDACTED])")
    }
}

/// External locator signing boundary.
///
/// Production implementations should select an HSM/KMS key in
/// [`Self::active_key_id`] and sign with that exact key in [`Self::sign`].
/// Neither a software private-key implementation nor production key material
/// is provided by this crate.
pub trait RelayLocatorSigner: Send + Sync {
    fn active_key_id(&self) -> Result<LocatorKeyId, RelayLocatorSignerError>;

    fn sign(
        &self,
        key_id: &LocatorKeyId,
        canonical_signing_bytes: &[u8; LOCATOR_CANONICAL_SIGNING_BYTES],
    ) -> Result<LocatorSignature, RelayLocatorSignerError>;
}

pub struct RelayLocatorIssuer<S> {
    signer: S,
}

impl<S> RelayLocatorIssuer<S> {
    pub const fn new(signer: S) -> Self {
        Self { signer }
    }
}

impl<S: RelayLocatorSigner> RelayLocatorIssuer<S> {
    pub fn issue(
        &self,
        request: &OwnerPublishRequest,
        binding: &TicketVerifiedOwnerBinding<'_>,
    ) -> Result<SignedLocatorResponse, RelayLocatorIssueError> {
        self.issue_at(request, binding, unix_now()?)
    }

    pub fn issue_at(
        &self,
        request: &OwnerPublishRequest,
        binding: &TicketVerifiedOwnerBinding<'_>,
        now_unix: u64,
    ) -> Result<SignedLocatorResponse, RelayLocatorIssueError> {
        if now_unix == 0 {
            return Err(RelayLocatorIssueError::ZeroCurrentTime);
        }
        let ticket = binding.ticket;
        let ticket_is_current = ticket.not_before_unix_seconds() <= now_unix
            && now_unix < ticket.expires_at_unix_seconds();
        let owner_key_matches = bool::from(
            ticket
                .dh_pub_x25519()
                .ct_eq(request.owner_noise_static().as_bytes()),
        );
        if !ticket_is_current || !owner_key_matches {
            return Err(RelayLocatorIssueError::OwnerBindingRejected);
        }
        let expires_at_unix = now_unix
            .checked_add(request.desired_lifetime().seconds())
            .ok_or(RelayLocatorIssueError::ExpiryOverflow)?;
        let key_id = self.signer.active_key_id()?;
        let claims = UnsignedRelayLocatorV1::new(
            request.home_region(),
            *request.route_id(),
            request.generation(),
            *request.owner_noise_static(),
            request.expected_discovery_id().clone(),
            now_unix,
            expires_at_unix,
            key_id.clone(),
        )?;
        claims.validate_pairing_window(now_unix)?;
        let signature = self
            .signer
            .sign(&key_id, &claims.canonical_signing_bytes())?;
        Ok(SignedLocatorResponse {
            locator: claims.attach_signature(signature),
        })
    }
}

impl<S> fmt::Debug for RelayLocatorIssuer<S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RelayLocatorIssuer")
            .field("signer", &"[EXTERNAL]")
            .finish()
    }
}

pub struct SignedLocatorResponse {
    pub(crate) locator: RelayLocatorV1,
}

impl SignedLocatorResponse {
    pub fn locator(&self) -> &RelayLocatorV1 {
        &self.locator
    }

    pub fn encode(&self) -> String {
        self.locator.encode()
    }

    pub fn decode(encoded: &str) -> Result<Self, RelayLocatorIssueError> {
        Ok(Self {
            locator: RelayLocatorV1::decode(encoded)?,
        })
    }
}

impl fmt::Debug for SignedLocatorResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SignedLocatorResponse([REDACTED])")
    }
}

fn unix_now() -> Result<u64, RelayLocatorIssueError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| RelayLocatorIssueError::ClockBeforeUnixEpoch)?
        .as_secs();
    if now == 0 {
        return Err(RelayLocatorIssueError::ZeroCurrentTime);
    }
    Ok(now)
}
