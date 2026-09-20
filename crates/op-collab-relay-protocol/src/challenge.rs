use std::fmt;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hkdf::Hkdf;
use hmac::{Hmac, Mac as _};
use sha2::{Digest as _, Sha256};
use zeroize::Zeroizing;

use crate::locator::require_exact;
use crate::{RelayClientHello, RelayHelloAuthMode, RelayProtocolError};

/// HTTP response header carrying a canonical [`RelayServerChallengeV1`].
pub const RELAY_CHALLENGE_HEADER_NAME: &str = "openpencil-relay-challenge";
pub const RELAY_CHALLENGE_PREFIX: &str = "oprc1_";
pub const RELAY_CHALLENGE_VERSION: u8 = 1;
pub const MAX_RELAY_CHALLENGE_KEY_ID_BYTES: usize = 30;
pub const RELAY_CHALLENGE_NONCE_BYTES: usize = 32;
pub const MAX_RELAY_CHALLENGE_HEADER_BYTES: usize = 92;
pub const RELAY_CHALLENGE_PROOF_V2_BYTES: usize = 33;
pub const RELAY_CHALLENGE_PROOF_VERSION: u8 = 2;

const CHALLENGE_FIXED_BYTES: usize = 2 + RELAY_CHALLENGE_NONCE_BYTES;
const PROOF_TAG_BYTES: usize = 32;
const PROOF_BINDING_DOMAIN: &[u8] =
    b"openpencil/op-collab-relay-protocol/challenge-proof-binding/v2\0";
const PROOF_KEY_INFO_DOMAIN: &[u8] =
    b"openpencil/op-collab-relay-protocol/challenge-proof-key/v2\0";

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, PartialEq, Eq)]
/// Bounded public identifier for the relay X25519 key used by a challenge.
pub struct RelayChallengeKeyId(String);

impl RelayChallengeKeyId {
    pub fn new(value: impl Into<String>) -> Result<Self, RelayProtocolError> {
        let value = value.into();
        validate_challenge_key_id(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for RelayChallengeKeyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RelayChallengeKeyId([REDACTED])")
    }
}

#[derive(Clone, PartialEq, Eq)]
/// Fresh relay nonce and X25519 key selection for one authentication attempt.
pub struct RelayServerChallengeV1 {
    key_id: RelayChallengeKeyId,
    nonce: [u8; RELAY_CHALLENGE_NONCE_BYTES],
}

impl RelayServerChallengeV1 {
    pub fn new(
        key_id: RelayChallengeKeyId,
        nonce: [u8; RELAY_CHALLENGE_NONCE_BYTES],
    ) -> Result<Self, RelayProtocolError> {
        if nonce.iter().all(|byte| *byte == 0) {
            return Err(RelayProtocolError::ZeroChallengeNonce);
        }
        Ok(Self { key_id, nonce })
    }

    #[cfg(feature = "random")]
    pub fn generate(key_id: RelayChallengeKeyId) -> Result<Self, RelayProtocolError> {
        let mut nonce = [0_u8; RELAY_CHALLENGE_NONCE_BYTES];
        getrandom::fill(&mut nonce).map_err(|_| RelayProtocolError::RandomUnavailable)?;
        Self::new(key_id, nonce)
    }

    pub fn encode_header(&self) -> String {
        let binary = self.canonical_binary();
        let mut encoded = String::with_capacity(MAX_RELAY_CHALLENGE_HEADER_BYTES);
        encoded.push_str(RELAY_CHALLENGE_PREFIX);
        URL_SAFE_NO_PAD.encode_string(binary, &mut encoded);
        encoded
    }

