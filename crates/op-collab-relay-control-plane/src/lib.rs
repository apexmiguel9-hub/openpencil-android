//! Open locator issuance core for the OpenPencil public collaboration relay.
//!
//! The owner generates the route id, generation, and bearer capability
//! locally. Only the bounded [`OwnerPublishRequest`] is sent over authenticated
//! HTTPS. A control plane verifies the collaboration ticket, binds its device
//! DH key to the requested owner Noise key, and delegates signing to an
//! external HSM/KMS through [`RelayLocatorSigner`]. The owner then verifies the
//! signed response against both pinned locator keys and its exact pending
//! request before combining it with the locally held capability.

mod error;
mod http_client;
mod issuer;
mod pairing_client;
mod pairing_service;
mod pairing_wire;
mod publish;
mod service;

pub use error::{RelayLocatorIssueError, RelayLocatorSignerError};
pub use http_client::{
    RelayLocatorHttpClient, CONTROL_PLANE_CONNECT_TIMEOUT, CONTROL_PLANE_REQUEST_TIMEOUT,
    MAX_PUBLISH_AUTHORIZATION_BYTES, OWNER_PUBLISH_CONTENT_TYPE, RELAY_LOCATOR_PUBLISH_PATH,
    SIGNED_LOCATOR_CONTENT_TYPE,
};
pub use issuer::{
    RelayLocatorIssuer, RelayLocatorSigner, SignedLocatorResponse, TicketVerifiedOwnerBinding,
};
pub use pairing_client::PairingClaimError;
pub use pairing_service::{
    PairingCodeStore, PairingPutOutcome, PairingStoreRejection, RelayPairingService,
    RelayPairingServiceError,
};
pub use pairing_wire::{
    PairingClaimRequest, PairingPublishRequest, MAX_PAIRING_CODE_TTL_SECS,
    MAX_PAIRING_PUBLISH_REQUEST_BYTES, MIN_PAIRING_PUBLISH_REQUEST_BYTES,
    PAIRING_CLAIM_CONTENT_TYPE, PAIRING_CLAIM_PATH, PAIRING_CLAIM_REQUEST_BYTES,
    PAIRING_PUBLISH_CONTENT_TYPE, PAIRING_PUBLISH_PATH, SEALED_INVITE_CONTENT_TYPE,
};
pub use publish::{
    OwnerPublishDraft, OwnerPublishRequest, RelayPublishLifetime, SecretInviteCode,
    SignedInviteResponse, MAX_ISSUER_CLOCK_SKEW_SECS, OWNER_PUBLISH_REQUEST_BYTES,
};
pub use service::{
    RegionBoundOwnerPublishPolicy, RelayLocatorPublishService, RelayLocatorPublishServiceError,
    RelayOwnerPublishPolicy, RelayOwnerPublishPolicyContext,
};

#[cfg(test)]
mod tests;
