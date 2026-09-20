use std::{
    env,
    ffi::OsString,
    fmt,
    net::SocketAddr,
    num::{NonZeroU64, NonZeroUsize},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use op_auth_bridge::CollabVerifierConfigError;
#[cfg(unix)]
use op_auth_bridge::{
    CollabJwksCacheLimits, CollabTicketVerifier, CollabVerifierConfig,
    DEFAULT_MAX_COLLAB_JWKS_BYTES,
};
#[cfg(unix)]
use op_collab_policy_file::PinnedPolicyFileFetcher;
#[cfg(unix)]
use op_collab_relay_control_plane::{
    RegionBoundOwnerPublishPolicy, RelayLocatorPublishService, RelayPairingService,
};
use op_collab_relay_protocol::{LocatorKeyId, RelayRegion};

#[cfg(unix)]
use crate::InMemoryPairingStore;
#[cfg(unix)]
use crate::UnixHsmRelayLocatorSigner;
use crate::{
    ExpectedUnixPeer, HsmSignerConfigError, HsmSignerError, LocatorHttpLimits, LocatorPublisher,
    LocatorServerConfig, LocatorServerConfigError, PairingEndpoints, DEFAULT_LOCATOR_LISTEN,
};

pub const LOCATOR_LISTEN_ENV: &str = "OPENPENCIL_COLLAB_LOCATOR_LISTEN";
pub const LOCATOR_HOME_REGION_ENV: &str = "OPENPENCIL_COLLAB_LOCATOR_HOME_REGION";
pub const LOCATOR_TICKET_POLICY_FILE_ENV: &str = "OPENPENCIL_COLLAB_LOCATOR_TICKET_POLICY_FILE";
pub const LOCATOR_POLICY_MAX_AGE_ENV: &str = "OPENPENCIL_COLLAB_LOCATOR_POLICY_MAX_AGE_SECONDS";
pub const LOCATOR_HSM_SOCKET_ENV: &str = "OPENPENCIL_COLLAB_LOCATOR_HSM_SOCKET";
pub const LOCATOR_HSM_KEY_ID_ENV: &str = "OPENPENCIL_COLLAB_LOCATOR_HSM_KEY_ID";
pub const LOCATOR_PUBLIC_KEYS_FILE_ENV: &str = "OPENPENCIL_COLLAB_LOCATOR_PUBLIC_KEYS_FILE";
pub const LOCATOR_HSM_PEER_UID_ENV: &str = "OPENPENCIL_COLLAB_LOCATOR_HSM_PEER_UID";
pub const LOCATOR_HSM_PEER_GID_ENV: &str = "OPENPENCIL_COLLAB_LOCATOR_HSM_PEER_GID";
pub const LOCATOR_HSM_TIMEOUT_MS_ENV: &str = "OPENPENCIL_COLLAB_LOCATOR_HSM_TIMEOUT_MS";
pub const LOCATOR_MAX_AUTH_IN_FLIGHT_ENV: &str = "OPENPENCIL_COLLAB_LOCATOR_MAX_AUTH_IN_FLIGHT";
pub const LOCATOR_RATE_PER_SECOND_ENV: &str = "OPENPENCIL_COLLAB_LOCATOR_RATE_PER_SECOND";

const DEFAULT_POLICY_MAX_AGE_SECONDS: u64 = 60;
const MAX_POLICY_MAX_AGE_SECONDS: u64 = 60 * 60;
const DEFAULT_HSM_TIMEOUT_MS: u64 = 2_000;
const MIN_HSM_TIMEOUT_MS: u64 = 50;
const MAX_HSM_TIMEOUT_MS: u64 = 5_000;
const MAX_AUTH_IN_FLIGHT: usize = 256;
const MAX_RATE_PER_SECOND: u32 = 10_000;

mod production_check;
pub use production_check::{check_production, ProductionLocatorCheckError};

#[cfg(test)]
mod production_check_tests;

pub struct ProductionLocatorConfig {
    server: LocatorServerConfig,
    home_region: RelayRegion,
    ticket_policy_file: PathBuf,
    policy_max_age_seconds: NonZeroU64,
    hsm_socket: PathBuf,
    hsm_key_id: LocatorKeyId,
    hsm_peer: ExpectedUnixPeer,
    hsm_timeout: Duration,
}

impl ProductionLocatorConfig {
    pub fn from_env() -> Result<Self, ProductionLocatorConfigError> {
        let listen = optional_unicode(LOCATOR_LISTEN_ENV)?
            .unwrap_or_else(|| DEFAULT_LOCATOR_LISTEN.to_owned())
            .parse::<SocketAddr>()
            .map_err(|_| ProductionLocatorConfigError::InvalidListen)?;
        let home_region = match required_unicode(LOCATOR_HOME_REGION_ENV)?.as_str() {
            "cn" => RelayRegion::Cn,
            "global" => RelayRegion::Global,
            _ => return Err(ProductionLocatorConfigError::InvalidRegion),
        };
        let ticket_policy_file = required_absolute_path(LOCATOR_TICKET_POLICY_FILE_ENV)?;
        let hsm_socket = required_absolute_path(LOCATOR_HSM_SOCKET_ENV)?;
        let hsm_key_id = LocatorKeyId::new(required_unicode(LOCATOR_HSM_KEY_ID_ENV)?)
            .map_err(|_| ProductionLocatorConfigError::InvalidHsmKeyId)?;
        let hsm_peer = ExpectedUnixPeer {
            uid: required_number(LOCATOR_HSM_PEER_UID_ENV)?,
            gid: required_number(LOCATOR_HSM_PEER_GID_ENV)?,
        };
        let policy_max_age_seconds =
            optional_bounded_u64(LOCATOR_POLICY_MAX_AGE_ENV, 1, MAX_POLICY_MAX_AGE_SECONDS)?
                .unwrap_or(DEFAULT_POLICY_MAX_AGE_SECONDS);
        let hsm_timeout_ms = optional_bounded_u64(
            LOCATOR_HSM_TIMEOUT_MS_ENV,
            MIN_HSM_TIMEOUT_MS,
            MAX_HSM_TIMEOUT_MS,
        )?
        .unwrap_or(DEFAULT_HSM_TIMEOUT_MS);
        let mut limits = LocatorHttpLimits::default();
        if let Some(value) =
            optional_bounded_u64(LOCATOR_MAX_AUTH_IN_FLIGHT_ENV, 1, MAX_AUTH_IN_FLIGHT as u64)?
        {
            limits.max_auth_in_flight =
                NonZeroUsize::new(value as usize).expect("bounded non-zero");
        }
        if let Some(value) = optional_bounded_u64(
            LOCATOR_RATE_PER_SECOND_ENV,
            1,
            u64::from(MAX_RATE_PER_SECOND),
        )? {
            limits.max_requests_per_second =
                std::num::NonZeroU32::new(value as u32).expect("bounded non-zero");
        }
        // Lowering the global ceiling must not turn the default per-client rate
        // into a startup error. An explicit per-client override is applied after
        // this clamp, so a value that exceeds the ceiling still fails loudly.
        limits.max_client_requests_per_second = limits
            .max_client_requests_per_second
            .min(limits.max_requests_per_second);
        limits.apply_env_overrides()?;
        let server = LocatorServerConfig::new(listen, limits)?;
        Ok(Self {
            server,
            home_region,
            ticket_policy_file,
            policy_max_age_seconds: NonZeroU64::new(policy_max_age_seconds)
                .expect("bounded non-zero"),
            hsm_socket,
            hsm_key_id,
            hsm_peer,
            hsm_timeout: Duration::from_millis(hsm_timeout_ms),
        })
    }

    pub const fn server(&self) -> &LocatorServerConfig {
        &self.server
    }
}

impl fmt::Debug for ProductionLocatorConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionLocatorConfig")
            .field("server", &self.server)
            .field("home_region", &self.home_region)
            .field(
                "ticket_policy_file",
                &redacted_path(&self.ticket_policy_file),
            )
            .field("policy_max_age_seconds", &self.policy_max_age_seconds)
            .field("hsm_socket", &redacted_path(&self.hsm_socket))
            .field("hsm_key_id", &self.hsm_key_id)
            .field("hsm_peer", &self.hsm_peer)
            .field("hsm_timeout", &self.hsm_timeout)
            .finish()
    }
}

