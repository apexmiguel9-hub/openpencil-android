//! Bounded TCP-to-WebSocket bridge for OpenPencil collaboration.
//!
//! The relay forwards the bytes produced by the existing inner Noise/TCP
//! transport. This crate does not parse, decrypt, log, or reinterpret them.
//!
//! "Forwards" is not the same as "cannot read". Document frames are inner
//! Noise ciphertext and stay opaque to the relay, but the bearer ticket, the
//! client hello, and the cleartext server prelude that precedes the handshake
//! (it is the Noise prologue, so it cannot be encrypted) are all visible to
//! the relay operator. See the "Relay operator visibility" section of
//! `docs/security/p2p-collaboration-threat-model.md` for exactly what that
//! discloses and which mitigations remain open.

mod auth;
mod bridge;
mod endpoint;
mod error;
mod limits;
mod reauth_budget;
mod session;

pub use auth::{
    ChallengeBoundRelayAuthenticator, PinnedRelayX25519Keys, RelayAuthAttempt, RelayAuthError,
    RelayAuthenticator, RelayClientX25519Agreement, RelayCredential, RelayCredentialProvider,
    RelayServerX25519PublicKey, MAX_PINNED_RELAY_X25519_KEYS,
};
pub use bridge::{RelayGuestBridge, RelayHandshake, RelayOwnerBridge};
pub use endpoint::{RelayEndpoint, RelayEndpointError};
pub use error::{
    RelayBridgePhase, RelayBridgeStatus, RelayClientError, RelayFailureKind, RelayStopError,
};
pub use limits::{
    DEFAULT_OWNER_LANE_COUNT, MAX_OWNER_LANE_COUNT, MAX_RELAY_BINARY_BYTES,
    MAX_RELAY_CONNECTION_BYTES,
};
