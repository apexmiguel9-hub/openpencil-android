use std::sync::Arc;

use op_collab::Epoch;
use op_collab_transport::{
    DeviceStaticKey, FileKeyStore, KeyStoreError, OsKeyStore, StaticKeyStore,
};

use super::{CollabRuntimeError, CollabRuntimeFailure};

#[cfg(debug_assertions)]
const DEV_FILE_KEY_STORE_ENV: &str = "OPENPENCIL_COLLAB_DEV_FILE_KEYSTORE";

struct ProductionKeyStore;

impl StaticKeyStore for ProductionKeyStore {
    fn load_or_generate(&self) -> Result<DeviceStaticKey, KeyStoreError> {
        if development_file_key_store_requested() {
            return load_or_generate_file_key();
        }
        match OsKeyStore::new().load_or_generate() {
            Err(error) if allows_file_fallback(&error) => load_or_generate_file_key(),
            result => result,
        }
    }
}

pub(super) fn production_key_store() -> Arc<dyn StaticKeyStore> {
    Arc::new(ProductionKeyStore)
}

fn allows_file_fallback(error: &KeyStoreError) -> bool {
    matches!(error, KeyStoreError::PlatformStoreUnavailable)
}

fn load_or_generate_file_key() -> Result<DeviceStaticKey, KeyStoreError> {
    let root = op_config_store::openpencil_dir()?;
    FileKeyStore::new(root.join("collaboration")).load_or_generate()
}

#[cfg(debug_assertions)]
fn development_file_key_store_requested() -> bool {
    let value = std::env::var_os(DEV_FILE_KEY_STORE_ENV);
    development_file_key_store_opt_in(true, value.as_deref().and_then(|value| value.to_str()))
}

#[cfg(not(debug_assertions))]
fn development_file_key_store_requested() -> bool {
    false
}

#[cfg(any(debug_assertions, test))]
fn development_file_key_store_opt_in(debug_assertions: bool, value: Option<&str>) -> bool {
    debug_assertions && matches!(value, Some("1"))
}

pub(super) fn random_epoch() -> Result<Epoch, CollabRuntimeError> {
    let mut bytes = [0_u8; 8];
    getrandom::fill(&mut bytes)
        .map_err(|_| CollabRuntimeError::new(CollabRuntimeFailure::SecureKeyUnavailable))?;
    Ok(Epoch(u64::from_le_bytes(bytes).max(1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_fallback_is_limited_to_an_absent_platform_store() {
        assert!(allows_file_fallback(
            &KeyStoreError::PlatformStoreUnavailable
        ));
        assert!(!allows_file_fallback(
            &KeyStoreError::PlatformStoreAccessDenied
        ));
        assert!(!allows_file_fallback(&KeyStoreError::PlatformStoreFailure));
        assert!(!allows_file_fallback(&KeyStoreError::UnsafePlatformEntry));
    }

    #[test]
    fn development_file_store_opt_in_is_debug_only_and_exact() {
        let cases = [
            (true, None, false),
            (true, Some(""), false),
            (true, Some("0"), false),
            (true, Some("01"), false),
            (true, Some("true"), false),
            (true, Some("TRUE"), false),
            (true, Some(" 1"), false),
            (true, Some("1 "), false),
            (true, Some("1"), true),
            (false, None, false),
            (false, Some("0"), false),
            (false, Some("1"), false),
        ];

        for (debug_assertions, value, expected) in cases {
            assert_eq!(
                development_file_key_store_opt_in(debug_assertions, value),
                expected,
                "debug_assertions={debug_assertions}, value={value:?}"
            );
        }
    }
}
