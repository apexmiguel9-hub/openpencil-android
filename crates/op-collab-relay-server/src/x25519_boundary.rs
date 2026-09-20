use std::{
    fmt,
    fs::{File, OpenOptions},
    io::{Read, Take},
    path::Path,
    sync::Arc,
};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use op_collab_relay_protocol::{
    CallerDeviceDhPublic, RelayChallengeKeyId, RelayChallengeProofV2, RelayClientHello,
    RelayServerChallengeV1,
};
use serde::Deserialize;
use subtle::ConstantTimeEq as _;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;

pub const RELAY_X25519_KEY_FILE_VERSION: u32 = 1;
pub const MAX_RELAY_X25519_KEY_FILE_BYTES: usize = 16 * 1024;
pub const MAX_RELAY_X25519_KEYS: usize = 8;

/// HSM-capable boundary for the server side of challenge-bound X25519 proof.
///
/// An HSM implementation can keep its private key and the derived shared
/// secret internal: the relay supplies only the public transcript and asks
/// whether the fixed-size proof is valid.
pub trait RelayServerX25519ProofBoundary: Send + Sync {
    fn active_key_id(&self) -> Result<RelayChallengeKeyId, RelayX25519BoundaryError>;

    fn verify(
        &self,
        challenge: &RelayServerChallengeV1,
        caller_public_key: &CallerDeviceDhPublic,
        bearer: &[u8],
        hello: &RelayClientHello,
        proof: &RelayChallengeProofV2,
    ) -> Result<bool, RelayX25519BoundaryError>;
}

/// Sealed-file adapter for deployments that do not yet have an HSM adapter.
///
/// The file is opened without following a final symlink and must have no
/// group/other permission bits on Unix. Private encodings and derived shared
/// secrets are wiped; Debug output exposes only the key count.
#[derive(Clone)]
pub struct PinnedX25519ProofBoundary {
    keys: Arc<PinnedX25519KeySet>,
}

impl PinnedX25519ProofBoundary {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, PinnedX25519KeyError> {
        Ok(Self {
            keys: Arc::new(PinnedX25519KeySet::from_file(path.as_ref())?),
        })
    }
}

impl RelayServerX25519ProofBoundary for PinnedX25519ProofBoundary {
    fn active_key_id(&self) -> Result<RelayChallengeKeyId, RelayX25519BoundaryError> {
        Ok(self.keys.active_key_id.clone())
    }

    fn verify(
        &self,
        challenge: &RelayServerChallengeV1,
        caller_public_key: &CallerDeviceDhPublic,
        bearer: &[u8],
        hello: &RelayClientHello,
        proof: &RelayChallengeProofV2,
    ) -> Result<bool, RelayX25519BoundaryError> {
        let secret = self
            .keys
            .keys
            .iter()
            .find(|key| key.key_id == *challenge.key_id())
            .map(|key| &key.secret)
            .ok_or(RelayX25519BoundaryError::UnknownKey)?;
        let peer = PublicKey::from(*caller_public_key.as_bytes());
        let shared = secret.diffie_hellman(&peer);
        if !shared.was_contributory() {
            return Err(RelayX25519BoundaryError::NonContributoryPeer);
        }
        let shared = Zeroizing::new(shared.to_bytes());
        Ok(proof.verify(&shared, challenge, bearer, hello).is_ok())
    }
}

impl fmt::Debug for PinnedX25519ProofBoundary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PinnedX25519ProofBoundary")
            .field("key_count", &self.keys.keys.len())
            .field("keys", &"[REDACTED]")
            .finish()
    }
}

struct PinnedX25519KeySet {
    active_key_id: RelayChallengeKeyId,
    keys: Vec<PinnedX25519Key>,
}

impl PinnedX25519KeySet {
    fn from_file(path: &Path) -> Result<Self, PinnedX25519KeyError> {
        let body = Zeroizing::new(read_private_key_file(
            path,
            MAX_RELAY_X25519_KEY_FILE_BYTES,
        )?);
        let file: X25519KeyFile =
            serde_json::from_slice(&body).map_err(|_| PinnedX25519KeyError::Malformed)?;
        if file.version != RELAY_X25519_KEY_FILE_VERSION {
            return Err(PinnedX25519KeyError::UnsupportedVersion);
        }
        if file.keys.is_empty() {
            return Err(PinnedX25519KeyError::EmptyKeys);
        }
        if file.keys.len() > MAX_RELAY_X25519_KEYS {
            return Err(PinnedX25519KeyError::TooManyKeys {
                maximum: MAX_RELAY_X25519_KEYS,
            });
        }
        let active_key_id = RelayChallengeKeyId::new(file.active_kid)
            .map_err(|_| PinnedX25519KeyError::InvalidKey)?;
        let mut keys = Vec::with_capacity(file.keys.len());
        for record in file.keys {
            let key_id = RelayChallengeKeyId::new(record.kid)
                .map_err(|_| PinnedX25519KeyError::InvalidKey)?;
            if keys
                .iter()
                .any(|existing: &PinnedX25519Key| existing.key_id == key_id)
            {
                return Err(PinnedX25519KeyError::DuplicateKey);
            }
            let private_encoding = Zeroizing::new(record.private_key_x25519);
            let private = Zeroizing::new(decode_key(&private_encoding)?);
            if private.iter().all(|byte| *byte == 0) {
                return Err(PinnedX25519KeyError::InvalidKey);
            }
            let expected_public = decode_key(&record.public_key_x25519)?;
            if expected_public.iter().all(|byte| *byte == 0) {
                return Err(PinnedX25519KeyError::InvalidKey);
            }
            let secret = StaticSecret::from(*private);
            let actual_public = PublicKey::from(&secret).to_bytes();
            if !bool::from(actual_public.ct_eq(&expected_public)) {
                return Err(PinnedX25519KeyError::InvalidKey);
            }
            keys.push(PinnedX25519Key { key_id, secret });
        }
        if !keys.iter().any(|key| key.key_id == active_key_id) {
            return Err(PinnedX25519KeyError::MissingActiveKey);
        }
        Ok(Self {
            active_key_id,
            keys,
        })
    }
}

