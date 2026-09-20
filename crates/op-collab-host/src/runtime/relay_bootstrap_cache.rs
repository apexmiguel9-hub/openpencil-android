//! On-disk cache for the signed collaboration bootstrap document.
//!
//! Split out of `relay_bootstrap.rs` to keep that module under the 800-line
//! reviewability cap; this is pure code motion plus the typed persist error.

use std::path::Path;

use serde::{Deserialize, Serialize};

use sha2::{Digest as _, Sha256};

use super::{strong_etag, BootstrapError, MAX_CACHE_BYTES, MAX_ETAG_BYTES, MAX_RESPONSE_BYTES};

/// Endpoint-scoped cache file name so each hub keeps its own anti-rollback
/// generation floor. The endpoint is hashed rather than embedded: it keeps
/// the name filesystem-safe and avoids writing the configured URL into a
/// directory listing.
pub(super) fn endpoint_cache_file(endpoint: &str) -> String {
    let digest = Sha256::digest(endpoint.as_bytes());
    let mut suffix = String::with_capacity(16);
    for byte in &digest[..8] {
        suffix.push_str(&format!("{byte:02x}"));
    }
    format!("collaboration-bootstrap-v1-{suffix}.json")
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BootstrapCache {
    pub(super) endpoint: String,
    pub(super) etag: Option<String>,
    pub(super) body: String,
}

pub(super) fn read_cache(
    path: &Path,
    endpoint: &str,
) -> Result<Option<BootstrapCache>, BootstrapError> {
    // Do not follow the final component here. In particular, a dangling
    // symlink is an unsafe non-regular cache, not an absent cache.
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(BootstrapError::Cache),
    };
    if !metadata.is_file() || metadata.len() > MAX_CACHE_BYTES {
        return Err(BootstrapError::Cache);
    }
    let bytes = std::fs::read(path).map_err(|_| BootstrapError::Cache)?;
    let cache: BootstrapCache =
        serde_json::from_slice(&bytes).map_err(|_| BootstrapError::Cache)?;
    if cache.endpoint != endpoint
        || cache.body.is_empty()
        || cache.body.len() > MAX_RESPONSE_BYTES
        || cache
            .etag
            .as_ref()
            .is_some_and(|etag| etag.is_empty() || etag.len() > MAX_ETAG_BYTES)
    {
        return Err(BootstrapError::Cache);
    }
    let expected_etag = strong_etag(cache.body.as_bytes());
    if cache.etag.as_deref() != Some(expected_etag.as_str()) {
        return Err(BootstrapError::Cache);
    }
    Ok(Some(cache))
}

/// Persists the freshly verified bootstrap document.
///
/// Failures are reported with the dedicated [`BootstrapError::CachePersist`]
/// so the caller can tell "the anti-rollback generation floor could not be
/// armed for the next start" apart from "the cache we read back was corrupt".
pub(super) fn write_cache(path: &Path, cache: &BootstrapCache) -> Result<(), BootstrapError> {
    let parent = path.parent().ok_or(BootstrapError::CachePersist)?;
    let file = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or(BootstrapError::CachePersist)?;
    op_config_store::ConfigStore::at(parent)
        .write_json(file, cache)
        .map_err(|_| BootstrapError::CachePersist)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_root(label: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "op-bootstrap-cache-unit-{label}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_file(&root);
        root
    }

    fn cache() -> BootstrapCache {
        let body = "{\"version\":1}".to_owned();
        BootstrapCache {
            endpoint: "https://hub.test.invalid/api/v1/collaboration/bootstrap".to_owned(),
            etag: Some(strong_etag(body.as_bytes())),
            body,
        }
    }

    #[test]
    fn write_cache_reports_a_persist_failure_rather_than_succeeding_quietly() {
        let root = scratch_root("blocked");
        std::fs::write(&root, b"not a directory").expect("blocking file");
        assert_eq!(
            write_cache(&root.join("bootstrap.json"), &cache()),
            Err(BootstrapError::CachePersist)
        );
        let _ = std::fs::remove_file(root);
    }

    #[test]
    fn write_then_read_round_trips_and_a_corrupt_document_stays_a_cache_error() {
        let root = scratch_root("roundtrip");
        std::fs::create_dir_all(&root).expect("cache directory");
        let path = root.join("bootstrap.json");
        let written = cache();
        let endpoint = written.endpoint.clone();
        assert_eq!(write_cache(&path, &written), Ok(()));
        let read = read_cache(&path, &endpoint)
            .expect("read")
            .expect("cached document");
        assert_eq!(read.body, written.body);
        // A corrupt cache keeps the distinct `Cache` variant, so a caller can
        // still tell "could not persist" apart from "read back garbage".
        std::fs::write(&path, b"not json").expect("corrupt");
        assert!(matches!(
            read_cache(&path, &endpoint),
            Err(BootstrapError::Cache)
        ));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn only_a_missing_path_is_an_empty_cache() {
        let root = scratch_root("missing");
        let path = root.join("bootstrap.json");
        assert!(matches!(read_cache(&path, &cache().endpoint), Ok(None)));
    }

    #[test]
    fn a_non_regular_cache_is_rejected() {
        let root = scratch_root("directory");
        std::fs::create_dir_all(&root).expect("cache directory");
        assert!(matches!(
            read_cache(&root, &cache().endpoint),
            Err(BootstrapError::Cache)
        ));
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn a_dangling_symlink_is_not_mistaken_for_a_missing_cache() {
        use std::os::unix::fs::symlink;

        let root = scratch_root("dangling-symlink");
        std::fs::create_dir_all(&root).expect("cache directory");
        let path = root.join("bootstrap.json");
        symlink(root.join("missing.json"), &path).expect("cache symlink");
        assert!(matches!(
            read_cache(&path, &cache().endpoint),
            Err(BootstrapError::Cache)
        ));
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_cache_is_rejected() {
        use std::os::unix::fs::PermissionsExt as _;

        let root = scratch_root("unreadable");
        std::fs::create_dir_all(&root).expect("cache directory");
        let path = root.join("bootstrap.json");
        std::fs::write(&path, serde_json::to_vec(&cache()).expect("cache json"))
            .expect("cache file");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000))
            .expect("permissions");
        // Root and unusual ACL environments can still read mode-000 files, so
        // they cannot exercise the permission-denied branch.
        if std::fs::File::open(&path).is_ok() {
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                .expect("restore permissions");
            let _ = std::fs::remove_dir_all(root);
            return;
        }
        assert!(matches!(
            read_cache(&path, &cache().endpoint),
            Err(BootstrapError::Cache)
        ));
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .expect("restore permissions");
        let _ = std::fs::remove_dir_all(root);
    }
}
