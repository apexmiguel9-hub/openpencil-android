use std::{fmt, sync::Arc};

use op_collab_relay_protocol::{
    CallerDeviceDhPublic, RelayAuthExtensionV1, RelayChallengeKeyId, RelayChallengeProofV2,
    RelayClientHello, RelayReauthResponseV1, RelayRole, RelayServerChallengeV1, VerifiedRelayRoute,
    MAX_RELAY_BEARER_BYTES,
};
use thiserror::Error;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use zeroize::Zeroizing;

const BEARER_PREFIX_BYTES: usize = 7;
pub const MAX_PINNED_RELAY_X25519_KEYS: usize = 32;

/// A short-lived authorization value returned by a caller-owned provider.
pub struct RelayCredential {
    bearer: Zeroizing<String>,
}

impl RelayCredential {
    pub fn bearer(secret: impl Into<String>) -> Result<Self, RelayAuthError> {
        let secret = Zeroizing::new(secret.into());
        if secret.is_empty() || secret.len() > MAX_RELAY_BEARER_BYTES || !valid_b64token(&secret) {
            return Err(RelayAuthError::InvalidCredential);
        }
        Ok(Self { bearer: secret })
    }

    pub(crate) fn authorization_header(&self) -> Result<HeaderValue, RelayAuthError> {
        let mut value = Zeroizing::new(String::with_capacity(
            BEARER_PREFIX_BYTES + self.bearer.len(),
        ));
        value.push_str("Bearer ");
        value.push_str(&self.bearer);
        HeaderValue::from_str(&value).map_err(|_| RelayAuthError::InvalidCredential)
    }

    pub(crate) fn bearer_bytes(&self) -> &[u8] {
        self.bearer.as_bytes()
    }
}

fn valid_b64token(value: &str) -> bool {
    let mut padding = false;
    for byte in value.bytes() {
        if byte == b'=' {
            padding = true;
        } else if padding
            || !(byte.is_ascii_alphanumeric()
                || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'+' | b'/'))
        {
            return false;
        }
    }
    !value.starts_with('=')
}

impl fmt::Debug for RelayCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RelayCredential")
            .field("bearer", &"[REDACTED]")
            .finish()
    }
}

/// Supplies one fresh bearer for every relay connection or reauthentication.
pub trait RelayCredentialProvider: Send + Sync + 'static {
    fn credential(&self) -> Result<RelayCredential, RelayAuthError>;
}

impl<F> RelayCredentialProvider for F
where
    F: Fn() -> Result<RelayCredential, RelayAuthError> + Send + Sync + 'static,
{
    fn credential(&self) -> Result<RelayCredential, RelayAuthError> {
        self()
    }
}

/// Device-key agreement boundary used by the challenge proof generator.
///
/// Implementations may delegate to an OS key store or hardware-backed key.
/// They expose the public key and a zeroizing shared secret, never the device
/// private key.
pub trait RelayClientX25519Agreement: Send + Sync + 'static {
    fn caller_public_key(&self) -> Result<CallerDeviceDhPublic, RelayAuthError>;

    fn agree(&self, relay_public_key: &[u8; 32]) -> Result<Zeroizing<[u8; 32]>, RelayAuthError>;
}

#[derive(Clone)]
pub struct RelayServerX25519PublicKey {
    key_id: RelayChallengeKeyId,
    public_key: [u8; 32],
}

impl RelayServerX25519PublicKey {
    pub fn new(key_id: RelayChallengeKeyId, public_key: [u8; 32]) -> Result<Self, RelayAuthError> {
        if public_key.iter().all(|byte| *byte == 0) {
            return Err(RelayAuthError::InvalidPinnedRelayKey);
        }
        Ok(Self { key_id, public_key })
    }
}

impl fmt::Debug for RelayServerX25519PublicKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RelayServerX25519PublicKey([REDACTED])")
    }
}

/// Bounded relay public keys pinned by the application/control plane.
pub struct PinnedRelayX25519Keys {
    keys: Vec<RelayServerX25519PublicKey>,
}

impl PinnedRelayX25519Keys {
    pub fn new(
        keys: impl IntoIterator<Item = RelayServerX25519PublicKey>,
    ) -> Result<Self, RelayAuthError> {
        let keys: Vec<_> = keys.into_iter().collect();
        if keys.is_empty() || keys.len() > MAX_PINNED_RELAY_X25519_KEYS {
            return Err(RelayAuthError::InvalidPinnedRelayKey);
        }
        for (index, key) in keys.iter().enumerate() {
            if keys[..index]
                .iter()
                .any(|candidate| candidate.key_id == key.key_id)
            {
                return Err(RelayAuthError::DuplicatePinnedRelayKey);
            }
        }
        Ok(Self { keys })
    }