fn redacted_path(_path: &Path) -> &'static str {
    "[REDACTED]"
}

#[cfg(unix)]
pub fn build_production_publisher(
    config: &ProductionLocatorConfig,
) -> Result<Arc<dyn LocatorPublisher>, ProductionLocatorConfigError> {
    let verifier_config = CollabVerifierConfig::production();
    let fetcher = validated_policy_fetcher(config, &verifier_config)?;
    let ticket_verifier =
        CollabTicketVerifier::new(verifier_config, fetcher, CollabJwksCacheLimits::default())?;
    let signer = UnixHsmRelayLocatorSigner::new(
        &config.hsm_socket,
        config.hsm_key_id.clone(),
        config.hsm_peer,
        config.hsm_timeout,
    )?;
    signer
        .validate_socket()
        .map_err(ProductionLocatorConfigError::HsmUnavailable)?;
    Ok(Arc::new(RelayLocatorPublishService::new(
        ticket_verifier,
        signer,
        RegionBoundOwnerPublishPolicy::new(config.home_region),
    )))
}

/// NOTE: the in-memory store is process-local. Behind a multi-replica or
/// restarting deployment, publish and claim can land on different instances
/// and 404; such deployments need a shared `PairingCodeStore` impl.
#[cfg(unix)]
pub fn build_production_pairing(
    config: &ProductionLocatorConfig,
) -> Result<Arc<dyn PairingEndpoints>, ProductionLocatorConfigError> {
    let verifier_config = CollabVerifierConfig::production();
    let fetcher = validated_policy_fetcher(config, &verifier_config)?;
    Ok(Arc::new(RelayPairingService::production(
        fetcher,
        InMemoryPairingStore::default(),
    )?))
}

