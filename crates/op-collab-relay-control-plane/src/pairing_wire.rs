//! Bounded wire shapes for the short pairing-code endpoints.
//!
//! Publish stores a sealed invite blob under a `code_id`; claim retrieves it.
//! Both requests carry the caller's device DH public key so the control plane
//! can bind the collaboration ticket exactly the way locator publish does.
//! The sealed blob is opaque to the server: the sealing key derives from the
//! pairing code, which never reaches the control plane.

use op_collab_relay_protocol::{
    RelayProtocolError, MAX_SEALED_INVITE_BYTES, PAIRING_CODE_ID_BYTES, RELAY_PROTOCOL_VERSION,
};

use crate::RelayLocatorIssueError;

pub const PAIRING_PUBLISH_PATH: &str = "/v1/pairing-code";
pub const PAIRING_CLAIM_PATH: &str = "/v1/pairing-code/claim";
pub const PAIRING_PUBLISH_CONTENT_TYPE: &str =
    "application/vnd.openpencil.relay-pairing-publish-v1";
pub const PAIRING_CLAIM_CONTENT_TYPE: &str = "application/vnd.openpencil.relay-pairing-claim-v1";
pub const SEALED_INVITE_CONTENT_TYPE: &str = "application/vnd.openpencil.relay-sealed-invite-v1";

/// Server-side ceiling for a stored code's lifetime. Mirrors the locator
/// pairing window: a code must never outlive the invite it seals.
pub const MAX_PAIRING_CODE_TTL_SECS: u32 = 3_600;

const DEVICE_STATIC_OFFSET: usize = 1;
const CODE_ID_OFFSET: usize = DEVICE_STATIC_OFFSET + 32;
const TTL_OFFSET: usize = CODE_ID_OFFSET + PAIRING_CODE_ID_BYTES;
const SEALED_LEN_OFFSET: usize = TTL_OFFSET + 4;
const SEALED_OFFSET: usize = SEALED_LEN_OFFSET + 2;

pub const PAIRING_CLAIM_REQUEST_BYTES: usize = CODE_ID_OFFSET + PAIRING_CODE_ID_BYTES;
/// Smallest well-formed publish body: full header plus a one-byte blob. HTTP
/// pre-filters use this instead of re-deriving the layout by hand.
pub const MIN_PAIRING_PUBLISH_REQUEST_BYTES: usize = SEALED_OFFSET + 1;
pub const MAX_PAIRING_PUBLISH_REQUEST_BYTES: usize = SEALED_OFFSET + MAX_SEALED_INVITE_BYTES;

/// Owner-side request storing one sealed invite under a code id.
#[derive(Clone, PartialEq, Eq)]
pub struct PairingPublishRequest {
    device_static: [u8; 32],
    code_id: [u8; PAIRING_CODE_ID_BYTES],
    ttl_secs: u32,
    sealed: Vec<u8>,
}

impl PairingPublishRequest {
    pub fn new(
        device_static: [u8; 32],
        code_id: [u8; PAIRING_CODE_ID_BYTES],
        ttl_secs: u32,
        sealed: Vec<u8>,
    ) -> Result<Self, RelayLocatorIssueError> {
        if ttl_secs == 0 {
            return Err(RelayLocatorIssueError::ZeroLifetime);
        }
        if ttl_secs > MAX_PAIRING_CODE_TTL_SECS {
            return Err(RelayLocatorIssueError::LifetimeTooLong {
                actual: u64::from(ttl_secs),
                maximum: u64::from(MAX_PAIRING_CODE_TTL_SECS),
            });
        }
        if sealed.is_empty() || sealed.len() > MAX_SEALED_INVITE_BYTES {
            return Err(RelayLocatorIssueError::Protocol(
                RelayProtocolError::InvalidSealedInvite,
            ));
        }
        Ok(Self {
            device_static,
            code_id,
            ttl_secs,
            sealed,
        })
    }

    pub fn device_static(&self) -> &[u8; 32] {
        &self.device_static
    }

    pub fn code_id(&self) -> &[u8; PAIRING_CODE_ID_BYTES] {
        &self.code_id
    }

    pub fn ttl_secs(&self) -> u32 {
        self.ttl_secs
    }

    pub fn sealed(&self) -> &[u8] {
        &self.sealed
    }