    pub fn decode_header(value: &str) -> Result<Self, RelayProtocolError> {
        if value.len() > MAX_RELAY_CHALLENGE_HEADER_BYTES {
            return Err(RelayProtocolError::ChallengeHeaderTooLong {
                actual: value.len(),
                maximum: MAX_RELAY_CHALLENGE_HEADER_BYTES,
            });
        }
        let encoded = value
            .strip_prefix(RELAY_CHALLENGE_PREFIX)
            .ok_or(RelayProtocolError::InvalidChallengePrefix)?;
        if encoded.is_empty() || encoded.bytes().any(|byte| byte == b'=') {
            return Err(RelayProtocolError::InvalidChallengeEncoding);
        }
        let binary = URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|_| RelayProtocolError::InvalidChallengeEncoding)?;
        if URL_SAFE_NO_PAD.encode(&binary) != encoded {
            return Err(RelayProtocolError::InvalidChallengeEncoding);
        }
        if binary.len() < 2 {
            return Err(RelayProtocolError::Truncated {
                context: "relay server challenge",
                expected: 2,
                actual: binary.len(),
            });
        }
        if binary[0] != RELAY_CHALLENGE_VERSION {
            return Err(RelayProtocolError::UnsupportedChallengeVersion {
                actual: binary[0],
                expected: RELAY_CHALLENGE_VERSION,
            });
        }
        let key_id_len = usize::from(binary[1]);
        if key_id_len > MAX_RELAY_CHALLENGE_KEY_ID_BYTES {
            return Err(RelayProtocolError::AsciiFieldTooLong {
                field: "challenge_key_id",
                actual: key_id_len,
                maximum: MAX_RELAY_CHALLENGE_KEY_ID_BYTES,
            });
        }
        let expected = CHALLENGE_FIXED_BYTES + key_id_len;
        require_exact("relay server challenge", binary.len(), expected)?;
        let key_id = std::str::from_utf8(&binary[2..2 + key_id_len])
            .map_err(|_| RelayProtocolError::InvalidChallengeEncoding)?;
        let key_id = RelayChallengeKeyId::new(key_id)?;
        let nonce = binary[2 + key_id_len..]
            .try_into()
            .map_err(|_| RelayProtocolError::InvalidChallengeEncoding)?;
        let challenge = Self::new(key_id, nonce)?;
        if challenge.encode_header() != value {
            return Err(RelayProtocolError::InvalidChallengeEncoding);
        }
        Ok(challenge)
    }

    pub fn key_id(&self) -> &RelayChallengeKeyId {
        &self.key_id
    }

    pub const fn nonce(&self) -> &[u8; RELAY_CHALLENGE_NONCE_BYTES] {
        &self.nonce
    }

    fn canonical_binary(&self) -> Vec<u8> {
        let mut binary = Vec::with_capacity(CHALLENGE_FIXED_BYTES + self.key_id.as_str().len());
        binary.push(RELAY_CHALLENGE_VERSION);
        binary.push(self.key_id.as_str().len() as u8);
        binary.extend_from_slice(self.key_id.as_str().as_bytes());
        binary.extend_from_slice(&self.nonce);
        binary
    }
}

impl fmt::Debug for RelayServerChallengeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RelayServerChallengeV1")
            .field("key_id", &"[REDACTED]")
            .field("nonce", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
/// Exact mode-2 proof wire value: a version byte followed by an HMAC tag.
pub struct RelayChallengeProofV2 {
    wire: [u8; RELAY_CHALLENGE_PROOF_V2_BYTES],
}

impl RelayChallengeProofV2 {
    /// Derives a proof from an already validated, nonzero X25519 result.
    ///
    /// `bearer_token` is the exact token value without the `Bearer ` scheme.
    /// The caller remains responsible for wiping its original shared secret.
    pub fn derive(
        shared_secret: &[u8; 32],
        challenge: &RelayServerChallengeV1,
        bearer_token: &[u8],
        hello: &RelayClientHello,
    ) -> Result<Self, RelayProtocolError> {
        let tag: [u8; PROOF_TAG_BYTES] =
            challenge_proof_mac(shared_secret, challenge, bearer_token, hello)?
                .finalize()
                .into_bytes()
                .into();
        let mut wire = [0_u8; RELAY_CHALLENGE_PROOF_V2_BYTES];
        wire[0] = RELAY_CHALLENGE_PROOF_VERSION;
        wire[1..].copy_from_slice(&tag);
        Ok(Self { wire })
    }

    pub fn decode(wire: &[u8]) -> Result<Self, RelayProtocolError> {
        require_exact(
            "relay challenge proof",
            wire.len(),
            RELAY_CHALLENGE_PROOF_V2_BYTES,
        )?;
        if wire[0] != RELAY_CHALLENGE_PROOF_VERSION {
            return Err(RelayProtocolError::UnsupportedChallengeProofVersion {
                actual: wire[0],
                expected: RELAY_CHALLENGE_PROOF_VERSION,
            });
        }
        Ok(Self {
            wire: wire
                .try_into()
                .map_err(|_| RelayProtocolError::InvalidChallengeProof)?,
        })
    }

    pub const fn as_bytes(&self) -> &[u8; RELAY_CHALLENGE_PROOF_V2_BYTES] {
        &self.wire
    }

    /// Verifies the tag in constant time through the `hmac` implementation.
    pub fn verify(
        &self,
        shared_secret: &[u8; 32],
        challenge: &RelayServerChallengeV1,
        bearer_token: &[u8],
        hello: &RelayClientHello,
    ) -> Result<(), RelayProtocolError> {
        challenge_proof_mac(shared_secret, challenge, bearer_token, hello)?
            .verify_slice(&self.wire[1..])
            .map_err(|_| RelayProtocolError::ChallengeProofVerificationFailed)
    }
}

impl fmt::Debug for RelayChallengeProofV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RelayChallengeProofV2([REDACTED])")
    }
}