#[cfg(unix)]
fn validated_policy_fetcher(
    config: &ProductionLocatorConfig,
    verifier_config: &CollabVerifierConfig,
) -> Result<PinnedPolicyFileFetcher, ProductionLocatorConfigError> {
    let fetcher = PinnedPolicyFileFetcher::new(
        verifier_config,
        &config.ticket_policy_file,
        config.policy_max_age_seconds,
    );
    fetcher
        .validate_source(DEFAULT_MAX_COLLAB_JWKS_BYTES)
        .map_err(|_| ProductionLocatorConfigError::UnsafePolicyFile)?;
    Ok(fetcher)
}

#[cfg(not(unix))]
pub fn build_production_publisher(
    _config: &ProductionLocatorConfig,
) -> Result<Arc<dyn LocatorPublisher>, ProductionLocatorConfigError> {
    Err(ProductionLocatorConfigError::UnsupportedPlatform)
}

#[cfg(not(unix))]
pub fn build_production_pairing(
    _config: &ProductionLocatorConfig,
) -> Result<Arc<dyn PairingEndpoints>, ProductionLocatorConfigError> {
    Err(ProductionLocatorConfigError::UnsupportedPlatform)
}

fn required_unicode(name: &'static str) -> Result<String, ProductionLocatorConfigError> {
    optional_unicode(name)?.ok_or(ProductionLocatorConfigError::Missing(name))
}

fn optional_unicode(name: &'static str) -> Result<Option<String>, ProductionLocatorConfigError> {
    env::var_os(name)
        .map(|value| {
            value
                .into_string()
                .map_err(|_| ProductionLocatorConfigError::NonUnicode(name))
                .and_then(|value| {
                    if value.is_empty() {
                        Err(ProductionLocatorConfigError::Missing(name))
                    } else {
                        Ok(value)
                    }
                })
        })
        .transpose()
}

fn required_absolute_path(name: &'static str) -> Result<PathBuf, ProductionLocatorConfigError> {
    let value = env::var_os(name).ok_or(ProductionLocatorConfigError::Missing(name))?;
    absolute_path(name, value)
}

fn absolute_path(
    name: &'static str,
    value: OsString,
) -> Result<PathBuf, ProductionLocatorConfigError> {
    if value.is_empty() {
        return Err(ProductionLocatorConfigError::Missing(name));
    }
    let path = Path::new(&value);
    if !path.is_absolute() {
        return Err(ProductionLocatorConfigError::PathNotAbsolute(name));
    }
    Ok(path.to_path_buf())
}

fn required_number<T>(name: &'static str) -> Result<T, ProductionLocatorConfigError>
where
    T: std::str::FromStr,
{
    required_unicode(name)?
        .parse()
        .map_err(|_| ProductionLocatorConfigError::InvalidNumber(name))
}

fn optional_bounded_u64(
    name: &'static str,
    minimum: u64,
    maximum: u64,
) -> Result<Option<u64>, ProductionLocatorConfigError> {
    optional_unicode(name)?
        .map(|value| {
            value
                .parse::<u64>()
                .ok()
                .filter(|value| (*value >= minimum) && (*value <= maximum))
                .ok_or(ProductionLocatorConfigError::InvalidNumber(name))
        })
        .transpose()
}

#[derive(Debug, thiserror::Error)]
pub enum ProductionLocatorConfigError {
    #[error("required production locator setting {0} is missing")]
    Missing(&'static str),
    #[error("production locator setting {0} is not valid Unicode")]
    NonUnicode(&'static str),
    #[error("production locator setting {0} must be an absolute path")]
    PathNotAbsolute(&'static str),
    #[error("production locator setting {0} is not a valid bounded number")]
    InvalidNumber(&'static str),
    #[error("production locator listen address is invalid")]
    InvalidListen,
    #[error("production locator home region must be `cn` or `global`")]
    InvalidRegion,
    #[error("production locator HSM key id is invalid")]
    InvalidHsmKeyId,
    #[error("production locator signed-policy file is unavailable or unsafe")]
    UnsafePolicyFile,
    #[error("production locator HSM socket configuration is invalid")]
    HsmConfig(#[from] HsmSignerConfigError),
    #[error("production locator HSM is unavailable or rejected")]
    HsmUnavailable(#[source] HsmSignerError),
    #[error("production locator ticket verifier configuration is invalid")]
    TicketVerifier(#[from] CollabVerifierConfigError),
    #[error(transparent)]
    Server(#[from] LocatorServerConfigError),
    #[error("production locator HSM peer authentication is unsupported on this platform")]
    UnsupportedPlatform,
}