    fn public_key(&self, key_id: &RelayChallengeKeyId) -> Option<&[u8; 32]> {
        self.keys
            .iter()
            .find(|candidate| candidate.key_id == *key_id)
            .map(|key| &key.public_key)
    }
}

impl fmt::Debug for PinnedRelayX25519Keys {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PinnedRelayX25519Keys")
            .field("key_count", &self.keys.len())
            .field("keys", &"[REDACTED]")
            .finish()
    }
}

/// Supplies a consumed authentication attempt for one connection.
///
/// The returned object owns the exact bearer that is placed on the upgrade
/// request. The same object must then consume the relay challenge and create
/// the hello proof, preventing a refreshed bearer from being paired with a
/// stale proof.
pub trait RelayAuthenticator: Send + Sync + 'static {
    fn begin_attempt(&self) -> Result<RelayAuthAttempt, RelayAuthError>;
}

pub struct ChallengeBoundRelayAuthenticator {
    credentials: Arc<dyn RelayCredentialProvider>,
    key_agreement: Arc<dyn RelayClientX25519Agreement>,
    relay_keys: Arc<PinnedRelayX25519Keys>,
}

impl ChallengeBoundRelayAuthenticator {
    pub fn new(
        credentials: Arc<dyn RelayCredentialProvider>,
        key_agreement: Arc<dyn RelayClientX25519Agreement>,
        relay_keys: Arc<PinnedRelayX25519Keys>,
    ) -> Self {
        Self {
            credentials,
            key_agreement,
            relay_keys,
        }
    }
}

impl RelayAuthenticator for ChallengeBoundRelayAuthenticator {
    fn begin_attempt(&self) -> Result<RelayAuthAttempt, RelayAuthError> {
        let credential = self.credentials.credential()?;
        let caller_public_key = self.key_agreement.caller_public_key()?;
        Ok(RelayAuthAttempt {
            credential,
            caller_public_key,
            key_agreement: Arc::clone(&self.key_agreement),
            relay_keys: Arc::clone(&self.relay_keys),
        })
    }
}

impl fmt::Debug for ChallengeBoundRelayAuthenticator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ChallengeBoundRelayAuthenticator([REDACTED])")
    }
}

/// One non-cloneable bearer and device-key proof attempt.
pub struct RelayAuthAttempt {
    credential: RelayCredential,
    caller_public_key: CallerDeviceDhPublic,
    key_agreement: Arc<dyn RelayClientX25519Agreement>,
    relay_keys: Arc<PinnedRelayX25519Keys>,
}

impl RelayAuthAttempt {
    pub fn new(
        credential: RelayCredential,
        key_agreement: Arc<dyn RelayClientX25519Agreement>,
        relay_keys: Arc<PinnedRelayX25519Keys>,
    ) -> Result<Self, RelayAuthError> {
        let caller_public_key = key_agreement.caller_public_key()?;
        Ok(Self {
            credential,
            caller_public_key,
            key_agreement,
            relay_keys,
        })
    }

    pub(crate) fn authorization_header(&self) -> Result<HeaderValue, RelayAuthError> {
        self.credential.authorization_header()
    }

    pub fn finish(
        self,
        challenge: &RelayServerChallengeV1,
        role: RelayRole,
        route: &VerifiedRelayRoute,
    ) -> Result<RelayAttemptResponse, RelayAuthError> {
        let relay_public_key = self
            .relay_keys
            .public_key(challenge.key_id())
            .ok_or(RelayAuthError::UnknownRelayKey)?;
        let shared_secret = self.key_agreement.agree(relay_public_key)?;
        let template = RelayClientHello::new_challenge_bound_v2(
            role,
            route,
            RelayAuthExtensionV1::without_possession_proof(self.caller_public_key),
        )
        .map_err(|_| RelayAuthError::ProofGeneration)?;
        let proof = RelayChallengeProofV2::derive(
            &shared_secret,
            challenge,
            self.credential.bearer_bytes(),
            &template,
        )
        .map_err(|_| RelayAuthError::ProofGeneration)?;
        let auth =
            RelayAuthExtensionV1::new(self.caller_public_key, Some(proof.as_bytes().to_vec()))
                .map_err(|_| RelayAuthError::ProofGeneration)?;
        let hello = RelayClientHello::new_challenge_bound_v2(role, route, auth)
            .map_err(|_| RelayAuthError::ProofGeneration)?;
        Ok(RelayAttemptResponse {
            credential: self.credential,
            hello,
        })
    }
}

impl fmt::Debug for RelayAuthAttempt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RelayAuthAttempt([REDACTED])")
    }
}

