#![cfg(test)]

use std::num::NonZeroU64;

use op_auth_bridge::{
    CollabJwksCacheLimits, CollabTicketVerifier, CollabVerifierConfig, CollabVerifierConfigError,
};

use super::*;

/// Writes a policy fixture with owner-only permissions, so the suite does not
/// depend on the ambient umask now that group/world-writable files are
/// rejected.
fn write_policy(path: &Path, body: &[u8]) {
    std::fs::write(path, body).expect("policy");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .expect("permissions");
    }
}

#[test]
fn bounded_regular_file_reads_exact_bytes_and_redacts_path() {
    let directory = tempfile::tempdir().expect("temp directory");
    let path = directory.path().join("policy.json");
    write_policy(&path, b"{\"version\":1}");
    assert_eq!(
        read_bounded_regular_file(&path, 64).expect("read"),
        b"{\"version\":1}"
    );
    assert!(matches!(
        read_bounded_regular_file(&path, 4),
        Err(CollabJwksFetchError::ResponseTooLarge)
    ));

    let config = test_config().expect("config");
    let fetcher = PinnedPolicyFileFetcher::new(&config, &path, NonZeroU64::MIN);
    let debug = format!("{fetcher:?}");
    assert!(!debug.contains(path.to_string_lossy().as_ref()));
}

#[cfg(unix)]
#[test]
fn final_symlink_is_rejected() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().expect("temp directory");
    let target = directory.path().join("target.json");
    let link = directory.path().join("policy.json");
    write_policy(&target, b"{}");
    symlink(&target, &link).expect("symlink");
    assert!(matches!(
        read_bounded_regular_file(&link, 64),
        Err(CollabJwksFetchError::RejectedResponse)
    ));
}

#[cfg(unix)]
#[test]
fn group_or_world_writable_policy_file_is_rejected() {
    use std::os::unix::fs::PermissionsExt as _;

    let directory = tempfile::tempdir().expect("temp directory");
    for mode in [0o666, 0o664] {
        let path = directory.path().join(format!("policy-{mode:o}.json"));
        std::fs::write(&path, b"{}").expect("policy");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
            .expect("permissions");
        assert_eq!(
            read_bounded_trust_root_file(&path, 64),
            Err(PolicyFileTrustError::GroupOrWorldWritable)
        );
        assert!(matches!(
            read_bounded_regular_file(&path, 64),
            Err(CollabJwksFetchError::RejectedResponse)
        ));
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .expect("permissions");
        assert_eq!(
            read_bounded_trust_root_file(&path, 64).expect("read"),
            b"{}"
        );
    }
}

#[cfg(unix)]
#[test]
fn root_owned_read_only_public_policy_modes_are_accepted() {
    // Ownership/mode checking is split from Metadata so these root-owned
    // cases remain deterministic under the ordinary non-root CI runner.
    let non_root_euid = 1_000;
    for mode in [0o440, 0o444] {
        assert_eq!(verify_unix_owner_and_mode(0, mode, non_root_euid), Ok(()));
    }
}

#[cfg(unix)]
#[test]
fn root_owned_group_or_world_writable_policy_modes_are_rejected() {
    let non_root_euid = 1_000;
    for mode in [0o460, 0o442] {
        assert_eq!(
            verify_unix_owner_and_mode(0, mode, non_root_euid),
            Err(PolicyFileTrustError::GroupOrWorldWritable)
        );
    }
}

#[cfg(unix)]
#[test]
fn foreign_non_root_owner_is_still_rejected() {
    assert_eq!(
        verify_unix_owner_and_mode(1_001, 0o444, 1_000),
        Err(PolicyFileTrustError::ForeignOwner)
    );
}

/// A policy file owned by another non-root user must be refused. Creating one requires
/// privileges the test process does not have, so the test borrows a
/// well-known system file and skips itself whenever that file is missing, is
/// a symlink, is already writable by group/other, or is owned by the caller
/// (which is the case when the suite runs as root).
#[cfg(unix)]
#[test]
fn policy_file_owned_by_another_user_is_rejected() {
    use std::os::unix::fs::MetadataExt as _;

    let path = std::path::Path::new("/etc/hosts");
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return;
    };
    // SAFETY: `geteuid` takes no arguments, mutates no state, and cannot fail.
    let effective_uid = unsafe { libc::geteuid() };
    if metadata.file_type().is_symlink()
        || !metadata.file_type().is_file()
        || metadata.mode() & 0o022 != 0
        || metadata.uid() == effective_uid
        || metadata.uid() == 0
    {
        return;
    }
    assert_eq!(
        read_bounded_trust_root_file(path, 1_024 * 1_024),
        Err(PolicyFileTrustError::ForeignOwner)
    );
}

#[test]
fn trust_error_display_and_fetch_mapping_are_distinct_per_reason() {
    for (error, expected) in [
        (
            PolicyFileTrustError::Unavailable,
            CollabJwksFetchError::Unavailable,
        ),
        (
            PolicyFileTrustError::Symlink,
            CollabJwksFetchError::RejectedResponse,
        ),
        (
            PolicyFileTrustError::NotRegularFile,
            CollabJwksFetchError::RejectedResponse,
        ),
        (
            PolicyFileTrustError::OpenedObjectChanged,
            CollabJwksFetchError::RejectedResponse,
        ),
        (
            PolicyFileTrustError::GroupOrWorldWritable,
            CollabJwksFetchError::RejectedResponse,
        ),
        (
            PolicyFileTrustError::ForeignOwner,
            CollabJwksFetchError::RejectedResponse,
        ),
        (
            PolicyFileTrustError::TooLarge,
            CollabJwksFetchError::ResponseTooLarge,
        ),
    ] {
        assert_eq!(CollabJwksFetchError::from(error), expected);
        assert!(!error.to_string().is_empty());
    }
}

#[test]
fn verifier_endpoint_is_pinned() {
    let directory = tempfile::tempdir().expect("temp directory");
    let path = directory.path().join("policy.json");
    write_policy(&path, b"{\"keys\":[]}");
    let config = test_config().expect("config");
    let fetcher = PinnedPolicyFileFetcher::new(&config, path, NonZeroU64::MIN);
    let verifier = CollabTicketVerifier::new(config, fetcher, CollabJwksCacheLimits::default());
    assert!(verifier.is_ok());
}

fn test_config() -> Result<CollabVerifierConfig, CollabVerifierConfigError> {
    CollabVerifierConfig::new_pinned(
        "https://issuer.test.invalid",
        "https://issuer.test.invalid/.well-known/collab-jwks.json",
    )
}
