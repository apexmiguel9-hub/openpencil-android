use std::{fmt, path::Path, sync::Arc};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use ed25519_dalek::{Signature, VerifyingKey};
use op_auth_bridge::CollabJwksFetchError;
use op_collab_relay_protocol::{LocatorKeyId, RelayLocatorVerifier, MAX_LOCATOR_KEY_ID_BYTES};
use serde::Deserialize;

use crate::read_bounded_regular_file;

pub const PINNED_VERIFIER_KEY_FILE_VERSION: u32 = 1;
pub const MAX_PINNED_VERIFIER_KEY_FILE_BYTES: usize = 64 * 1024;
pub const MAX_PINNED_VERIFIER_KEYS: usize = 64;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PinnedKeyFile {
    version: u32,
    keys: Vec<PinnedKeyRecord>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PinnedKeyRecord {
    kid: String,
    public_key_ed25519: String,
}

struct PinnedEd25519KeySet {
    keys: Vec<(String, VerifyingKey)>,
}

impl PinnedEd25519KeySet {
    fn from_file(path: &Path, maximum_key_id_bytes: usize) -> Result<Self, PinnedVerifierError> {
        let body = read_bounded_regular_file(path, MAX_PINNED_VERIFIER_KEY_FILE_BYTES)
            .map_err(map_file_error)?;
        let file: PinnedKeyFile =
            serde_json::from_slice(&body).map_err(|_| PinnedVerifierError::Malformed)?;
        if file.version != PINNED_VERIFIER_KEY_FILE_VERSION {
            return Err(PinnedVerifierError::UnsupportedVersion);
        }
        if file.keys.is_empty() {
            return Err(PinnedVerifierError::EmptyKeys);
        }
        if file.keys.len() > MAX_PINNED_VERIFIER_KEYS {
            return Err(PinnedVerifierError::TooManyKeys {
                maximum: MAX_PINNED_VERIFIER_KEYS,
            });
        }

        let mut keys = Vec::with_capacity(file.keys.len());
        for record in file.keys {
            let key_id =
                LocatorKeyId::new(&record.kid).map_err(|_| PinnedVerifierError::InvalidKey)?;
            if key_id.as_str().len() > maximum_key_id_bytes {
                return Err(PinnedVerifierError::InvalidKey);
            }
            if keys.iter().any(|(existing, _)| existing == key_id.as_str()) {
                return Err(PinnedVerifierError::DuplicateKey);
            }
            let decoded = URL_SAFE_NO_PAD
                .decode(&record.public_key_ed25519)
                .map_err(|_| PinnedVerifierError::InvalidKey)?;
            if URL_SAFE_NO_PAD.encode(&decoded) != record.public_key_ed25519 {
                return Err(PinnedVerifierError::InvalidKey);
            }
            let key_bytes: [u8; 32] = decoded
                .try_into()
                .map_err(|_| PinnedVerifierError::InvalidKey)?;
            let key = VerifyingKey::from_bytes(&key_bytes)
                .map_err(|_| PinnedVerifierError::InvalidKey)?;
            keys.push((record.kid, key));
        }
        Ok(Self { keys })
    }

    fn verify(&self, key_id: &str, message: &[u8], signature: &[u8; 64]) -> bool {
        let Some((_, key)) = self.keys.iter().find(|(candidate, _)| candidate == key_id) else {
            return false;
        };
        key.verify_strict(message, &Signature::from_bytes(signature))
            .is_ok()
    }
}

impl fmt::Debug for PinnedEd25519KeySet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PinnedEd25519KeySet")
            .field("key_count", &self.keys.len())
            .field("keys", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone)]
pub struct PinnedEd25519LocatorVerifier {
    keys: Arc<PinnedEd25519KeySet>,
}

impl PinnedEd25519LocatorVerifier {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, PinnedVerifierError> {
        Ok(Self {
            keys: Arc::new(PinnedEd25519KeySet::from_file(
                path.as_ref(),
                MAX_LOCATOR_KEY_ID_BYTES,
            )?),
        })
    }
}

impl RelayLocatorVerifier for PinnedEd25519LocatorVerifier {
    fn verify(
        &self,
        key_id: &LocatorKeyId,
        canonical_signing_bytes: &[u8],
        signature: &[u8; 64],
    ) -> bool {
        self.keys
            .verify(key_id.as_str(), canonical_signing_bytes, signature)
    }
}

impl fmt::Debug for PinnedEd25519LocatorVerifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PinnedEd25519LocatorVerifier")
            .field("key_count", &self.keys.keys.len())
            .field("keys", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PinnedVerifierError {
    #[error("pinned verifier key file is unavailable")]
    FileUnavailable,
    #[error("pinned verifier key path is not a regular non-symlink file")]
    FileRejected,
    #[error("pinned verifier key file exceeds {maximum} bytes")]
    FileTooLarge { maximum: usize },
    #[error("pinned verifier key file is malformed")]
    Malformed,
    #[error("pinned verifier key file version is unsupported")]
    UnsupportedVersion,
    #[error("pinned verifier key file contains no keys")]
    EmptyKeys,
    #[error("pinned verifier key file contains more than {maximum} keys")]
    TooManyKeys { maximum: usize },
    #[error("pinned verifier key file contains an invalid key")]
    InvalidKey,
    #[error("pinned verifier key file contains a duplicate key id")]
    DuplicateKey,
}

fn map_file_error(error: CollabJwksFetchError) -> PinnedVerifierError {
    match error {
        CollabJwksFetchError::RejectedResponse => PinnedVerifierError::FileRejected,
        CollabJwksFetchError::ResponseTooLarge => PinnedVerifierError::FileTooLarge {
            maximum: MAX_PINNED_VERIFIER_KEY_FILE_BYTES,
        },
        CollabJwksFetchError::Cancelled | CollabJwksFetchError::Unavailable => {
            PinnedVerifierError::FileUnavailable
        }
    }
}
