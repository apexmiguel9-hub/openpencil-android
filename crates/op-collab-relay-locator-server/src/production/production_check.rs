use std::{ffi::OsString, path::Path, time::SystemTime};

// The real check runs a HSM signing round trip that only exists on unix
// (see the cfg(not(unix)) stub below), so everything it pulls in is
// unix-only — matching the existing UnixHsmRelayLocatorSigner import.
#[cfg(unix)]
use std::num::NonZeroU64;

#[cfg(unix)]
use op_auth_bridge::{CollabUnionPolicy, CollabVerifierConfig, DEFAULT_MAX_COLLAB_JWKS_BYTES};
#[cfg(unix)]
use op_collab_policy_file::{read_bounded_regular_file, PinnedEd25519LocatorVerifier};
#[cfg(unix)]
use op_collab_relay_control_plane::RelayLocatorSigner;
#[cfg(unix)]
use op_collab_relay_protocol::{
    ExpectedDiscoveryId, OwnerNoiseStatic, RelayLocatorVerifier, RouteId, UnsignedRelayLocatorV1,
};
#[cfg(unix)]
use sha2::{Digest as _, Sha256};

use super::{required_absolute_path, ProductionLocatorConfig, LOCATOR_PUBLIC_KEYS_FILE_ENV};
#[cfg(unix)]
use crate::UnixHsmRelayLocatorSigner;

#[cfg(unix)]
const EXPIRED_NOT_BEFORE_UNIX: u64 = 1;
const EXPIRED_AT_UNIX: u64 = 2;
const EXPECTED_POLICY_SHA256_ENV: &str = "OPENPENCIL_COLLAB_EXPECTED_POLICY_SHA256";

/// Verify the production policy and exercise one real HSM signing round trip.
pub fn check_production() -> Result<(), ProductionLocatorCheckError> {
    let now_unix_seconds = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
        .filter(|now| *now > EXPIRED_AT_UNIX)
        .ok_or(ProductionLocatorCheckError::Clock)?;
    let config = ProductionLocatorConfig::from_env()
        .map_err(|_| ProductionLocatorCheckError::Configuration)?;
    let public_keys_file = required_absolute_path(LOCATOR_PUBLIC_KEYS_FILE_ENV)
        .map_err(|_| ProductionLocatorCheckError::Configuration)?;
    let expected_policy_sha256 = expected_policy_sha256_from_env()?;
    check_production_config_at(
        &config,
        &public_keys_file,
        now_unix_seconds,
        &expected_policy_sha256,
    )
}

#[cfg(unix)]
pub(crate) fn check_production_config_at(
    config: &ProductionLocatorConfig,
    public_keys_file: &Path,
    now_unix_seconds: u64,
    expected_policy_sha256: &str,
) -> Result<(), ProductionLocatorCheckError> {
    if now_unix_seconds <= EXPIRED_AT_UNIX {
        return Err(ProductionLocatorCheckError::Clock);
    }
    let verifier_config = CollabVerifierConfig::production();
    let policy_body =
        read_bounded_regular_file(&config.ticket_policy_file, DEFAULT_MAX_COLLAB_JWKS_BYTES)
            .map_err(|_| ProductionLocatorCheckError::Policy)?;
    if format!("{:x}", Sha256::digest(&policy_body)) != expected_policy_sha256 {
        return Err(ProductionLocatorCheckError::Policy);
    }
    CollabUnionPolicy::from_json(
        &policy_body,
        DEFAULT_MAX_COLLAB_JWKS_BYTES,
        verifier_config.issuer(),
        now_unix_seconds,
    )
    .map_err(|_| ProductionLocatorCheckError::Policy)?;

    let verifier = PinnedEd25519LocatorVerifier::from_file(public_keys_file)
        .map_err(|_| ProductionLocatorCheckError::LocatorKeys)?;
    let signer = UnixHsmRelayLocatorSigner::new(
        &config.hsm_socket,
        config.hsm_key_id.clone(),
        config.hsm_peer,
        config.hsm_timeout,
    )
    .map_err(|_| ProductionLocatorCheckError::Hsm)?;
    signer
        .validate_socket()
        .map_err(|_| ProductionLocatorCheckError::Hsm)?;

    let claims = fixed_expired_claims(config)?;
    if claims.validate_pairing_window(now_unix_seconds).is_ok() {
        return Err(ProductionLocatorCheckError::ProbeProfile);
    }
    let canonical = claims.canonical_signing_bytes();
    let signature = signer
        .sign(&config.hsm_key_id, &canonical)
        .map_err(|_| ProductionLocatorCheckError::Hsm)?;
    if !verifier.verify(&config.hsm_key_id, &canonical, signature.as_bytes()) {
        return Err(ProductionLocatorCheckError::Signature);
    }
    Ok(())
}

#[cfg(not(unix))]
pub(crate) fn check_production_config_at(
    _config: &ProductionLocatorConfig,
    _public_keys_file: &Path,
    _now_unix_seconds: u64,
    _expected_policy_sha256: &str,
) -> Result<(), ProductionLocatorCheckError> {
    Err(ProductionLocatorCheckError::UnsupportedPlatform)
}

fn expected_policy_sha256_from_env() -> Result<String, ProductionLocatorCheckError> {
    parse_expected_policy_sha256(std::env::var_os(EXPECTED_POLICY_SHA256_ENV))
}

pub(crate) fn parse_expected_policy_sha256(
    value: Option<OsString>,
) -> Result<String, ProductionLocatorCheckError> {
    let value = value
        .ok_or(ProductionLocatorCheckError::Configuration)?
        .into_string()
        .map_err(|_| ProductionLocatorCheckError::Configuration)?;
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ProductionLocatorCheckError::Configuration);
    }
    Ok(value)
}

#[cfg(unix)]
fn fixed_expired_claims(
    config: &ProductionLocatorConfig,
) -> Result<UnsignedRelayLocatorV1, ProductionLocatorCheckError> {
    let route_id =
        RouteId::new([0x51; 16]).map_err(|_| ProductionLocatorCheckError::ProbeProfile)?;
    let owner_static =
        OwnerNoiseStatic::new([0x52; 32]).map_err(|_| ProductionLocatorCheckError::ProbeProfile)?;
    let discovery = ExpectedDiscoveryId::new("production-check-expired-v1")
        .map_err(|_| ProductionLocatorCheckError::ProbeProfile)?;
    UnsignedRelayLocatorV1::new(
        config.home_region,
        route_id,
        NonZeroU64::new(1).expect("fixed generation is non-zero"),
        owner_static,
        discovery,
        EXPIRED_NOT_BEFORE_UNIX,
        EXPIRED_AT_UNIX,
        config.hsm_key_id.clone(),
    )
    .map_err(|_| ProductionLocatorCheckError::ProbeProfile)
}

#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProductionLocatorCheckError {
    #[error("configuration")]
    Configuration,
    #[error("clock")]
    Clock,
    #[error("signed policy")]
    Policy,
    #[error("locator verification keys")]
    LocatorKeys,
    #[error("probe profile")]
    ProbeProfile,
    #[error("HSM signing")]
    Hsm,
    #[error("HSM signature verification")]
    Signature,
    #[error("unsupported platform")]
    UnsupportedPlatform,
}
