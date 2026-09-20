use std::fmt;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use zeroize::Zeroizing;

use crate::locator::require_exact;
use crate::{
    RelayClientHello, RelayHelloAuthMode, RelayProtocolError, RelayServerChallengeV1,
    MAX_RELAY_CHALLENGE_HEADER_BYTES, RELAY_CHALLENGE_PROOF_V2_BYTES, RELAY_CLIENT_HELLO_BYTES,
};

pub const RELAY_REAUTH_VERSION: u8 = 1;
pub const RELAY_REAUTH_CHALLENGE_PREFIX: &str = "oprrc1_";
pub const RELAY_REAUTH_RESPONSE_PREFIX: &str = "oprrr1_";
/// Relay bearers share the authentication bridge's opaque ticket ceiling.
///
/// Keep this literal here so the wire protocol remains dependency-free; the
/// cross-crate contract tests pin it to `op_auth_bridge::MAX_COLLAB_TICKET_BYTES`.
pub const MAX_RELAY_BEARER_BYTES: usize = 48 * 1024;
pub const MAX_RELAY_REAUTH_CHALLENGE_TEXT_BYTES: usize =
    RELAY_REAUTH_CHALLENGE_PREFIX.len() + MAX_RELAY_CHALLENGE_HEADER_BYTES;

const RESPONSE_HEADER_BYTES: usize = 4;
const MAX_RESPONSE_BINARY_BYTES: usize = RESPONSE_HEADER_BYTES
    + MAX_RELAY_CHALLENGE_HEADER_BYTES
    + MAX_RELAY_BEARER_BYTES
    + RELAY_CLIENT_HELLO_BYTES;
pub const MAX_RELAY_REAUTH_RESPONSE_TEXT_BYTES: usize =
    RELAY_REAUTH_RESPONSE_PREFIX.len() + base64url_no_pad_len(MAX_RESPONSE_BINARY_BYTES);

const fn base64url_no_pad_len(bytes: usize) -> usize {
    (bytes * 4).div_ceil(3)
}

/// A fresh, server-originated online reauthentication challenge.
#[derive(PartialEq, Eq)]
pub struct RelayReauthChallengeV1 {
    challenge: RelayServerChallengeV1,
}

impl RelayReauthChallengeV1 {
    pub fn new(challenge: RelayServerChallengeV1) -> Self {
        Self { challenge }
    }

    pub fn challenge(&self) -> &RelayServerChallengeV1 {
        &self.challenge
    }

    pub fn into_challenge(self) -> RelayServerChallengeV1 {
        self.challenge
    }

    pub fn encode_text(&self) -> String {
        let header = self.challenge.encode_header();
        let mut text = String::with_capacity(RELAY_REAUTH_CHALLENGE_PREFIX.len() + header.len());
        text.push_str(RELAY_REAUTH_CHALLENGE_PREFIX);
        text.push_str(&header);
        text
    }

    pub fn decode_text(text: &str) -> Result<Self, RelayProtocolError> {
        if text.len() > MAX_RELAY_REAUTH_CHALLENGE_TEXT_BYTES {
            return Err(RelayProtocolError::ReauthControlTooLong {
                actual: text.len(),
                maximum: MAX_RELAY_REAUTH_CHALLENGE_TEXT_BYTES,
            });
        }
        let header = text
            .strip_prefix(RELAY_REAUTH_CHALLENGE_PREFIX)
            .ok_or(RelayProtocolError::InvalidReauthControlPrefix)?;
        let challenge = RelayServerChallengeV1::decode_header(header)?;
        let value = Self::new(challenge);
        if value.encode_text() != text {
            return Err(RelayProtocolError::InvalidReauthControlEncoding);
        }
        Ok(value)
    }
}

impl fmt::Debug for RelayReauthChallengeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RelayReauthChallengeV1([REDACTED])")
    }
}

/// Canonical, bounded response to one online reauthentication challenge.
///
/// The bearer is retained in zeroizing storage and all debug output is
/// redacted. This type is intentionally not cloneable.
pub struct RelayReauthResponseV1 {
    challenge: RelayServerChallengeV1,
    bearer: Zeroizing<Vec<u8>>,
    hello: RelayClientHello,
}

impl RelayReauthResponseV1 {
    pub fn new(
        challenge: RelayServerChallengeV1,
        bearer: &[u8],
        hello: RelayClientHello,
    ) -> Result<Self, RelayProtocolError> {
        validate_bearer(bearer)?;
        validate_reauth_hello(&hello)?;
        Ok(Self {
            challenge,
            bearer: Zeroizing::new(bearer.to_vec()),
            hello,
        })
    }

    pub fn challenge(&self) -> &RelayServerChallengeV1 {
        &self.challenge
    }

    pub fn bearer(&self) -> &[u8] {
        &self.bearer
    }

    pub fn hello(&self) -> &RelayClientHello {
        &self.hello
    }

    pub fn encode_text(&self) -> Zeroizing<String> {
        let challenge = self.challenge.encode_header();
        let mut binary = Zeroizing::new(Vec::with_capacity(
            RESPONSE_HEADER_BYTES + challenge.len() + self.bearer.len() + RELAY_CLIENT_HELLO_BYTES,
        ));
        binary.push(RELAY_REAUTH_VERSION);
        binary.push(challenge.len() as u8);
        binary.extend_from_slice(&(self.bearer.len() as u16).to_be_bytes());
        binary.extend_from_slice(challenge.as_bytes());
        binary.extend_from_slice(&self.bearer);
        binary.extend_from_slice(&self.hello.encode());

        let mut text = Zeroizing::new(String::with_capacity(
            RELAY_REAUTH_RESPONSE_PREFIX.len() + base64url_no_pad_len(binary.len()),
        ));
        text.push_str(RELAY_REAUTH_RESPONSE_PREFIX);
        URL_SAFE_NO_PAD.encode_string(&binary, &mut text);
        text
    }