    pub fn encode_binary(&self) -> Vec<u8> {
        let mut raw = Vec::with_capacity(SEALED_OFFSET + self.sealed.len());
        raw.push(RELAY_PROTOCOL_VERSION);
        raw.extend_from_slice(&self.device_static);
        raw.extend_from_slice(&self.code_id);
        raw.extend_from_slice(&self.ttl_secs.to_be_bytes());
        raw.extend_from_slice(&(self.sealed.len() as u16).to_be_bytes());
        raw.extend_from_slice(&self.sealed);
        raw
    }

    pub fn decode_binary(raw: &[u8]) -> Result<Self, RelayLocatorIssueError> {
        if raw.len() < SEALED_OFFSET + 1 || raw.len() > MAX_PAIRING_PUBLISH_REQUEST_BYTES {
            return Err(RelayLocatorIssueError::InvalidRequestLength {
                actual: raw.len(),
                expected: SEALED_OFFSET + 1,
            });
        }
        if raw[0] != RELAY_PROTOCOL_VERSION {
            return Err(RelayLocatorIssueError::UnsupportedRequestVersion {
                actual: raw[0],
                expected: RELAY_PROTOCOL_VERSION,
            });
        }
        let mut device_static = [0_u8; 32];
        device_static.copy_from_slice(&raw[DEVICE_STATIC_OFFSET..CODE_ID_OFFSET]);
        let mut code_id = [0_u8; PAIRING_CODE_ID_BYTES];
        code_id.copy_from_slice(&raw[CODE_ID_OFFSET..TTL_OFFSET]);
        let ttl_secs = u32::from_be_bytes(
            raw[TTL_OFFSET..SEALED_LEN_OFFSET]
                .try_into()
                .expect("fixed ttl range"),
        );
        let sealed_len = u16::from_be_bytes(
            raw[SEALED_LEN_OFFSET..SEALED_OFFSET]
                .try_into()
                .expect("fixed length range"),
        ) as usize;
        if raw.len() != SEALED_OFFSET + sealed_len {
            return Err(RelayLocatorIssueError::InvalidRequestLength {
                actual: raw.len(),
                expected: SEALED_OFFSET + sealed_len,
            });
        }
        Self::new(
            device_static,
            code_id,
            ttl_secs,
            raw[SEALED_OFFSET..].to_vec(),
        )
    }
}

impl std::fmt::Debug for PairingPublishRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PairingPublishRequest([REDACTED])")
    }
}

/// Guest-side request redeeming a code id for the sealed blob.
#[derive(Clone, PartialEq, Eq)]
pub struct PairingClaimRequest {
    device_static: [u8; 32],
    code_id: [u8; PAIRING_CODE_ID_BYTES],
}

impl PairingClaimRequest {
    pub fn new(device_static: [u8; 32], code_id: [u8; PAIRING_CODE_ID_BYTES]) -> Self {
        Self {
            device_static,
            code_id,
        }
    }

    pub fn device_static(&self) -> &[u8; 32] {
        &self.device_static
    }

    pub fn code_id(&self) -> &[u8; PAIRING_CODE_ID_BYTES] {
        &self.code_id
    }

    pub fn encode_binary(&self) -> [u8; PAIRING_CLAIM_REQUEST_BYTES] {
        let mut raw = [0_u8; PAIRING_CLAIM_REQUEST_BYTES];
        raw[0] = RELAY_PROTOCOL_VERSION;
        raw[DEVICE_STATIC_OFFSET..CODE_ID_OFFSET].copy_from_slice(&self.device_static);
        raw[CODE_ID_OFFSET..].copy_from_slice(&self.code_id);
        raw
    }

    pub fn decode_binary(raw: &[u8]) -> Result<Self, RelayLocatorIssueError> {
        if raw.len() != PAIRING_CLAIM_REQUEST_BYTES {
            return Err(RelayLocatorIssueError::InvalidRequestLength {
                actual: raw.len(),
                expected: PAIRING_CLAIM_REQUEST_BYTES,
            });
        }
        if raw[0] != RELAY_PROTOCOL_VERSION {
            return Err(RelayLocatorIssueError::UnsupportedRequestVersion {
                actual: raw[0],
                expected: RELAY_PROTOCOL_VERSION,
            });
        }
        let mut device_static = [0_u8; 32];
        device_static.copy_from_slice(&raw[DEVICE_STATIC_OFFSET..CODE_ID_OFFSET]);
        let mut code_id = [0_u8; PAIRING_CODE_ID_BYTES];
        code_id.copy_from_slice(&raw[CODE_ID_OFFSET..]);
        Ok(Self::new(device_static, code_id))
    }
}

impl std::fmt::Debug for PairingClaimRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PairingClaimRequest([REDACTED])")
    }
}
