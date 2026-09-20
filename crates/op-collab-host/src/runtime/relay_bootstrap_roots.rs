use std::collections::HashMap;

use ed25519_dalek::VerifyingKey;

use super::{canonical_ed25519_key, decode_fixed, valid_key_id, BootstrapError};

pub(super) const LEGACY_BUILTIN_ROOT_KID: &str = "openpencil-collab-union-root-v2";
pub(super) const LEGACY_BUILTIN_ROOT_X: &str = "5SVj-_jnJbuZlpDoD3M9x1eZAPDFLSq5jRb-c0xUh5A";
pub(super) const CURRENT_BUILTIN_ROOT_KID: &str = "openpencil-collab-bootstrap-root-v2";
pub(super) const CURRENT_BUILTIN_ROOT_X: &str = "2pPo4zN_Az7leTslTYWUkO-hyNjd_hZx83f6h_vUGAY";
const LEGACY_BUILTIN_ROOT_MAX_GENERATION: u64 = 2;
const CURRENT_BUILTIN_ROOT_MIN_GENERATION: u64 = 3;

pub(super) const BUILTIN_ROOTS: [(&str, &str); 2] = [
    (LEGACY_BUILTIN_ROOT_KID, LEGACY_BUILTIN_ROOT_X),
    (CURRENT_BUILTIN_ROOT_KID, CURRENT_BUILTIN_ROOT_X),
];

#[cfg(any(test, debug_assertions))]
const BOOTSTRAP_DEV_ROOT_KEYS_ENV: &str = "OPENPENCIL_COLLAB_BOOTSTRAP_DEV_ROOT_KEYS";
const MAX_ENV_BYTES: usize = 8 * 1024;
const MAX_ROOT_KEYS: usize = 8;

pub(super) fn builtin_roots() -> Result<HashMap<String, VerifyingKey>, BootstrapError> {
    let mut roots = HashMap::with_capacity(BUILTIN_ROOTS.len());
    for (kid, encoded) in BUILTIN_ROOTS {
        let bytes = decode_fixed::<32>(encoded)?;
        let key = canonical_ed25519_key(bytes, BootstrapError::InvalidRoot)?;
        if roots.insert(kid.to_owned(), key).is_some() {
            return Err(BootstrapError::InvalidRoot);
        }
    }
    Ok(roots)
}

pub(super) fn root_authorizes_generation(kid: &str, generation: u64) -> bool {
    match kid {
        LEGACY_BUILTIN_ROOT_KID => (1..=LEGACY_BUILTIN_ROOT_MAX_GENERATION).contains(&generation),
        CURRENT_BUILTIN_ROOT_KID => generation >= CURRENT_BUILTIN_ROOT_MIN_GENERATION,
        _ => true,
    }
}

#[cfg(any(test, debug_assertions))]
pub(super) fn add_development_roots(
    roots: &mut HashMap<String, VerifyingKey>,
) -> Result<(), BootstrapError> {
    let raw = std::env::var(BOOTSTRAP_DEV_ROOT_KEYS_ENV).ok();
    add_development_roots_for_build(roots, true, raw.as_deref())
}

#[cfg(not(any(test, debug_assertions)))]
pub(super) fn add_development_roots(
    roots: &mut HashMap<String, VerifyingKey>,
) -> Result<(), BootstrapError> {
    add_development_roots_for_build(roots, false, None)
}

pub(super) fn add_development_roots_for_build(
    roots: &mut HashMap<String, VerifyingKey>,
    development_build: bool,
    raw: Option<&str>,
) -> Result<(), BootstrapError> {
    if !development_build {
        return Ok(());
    }
    let Some(raw) = raw else {
        return Ok(());
    };
    if raw.is_empty() || raw.len() > MAX_ENV_BYTES {
        return Err(BootstrapError::InvalidRoot);
    }
    for entry in raw.split([',', ';']) {
        let (kid, encoded) = entry.split_once('=').ok_or(BootstrapError::InvalidRoot)?;
        if !valid_key_id(kid) || roots.len() >= MAX_ROOT_KEYS {
            return Err(BootstrapError::InvalidRoot);
        }
        let bytes = decode_fixed::<32>(encoded)?;
        let key = canonical_ed25519_key(bytes, BootstrapError::InvalidRoot)?;
        if roots.insert(kid.to_owned(), key).is_some() {
            return Err(BootstrapError::InvalidRoot);
        }
    }
    Ok(())
}
