//! Safe, bounded file-backed key source for signed collaboration policies.

use std::{
    fmt,
    fs::{File, Metadata, OpenOptions},
    io::{Read, Take},
    num::NonZeroU64,
    path::{Path, PathBuf},
};

use op_auth_bridge::{
    CollabJwksFetchError, CollabJwksFetchRequest, CollabJwksFetchResponse, CollabJwksFetcher,
    CollabVerifierConfig,
};

mod pinned_locator_keys;

pub use pinned_locator_keys::{
    PinnedEd25519LocatorVerifier, PinnedVerifierError, MAX_PINNED_VERIFIER_KEYS,
    MAX_PINNED_VERIFIER_KEY_FILE_BYTES, PINNED_VERIFIER_KEY_FILE_VERSION,
};

const FILE_ETAG_CONTEXT: &str = "openpencil/op-collab-policy-file/pinned-policy-file-etag/v1";

/// Typed rejection reasons for a policy file used as a trust root.
///
/// Each variant names one distinct reason the file was refused so callers and
/// operators can tell an unreadable path apart from an unsafe one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyFileTrustError {
    /// The path could not be inspected, opened, or read.
    Unavailable,
    /// The final path component is a symbolic link.
    Symlink,
    /// The opened object is not a regular file.
    NotRegularFile,
    /// The object that was opened is not the object that was inspected before
    /// the open — the path was swapped between the two syscalls.
    OpenedObjectChanged,
    /// The file is writable by its group or by other users, so it is not a
    /// trustworthy source of verification keys.
    GroupOrWorldWritable,
    /// The file is owned by neither root nor the user running this process.
    ForeignOwner,
    /// The file is larger than the caller's maximum.
    TooLarge,
}

impl fmt::Display for PolicyFileTrustError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Unavailable => "policy file is unavailable",
            Self::Symlink => "policy file path is a symbolic link",
            Self::NotRegularFile => "policy file is not a regular file",
            Self::OpenedObjectChanged => "policy file changed between inspection and open",
            Self::GroupOrWorldWritable => "policy file is group- or world-writable",
            Self::ForeignOwner => "policy file is owned by neither root nor the running user",
            Self::TooLarge => "policy file is larger than the requested maximum",
        })
    }
}

impl std::error::Error for PolicyFileTrustError {}

impl From<PolicyFileTrustError> for CollabJwksFetchError {
    fn from(error: PolicyFileTrustError) -> Self {
        match error {
            PolicyFileTrustError::Unavailable => Self::Unavailable,
            PolicyFileTrustError::TooLarge => Self::ResponseTooLarge,
            PolicyFileTrustError::Symlink
            | PolicyFileTrustError::NotRegularFile
            | PolicyFileTrustError::OpenedObjectChanged
            | PolicyFileTrustError::GroupOrWorldWritable
            | PolicyFileTrustError::ForeignOwner => Self::RejectedResponse,
        }
    }
}

/// Reads one operator-pinned signed policy file without following a final
/// symlink.
///
/// The configured endpoint is compared byte-for-byte with each verifier
/// request. File metadata and reads are bounded by the verifier's requested
/// maximum. See [`read_bounded_trust_root_file`] for the exact trust-root
/// checks, including the reduced guarantee on non-Unix platforms.
pub struct PinnedPolicyFileFetcher {
    endpoint: String,
    path: PathBuf,
    max_age_seconds: NonZeroU64,
}

impl PinnedPolicyFileFetcher {
    pub fn new(
        verifier_config: &CollabVerifierConfig,
        path: impl Into<PathBuf>,
        max_age_seconds: NonZeroU64,
    ) -> Self {
        Self {
            endpoint: verifier_config.keyset_endpoint().to_owned(),
            path: path.into(),
            max_age_seconds,
        }
    }

    pub fn validate_source(&self, maximum_body_bytes: usize) -> Result<(), CollabJwksFetchError> {
        read_bounded_regular_file(&self.path, maximum_body_bytes).map(|_| ())
    }
}

impl CollabJwksFetcher for PinnedPolicyFileFetcher {
    fn fetch(
        &self,
        request: CollabJwksFetchRequest<'_>,
    ) -> Result<CollabJwksFetchResponse, CollabJwksFetchError> {
        if request.endpoint != self.endpoint {
            return Err(CollabJwksFetchError::RejectedResponse);
        }
        let body = read_bounded_regular_file(&self.path, request.maximum_body_bytes)?;
        let etag = file_etag(&body);
        if request.etag == Some(etag.as_str()) {
            return Ok(CollabJwksFetchResponse::NotModified {
                etag: Some(etag),
                max_age_seconds: self.max_age_seconds.get(),
            });
        }
        Ok(CollabJwksFetchResponse::Modified {
            body,
            etag: Some(etag),
            max_age_seconds: self.max_age_seconds.get(),
        })
    }
}

impl fmt::Debug for PinnedPolicyFileFetcher {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PinnedPolicyFileFetcher")
            .field("endpoint", &"[REDACTED]")
            .field("path", &"[REDACTED]")
            .field("max_age_seconds", &self.max_age_seconds)
            .finish()
    }
}