    pub fn decode_text(text: &str) -> Result<Self, RelayProtocolError> {
        if text.len() > MAX_RELAY_REAUTH_RESPONSE_TEXT_BYTES {
            return Err(RelayProtocolError::ReauthControlTooLong {
                actual: text.len(),
                maximum: MAX_RELAY_REAUTH_RESPONSE_TEXT_BYTES,
            });
        }
        let encoded = text
            .strip_prefix(RELAY_REAUTH_RESPONSE_PREFIX)
            .ok_or(RelayProtocolError::InvalidReauthControlPrefix)?;
        if encoded.is_empty() || encoded.bytes().any(|byte| byte == b'=') {
            return Err(RelayProtocolError::InvalidReauthControlEncoding);
        }
        let binary = Zeroizing::new(
            URL_SAFE_NO_PAD
                .decode(encoded)
                .map_err(|_| RelayProtocolError::InvalidReauthControlEncoding)?,
        );
        if URL_SAFE_NO_PAD.encode(&binary) != encoded {
            return Err(RelayProtocolError::InvalidReauthControlEncoding);
        }
        if binary.len() < RESPONSE_HEADER_BYTES {
            return Err(RelayProtocolError::Truncated {
                context: "relay reauthentication response",
                expected: RESPONSE_HEADER_BYTES,
                actual: binary.len(),
            });
        }
        if binary[0] != RELAY_REAUTH_VERSION {
            return Err(RelayProtocolError::UnsupportedReauthVersion {
                actual: binary[0],
                expected: RELAY_REAUTH_VERSION,
            });
        }
        let challenge_len = usize::from(binary[1]);
        if challenge_len > MAX_RELAY_CHALLENGE_HEADER_BYTES {
            return Err(RelayProtocolError::ReauthControlFieldTooLong {
                field: "challenge",
                actual: challenge_len,
                maximum: MAX_RELAY_CHALLENGE_HEADER_BYTES,
            });
        }
        let bearer_len = usize::from(u16::from_be_bytes([binary[2], binary[3]]));
        if bearer_len > MAX_RELAY_BEARER_BYTES {
            return Err(RelayProtocolError::ReauthControlFieldTooLong {
                field: "bearer",
                actual: bearer_len,
                maximum: MAX_RELAY_BEARER_BYTES,
            });
        }
        let expected = RESPONSE_HEADER_BYTES
            .checked_add(challenge_len)
            .and_then(|length| length.checked_add(bearer_len))
            .and_then(|length| length.checked_add(RELAY_CLIENT_HELLO_BYTES))
            .ok_or(RelayProtocolError::InvalidReauthControlEncoding)?;
        require_exact("relay reauthentication response", binary.len(), expected)?;

        let challenge_end = RESPONSE_HEADER_BYTES + challenge_len;
        let bearer_end = challenge_end + bearer_len;
        let challenge = std::str::from_utf8(&binary[RESPONSE_HEADER_BYTES..challenge_end])
            .map_err(|_| RelayProtocolError::InvalidReauthControlEncoding)?;
        let challenge = RelayServerChallengeV1::decode_header(challenge)?;
        let bearer = &binary[challenge_end..bearer_end];
        let hello = RelayClientHello::decode(&binary[bearer_end..])?;
        let response = Self::new(challenge, bearer, hello)?;
        if response.encode_text().as_str() != text {
            return Err(RelayProtocolError::InvalidReauthControlEncoding);
        }
        Ok(response)
    }
}

impl fmt::Debug for RelayReauthResponseV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RelayReauthResponseV1([REDACTED])")
    }
}

pub fn is_valid_relay_bearer_token(value: &[u8]) -> bool {
    if value.is_empty() || value.len() > MAX_RELAY_BEARER_BYTES || value.first() == Some(&b'=') {
        return false;
    }
    let mut padding = false;
    value.iter().all(|byte| {
        if *byte == b'=' {
            padding = true;
            return true;
        }
        !padding
            && (byte.is_ascii_alphanumeric()
                || matches!(*byte, b'-' | b'.' | b'_' | b'~' | b'+' | b'/'))
    })
}

fn validate_bearer(bearer: &[u8]) -> Result<(), RelayProtocolError> {
    if bearer.len() > MAX_RELAY_BEARER_BYTES {
        return Err(RelayProtocolError::ReauthControlFieldTooLong {
            field: "bearer",
            actual: bearer.len(),
            maximum: MAX_RELAY_BEARER_BYTES,
        });
    }
    if !is_valid_relay_bearer_token(bearer) {
        return Err(RelayProtocolError::InvalidRelayBearer);
    }
    Ok(())
}

fn validate_reauth_hello(hello: &RelayClientHello) -> Result<(), RelayProtocolError> {
    if hello.auth_mode() != RelayHelloAuthMode::ChallengeBoundX25519V2
        || hello
            .auth_extension()
            .possession_proof()
            .is_none_or(|proof| proof.len() != RELAY_CHALLENGE_PROOF_V2_BYTES)
    {
        return Err(RelayProtocolError::ReauthRequiresChallengeProof);
    }
    Ok(())
}