/// Completed response retained only until its hello/control frame is sent.
pub struct RelayAttemptResponse {
    credential: RelayCredential,
    hello: RelayClientHello,
}

impl RelayAttemptResponse {
    pub(crate) fn into_hello(self) -> RelayClientHello {
        drop(self.credential);
        self.hello
    }

    pub(crate) fn into_reauth_response(
        self,
        challenge: RelayServerChallengeV1,
    ) -> Result<RelayReauthResponseV1, RelayAuthError> {
        RelayReauthResponseV1::new(challenge, self.credential.bearer_bytes(), self.hello)
            .map_err(|_| RelayAuthError::ProofGeneration)
    }
}

impl fmt::Debug for RelayAttemptResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RelayAttemptResponse([REDACTED])")
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RelayAuthError {
    #[error("relay authentication material is unavailable")]
    Unavailable,
    #[error("relay credential is invalid")]
    InvalidCredential,
    #[error("pinned relay X25519 key configuration is invalid")]
    InvalidPinnedRelayKey,
    #[error("pinned relay X25519 key id is duplicated")]
    DuplicatePinnedRelayKey,
    #[error("relay challenge selected an unknown pinned key")]
    UnknownRelayKey,
    #[error("device X25519 agreement failed")]
    KeyAgreement,
    #[error("relay challenge proof generation failed")]
    ProofGeneration,
}

pub(crate) enum AuthMode {
    ChallengeBound(Arc<dyn RelayAuthenticator>),
    TicketBindingOnly(Arc<dyn RelayCredentialProvider>),
    #[cfg(any(test, debug_assertions))]
    DevelopmentAnonymous,
}

impl Clone for AuthMode {
    fn clone(&self) -> Self {
        match self {
            Self::ChallengeBound(authenticator) => Self::ChallengeBound(Arc::clone(authenticator)),
            Self::TicketBindingOnly(provider) => Self::TicketBindingOnly(Arc::clone(provider)),
            #[cfg(any(test, debug_assertions))]
            Self::DevelopmentAnonymous => Self::DevelopmentAnonymous,
        }
    }
}

impl fmt::Debug for AuthMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ChallengeBound(_) => formatter.write_str("AuthMode::ChallengeBound([REDACTED])"),
            Self::TicketBindingOnly(_) => {
                formatter.write_str("AuthMode::TicketBindingOnly([REDACTED])")
            }
            #[cfg(any(test, debug_assertions))]
            Self::DevelopmentAnonymous => formatter.write_str("AuthMode::DevelopmentAnonymous"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_reject_header_injection_and_redact_debug() {
        assert_eq!(
            RelayCredential::bearer("abc\r\nx-leak: yes").unwrap_err(),
            RelayAuthError::InvalidCredential
        );
        assert_eq!(
            RelayCredential::bearer("abc,def").unwrap_err(),
            RelayAuthError::InvalidCredential
        );
        assert_eq!(
            RelayCredential::bearer("abc=def").unwrap_err(),
            RelayAuthError::InvalidCredential
        );
        let credential = RelayCredential::bearer("top-secret").unwrap();
        let debug = format!("{credential:?}");
        assert!(!debug.contains("top-secret"));
        assert!(debug.contains("[REDACTED]"));
    }

    #[test]
    fn credential_accepts_the_authentication_bridge_ticket_ceiling() {
        let credential = RelayCredential::bearer("a".repeat(MAX_RELAY_BEARER_BYTES)).unwrap();
        assert_eq!(credential.bearer_bytes().len(), MAX_RELAY_BEARER_BYTES);
        assert_eq!(
            credential.authorization_header().unwrap().as_bytes().len(),
            BEARER_PREFIX_BYTES + MAX_RELAY_BEARER_BYTES
        );
        assert_eq!(
            RelayCredential::bearer("a".repeat(MAX_RELAY_BEARER_BYTES + 1)).unwrap_err(),
            RelayAuthError::InvalidCredential
        );
    }

    #[test]
    fn pinned_keys_are_bounded_and_unique() {
        let key_id = RelayChallengeKeyId::new("relay-key").unwrap();
        let key = RelayServerX25519PublicKey::new(key_id.clone(), [7; 32]).unwrap();
        assert!(PinnedRelayX25519Keys::new([key.clone()]).is_ok());
        assert_eq!(
            PinnedRelayX25519Keys::new([key.clone(), key]).unwrap_err(),
            RelayAuthError::DuplicatePinnedRelayKey
        );
        assert_eq!(
            RelayServerX25519PublicKey::new(key_id, [0; 32]).unwrap_err(),
            RelayAuthError::InvalidPinnedRelayKey
        );
    }
}
