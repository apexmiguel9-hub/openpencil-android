//! HTTP client arms for the pairing-code endpoints.
//!
//! Both share `RelayLocatorHttpClient`'s hardened builder (no redirects, no
//! proxy, https-only, bounded timeouts); the pairing paths are derived from
//! the validated locator endpoint so a bootstrap document cannot point the
//! two at different hosts.

use std::io::Read as _;

use op_auth_bridge::OpaqueCollabTicket;
use op_collab_relay_protocol::{SealedPairingInvite, MAX_SEALED_INVITE_BYTES};
use reqwest::{
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
    StatusCode, Url,
};

use crate::http_client::{authorization_header, publish_status, RelayLocatorHttpClient};
use crate::pairing_wire::{
    PairingClaimRequest, PairingPublishRequest, PAIRING_CLAIM_CONTENT_TYPE, PAIRING_CLAIM_PATH,
    PAIRING_PUBLISH_CONTENT_TYPE, PAIRING_PUBLISH_PATH, SEALED_INVITE_CONTENT_TYPE,
};
use crate::RelayLocatorIssueError;

/// Claim outcome distinguishing "no such live code" from transport trouble.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PairingClaimError {
    #[error("pairing code not found or expired")]
    NotFound,
    #[error("pairing claim was rejected")]
    Rejected,
    /// The control plane refused the collaboration ticket (401/403) — a
    /// sign-in problem, not a bad code and not a flaky network.
    #[error("pairing claim credential was refused")]
    Unauthorized,
    /// The control plane is shedding load (429); retrying later helps.
    #[error("pairing claim was rate limited")]
    RateLimited,
    #[error("pairing claim transport unavailable")]
    TransportUnavailable,
}

impl<V> RelayLocatorHttpClient<V> {
    fn pairing_url(&self, path: &str) -> Url {
        let mut url = self.endpoint_clone();
        url.set_path(path);
        url
    }

    /// Store a sealed invite under its code id. Best-effort on the owner
    /// side: a failure leaves the long invite fully usable.
    pub fn publish_pairing_code(
        &self,
        request: &PairingPublishRequest,
        ticket: &OpaqueCollabTicket,
    ) -> Result<(), RelayLocatorIssueError> {
        let authorization = authorization_header(ticket.expose())?;
        let response = self
            .http_client()
            .post(self.pairing_url(PAIRING_PUBLISH_PATH))
            .header(CONTENT_TYPE, PAIRING_PUBLISH_CONTENT_TYPE)
            .header(AUTHORIZATION, authorization)
            .body(request.encode_binary())
            .send()
            .map_err(|_| RelayLocatorIssueError::PublishTransportUnavailable)?;
        // Success here is 204, so the shared classifier only gets to see the
        // failure statuses it exists to separate.
        match response.status() {
            StatusCode::NO_CONTENT => Ok(()),
            // `publish_status` treats 200 as success; here it is not one, so
            // an Ok from the classifier still means "rejected".
            status => Err(publish_status(status)
                .err()
                .unwrap_or(RelayLocatorIssueError::PublishResponseRejected)),
        }
    }

    /// Redeem a code id for the sealed blob stored by the owner.
    pub fn claim_pairing_code(
        &self,
        request: &PairingClaimRequest,
        ticket: &OpaqueCollabTicket,
    ) -> Result<SealedPairingInvite, PairingClaimError> {
        let authorization =
            authorization_header(ticket.expose()).map_err(|_| PairingClaimError::Rejected)?;
        let response = self
            .http_client()
            .post(self.pairing_url(PAIRING_CLAIM_PATH))
            .header(CONTENT_TYPE, PAIRING_CLAIM_CONTENT_TYPE)
            .header(ACCEPT, SEALED_INVITE_CONTENT_TYPE)
            .header(AUTHORIZATION, authorization)
            .body(request.encode_binary().to_vec())
            .send()
            .map_err(|_| PairingClaimError::TransportUnavailable)?;
        match response.status() {
            StatusCode::OK => {}
            StatusCode::NOT_FOUND => return Err(PairingClaimError::NotFound),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                return Err(PairingClaimError::Unauthorized)
            }
            StatusCode::TOO_MANY_REQUESTS => return Err(PairingClaimError::RateLimited),
            _ => return Err(PairingClaimError::Rejected),
        }
        let content_type_matches = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value == SEALED_INVITE_CONTENT_TYPE);
        if !content_type_matches {
            return Err(PairingClaimError::Rejected);
        }
        let mut body = Vec::with_capacity(MAX_SEALED_INVITE_BYTES);
        response
            .take((MAX_SEALED_INVITE_BYTES + 1) as u64)
            .read_to_end(&mut body)
            .map_err(|_| PairingClaimError::TransportUnavailable)?;
        SealedPairingInvite::from_bytes(&body).map_err(|_| PairingClaimError::Rejected)
    }
}