/// Computes the mode-2 binding from exact bearer-token bytes and a normalized
/// fixed-size hello.
///
/// The proof length and full proof slot are zeroed. Every other hello byte,
/// including its versions, role, caller DH key, route capability, and signed
/// locator, remains bound.
pub fn relay_challenge_proof_binding_digest(
    challenge: &RelayServerChallengeV1,
    bearer_token: &[u8],
    hello: &RelayClientHello,
) -> Result<[u8; 32], RelayProtocolError> {
    if hello.auth_mode() != RelayHelloAuthMode::ChallengeBoundX25519V2 {
        return Err(RelayProtocolError::ChallengeProofRequiresV2AuthMode);
    }

    let challenge_binary = challenge.canonical_binary();
    let bearer_digest: Zeroizing<[u8; 32]> = Zeroizing::new(Sha256::digest(bearer_token).into());
    let normalized_hello = Zeroizing::new(hello.challenge_proof_binding_bytes());
    let mut hasher = Sha256::new();
    hasher.update(PROOF_BINDING_DOMAIN);
    hasher.update((challenge_binary.len() as u16).to_be_bytes());
    hasher.update(&challenge_binary);
    hasher.update(&bearer_digest[..]);
    hasher.update(&normalized_hello[..]);
    Ok(hasher.finalize().into())
}

fn challenge_proof_mac(
    shared_secret: &[u8; 32],
    challenge: &RelayServerChallengeV1,
    bearer_token: &[u8],
    hello: &RelayClientHello,
) -> Result<HmacSha256, RelayProtocolError> {
    if shared_secret.iter().all(|byte| *byte == 0) {
        return Err(RelayProtocolError::NonContributoryX25519SharedSecret);
    }

    let shared_secret = Zeroizing::new(*shared_secret);
    let mut okm = Zeroizing::new([0_u8; 32]);
    let mut info =
        Vec::with_capacity(PROOF_KEY_INFO_DOMAIN.len() + 2 + challenge.key_id().as_str().len());
    info.extend_from_slice(PROOF_KEY_INFO_DOMAIN);
    info.push(RELAY_CHALLENGE_VERSION);
    info.push(challenge.key_id().as_str().len() as u8);
    info.extend_from_slice(challenge.key_id().as_str().as_bytes());
    {
        let hkdf = Hkdf::<Sha256>::new(Some(challenge.nonce()), &*shared_secret);
        hkdf.expand(&info, &mut *okm)
            .map_err(|_| RelayProtocolError::ChallengeProofDerivationFailed)?;
    }

    let binding_digest = Zeroizing::new(relay_challenge_proof_binding_digest(
        challenge,
        bearer_token,
        hello,
    )?);
    let mut mac = HmacSha256::new_from_slice(&*okm)
        .map_err(|_| RelayProtocolError::ChallengeProofDerivationFailed)?;
    mac.update(&*binding_digest);
    Ok(mac)
}

fn validate_challenge_key_id(value: &str) -> Result<(), RelayProtocolError> {
    if value.len() > MAX_RELAY_CHALLENGE_KEY_ID_BYTES {
        return Err(RelayProtocolError::AsciiFieldTooLong {
            field: "challenge_key_id",
            actual: value.len(),
            maximum: MAX_RELAY_CHALLENGE_KEY_ID_BYTES,
        });
    }
    if value.is_empty() || !value.bytes().all(|byte| (0x21..=0x7e).contains(&byte)) {
        return Err(RelayProtocolError::InvalidAsciiField {
            field: "challenge_key_id",
        });
    }
    Ok(())
}