struct PinnedX25519Key {
    key_id: RelayChallengeKeyId,
    secret: StaticSecret,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct X25519KeyFile {
    version: u32,
    active_kid: String,
    keys: Vec<X25519KeyRecord>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct X25519KeyRecord {
    kid: String,
    private_key_x25519: String,
    public_key_x25519: String,
}

fn decode_key(value: &str) -> Result<[u8; 32], PinnedX25519KeyError> {
    let decoded = Zeroizing::new(
        URL_SAFE_NO_PAD
            .decode(value)
            .map_err(|_| PinnedX25519KeyError::InvalidKey)?,
    );
    if URL_SAFE_NO_PAD.encode(&*decoded) != value {
        return Err(PinnedX25519KeyError::InvalidKey);
    }
    decoded
        .as_slice()
        .try_into()
        .map_err(|_| PinnedX25519KeyError::InvalidKey)
}

fn read_private_key_file(path: &Path, maximum: usize) -> Result<Vec<u8>, PinnedX25519KeyError> {
    let link_metadata =
        std::fs::symlink_metadata(path).map_err(|_| PinnedX25519KeyError::FileUnavailable)?;
    if link_metadata.file_type().is_symlink() {
        return Err(PinnedX25519KeyError::UnsafeFile);
    }
    let file = open_private_file(path)?;
    let metadata = file
        .metadata()
        .map_err(|_| PinnedX25519KeyError::FileUnavailable)?;
    if !metadata.file_type().is_file() {
        return Err(PinnedX25519KeyError::UnsafeFile);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(PinnedX25519KeyError::UnsafePermissions);
        }
        // Mode bits alone do not establish trust: a 0600 key owned by another
        // local account is readable by that account, not just by this process.
        // Requiring the running user to own it closes that gap. Root is allowed
        // because it can read the file regardless of ownership.
        let owner = metadata.uid();
        // SAFETY: `getuid` is always successful and takes no arguments.
        let current = unsafe { libc::getuid() };
        if owner != current && current != 0 {
            return Err(PinnedX25519KeyError::UnsafeOwner);
        }
    }
    if metadata.len() > maximum as u64 {
        return Err(PinnedX25519KeyError::FileTooLarge { maximum });
    }
    let mut reader = file.take((maximum as u64).saturating_add(1));
    let mut body = Vec::with_capacity(metadata.len() as usize);
    read_all(&mut reader, &mut body)?;
    if body.len() > maximum {
        return Err(PinnedX25519KeyError::FileTooLarge { maximum });
    }
    Ok(body)
}

fn open_private_file(path: &Path) -> Result<File, PinnedX25519KeyError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW);
    }
    options
        .open(path)
        .map_err(|_| PinnedX25519KeyError::FileUnavailable)
}

fn read_all(reader: &mut Take<File>, body: &mut Vec<u8>) -> Result<(), PinnedX25519KeyError> {
    reader
        .read_to_end(body)
        .map(|_| ())
        .map_err(|_| PinnedX25519KeyError::FileUnavailable)
}

#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum RelayX25519BoundaryError {
    #[error("relay X25519 proof boundary is unavailable")]
    Unavailable,
    #[error("relay X25519 proof key id is unknown")]
    UnknownKey,
    #[error("caller X25519 public key is non-contributory")]
    NonContributoryPeer,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PinnedX25519KeyError {
    #[error("relay X25519 key file is unavailable")]
    FileUnavailable,
    #[error("relay X25519 key path is unsafe")]
    UnsafeFile,
    #[error("relay X25519 key file permissions must deny group and other access")]
    UnsafePermissions,
    #[error("relay X25519 key file must be owned by the user running the relay")]
    UnsafeOwner,
    #[error("relay X25519 key file exceeds {maximum} bytes")]
    FileTooLarge { maximum: usize },
    #[error("relay X25519 key file is malformed")]
    Malformed,
    #[error("relay X25519 key file version is unsupported")]
    UnsupportedVersion,
    #[error("relay X25519 key file contains no keys")]
    EmptyKeys,
    #[error("relay X25519 key file contains more than {maximum} keys")]
    TooManyKeys { maximum: usize },
    #[error("relay X25519 key file contains a duplicate key id")]
    DuplicateKey,
    #[error("relay X25519 key file contains an invalid key")]
    InvalidKey,
    #[error("relay X25519 active key id is not present")]
    MissingActiveKey,
}