/// [`read_bounded_trust_root_file`] with the rejection reason collapsed into
/// the verifier's fetch error, for callers that speak `CollabJwksFetchError`.
pub fn read_bounded_regular_file(
    path: &Path,
    maximum: usize,
) -> Result<Vec<u8>, CollabJwksFetchError> {
    read_bounded_trust_root_file(path, maximum).map_err(CollabJwksFetchError::from)
}

/// Reads a bounded trust-root file, rejecting every unsafe source shape.
///
/// On Unix the open uses `O_NOFOLLOW | O_CLOEXEC`, and the opened descriptor's
/// `st_dev` / `st_ino` are compared with the pre-open `symlink_metadata` so a
/// path swapped between the two syscalls is refused instead of trusted. The
/// file must additionally be owned by root or by the running user and must not
/// be writable by its group or by other users, because anyone who can rewrite
/// it can replace the verification keys. This is public policy material, so a
/// root-owned `0440` or `0444` file is safe when the process can read it.
///
/// **Non-Unix platforms provide a strictly weaker guarantee.** There is no
/// `O_NOFOLLOW` equivalent applied here, no stable device/inode identity to
/// re-check after the open, and no ownership or permission model that maps
/// onto the Unix checks. The symlink test therefore remains a plain
/// check-then-open, and a same-privilege attacker who can replace the path
/// between the two syscalls is not detected. Deployments that need the full
/// guarantee must run the policy-file source on Unix.
pub fn read_bounded_trust_root_file(
    path: &Path,
    maximum: usize,
) -> Result<Vec<u8>, PolicyFileTrustError> {
    let link_metadata =
        std::fs::symlink_metadata(path).map_err(|_| PolicyFileTrustError::Unavailable)?;
    if link_metadata.file_type().is_symlink() {
        return Err(PolicyFileTrustError::Symlink);
    }

    let file = open_no_follow(path)?;
    let metadata = file
        .metadata()
        .map_err(|_| PolicyFileTrustError::Unavailable)?;
    if !metadata.file_type().is_file() {
        return Err(PolicyFileTrustError::NotRegularFile);
    }
    verify_trust_root_source(&link_metadata, &metadata)?;
    if metadata.len() > maximum as u64 {
        return Err(PolicyFileTrustError::TooLarge);
    }

    let mut reader = file.take((maximum as u64).saturating_add(1));
    let mut body = Vec::with_capacity(metadata.len() as usize);
    read_all(&mut reader, &mut body)?;
    if body.len() > maximum {
        return Err(PolicyFileTrustError::TooLarge);
    }
    Ok(body)
}

/// Closes the check-then-open window and enforces trust-root ownership and
/// permissions against the descriptor that was actually opened.
#[cfg(unix)]
fn verify_trust_root_source(
    link_metadata: &Metadata,
    opened_metadata: &Metadata,
) -> Result<(), PolicyFileTrustError> {
    use std::os::unix::fs::MetadataExt as _;

    // Identity is compared on the opened descriptor, not on a second path
    // lookup, so a rename/symlink swap between the two syscalls is detected
    // rather than merely made unlikely.
    if link_metadata.dev() != opened_metadata.dev() || link_metadata.ino() != opened_metadata.ino()
    {
        return Err(PolicyFileTrustError::OpenedObjectChanged);
    }
    // SAFETY: `geteuid` takes no arguments, mutates no state, and cannot fail.
    verify_unix_owner_and_mode(opened_metadata.uid(), opened_metadata.mode(), unsafe {
        libc::geteuid()
    })
}

#[cfg(unix)]
fn verify_unix_owner_and_mode(
    file_uid: libc::uid_t,
    file_mode: u32,
    effective_uid: libc::uid_t,
) -> Result<(), PolicyFileTrustError> {
    if file_mode & 0o022 != 0 {
        return Err(PolicyFileTrustError::GroupOrWorldWritable);
    }
    if file_uid != effective_uid && file_uid != 0 {
        return Err(PolicyFileTrustError::ForeignOwner);
    }
    Ok(())
}

/// Non-Unix builds have no device/inode identity, ownership, or writability
/// model to check here; see [`read_bounded_trust_root_file`] for the reduced
/// guarantee this leaves in place.
#[cfg(not(unix))]
fn verify_trust_root_source(
    _link_metadata: &Metadata,
    _opened_metadata: &Metadata,
) -> Result<(), PolicyFileTrustError> {
    Ok(())
}

fn open_no_follow(path: &Path) -> Result<File, PolicyFileTrustError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW);
    }
    options
        .open(path)
        .map_err(|_| PolicyFileTrustError::Unavailable)
}

fn read_all(reader: &mut Take<File>, body: &mut Vec<u8>) -> Result<(), PolicyFileTrustError> {
    reader
        .read_to_end(body)
        .map(|_| ())
        .map_err(|_| PolicyFileTrustError::Unavailable)
}

fn file_etag(body: &[u8]) -> String {
    let mut hasher = blake3::Hasher::new_derive_key(FILE_ETAG_CONTEXT);
    hasher.update(body);
    format!("\"{}\"", hasher.finalize().to_hex())
}

#[cfg(test)]
mod tests;
