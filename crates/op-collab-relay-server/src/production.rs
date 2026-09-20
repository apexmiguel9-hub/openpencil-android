use std::{
    env,
    ffi::OsString,
    fmt,
    num::NonZeroU64,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use op_auth_bridge::{
    CollabJwksCacheLimits, CollabJwksFetchError, CollabTicketVerifier, CollabUnionPolicy,
    CollabVerifierConfig, CollabVerifierConfigError, DEFAULT_MAX_COLLAB_JWKS_BYTES,
};
use op_collab_policy_file::read_bounded_regular_file;
use op_collab_relay_protocol::RelayRegion;
use sha2::{Digest as _, Sha256};

use crate::{
    run_with_authenticator, CollabTicketRelayAuthenticator, PinnedEd25519LocatorVerifier,
    PinnedPolicyFileFetcher, PinnedVerifierError, PinnedX25519KeyError, PinnedX25519ProofBoundary,
    RelayConfig, RelayServerError, RelayServerX25519ProofBoundary,
};

pub const HOME_REGION_ENV: &str = "OPENPENCIL_COLLAB_RELAY_HOME_REGION";
pub const TICKET_POLICY_FILE_ENV: &str = "OPENPENCIL_COLLAB_RELAY_TICKET_POLICY_FILE";
pub const LOCATOR_KEYS_FILE_ENV: &str = "OPENPENCIL_COLLAB_RELAY_LOCATOR_KEYS_FILE";
pub const RELAY_X25519_KEYS_FILE_ENV: &str = "OPENPENCIL_COLLAB_RELAY_X25519_KEYS_FILE";
pub const POLICY_MAX_AGE_ENV: &str = "OPENPENCIL_COLLAB_RELAY_POLICY_MAX_AGE_SECONDS";
pub const EXPECTED_POLICY_SHA256_ENV: &str = "OPENPENCIL_COLLAB_EXPECTED_POLICY_SHA256";
/// Migration switch for the legacy full-collaboration-ticket relay bearer.
///
/// `accept` (the default) dual-accepts the claim-minimized relay token and the
/// legacy ticket so pre-migration clients keep connecting. `reject` narrows the
/// relay to the minimized token only. Any other value fails closed at startup
/// rather than silently picking a policy.
pub const LEGACY_TICKET_BEARER_ENV: &str = "OPENPENCIL_COLLAB_RELAY_LEGACY_TICKET_BEARER";

const DEFAULT_POLICY_MAX_AGE_SECONDS: u64 = 60;
const MAX_POLICY_MAX_AGE_SECONDS: u64 = 60 * 60;

/// Verify every production trust input without opening a network listener.
///
/// The signed policy is parsed here, rather than merely opened, so the binary's
/// embedded union roots and generation fence are exercised before promotion.
pub fn check_production() -> Result<(), ProductionRelayCheckError> {
    let now_unix_seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
        .filter(|now| *now != 0)
        .ok_or(ProductionRelayCheckError::Clock)?;
    let config = ProductionRelayAuthConfig::from_env(false)
        .map_err(|_| ProductionRelayCheckError::Configuration)?;
    let expected_policy_sha256 = expected_policy_sha256_from_env()?;
    check_production_config_at(&config, now_unix_seconds, &expected_policy_sha256)
}

pub(crate) fn check_production_config_at(
    config: &ProductionRelayAuthConfig,
    now_unix_seconds: u64,
    expected_policy_sha256: &str,
) -> Result<(), ProductionRelayCheckError> {
    let verifier_config = CollabVerifierConfig::production();
    let policy_body =
        read_bounded_regular_file(&config.ticket_policy_file, DEFAULT_MAX_COLLAB_JWKS_BYTES)
            .map_err(|_| ProductionRelayCheckError::Policy)?;
    if format!("{:x}", Sha256::digest(&policy_body)) != expected_policy_sha256 {
        return Err(ProductionRelayCheckError::Policy);
    }
    CollabUnionPolicy::from_json(
        &policy_body,
        DEFAULT_MAX_COLLAB_JWKS_BYTES,
        verifier_config.issuer(),
        now_unix_seconds,
    )
    .map_err(|_| ProductionRelayCheckError::Policy)?;
    PinnedEd25519LocatorVerifier::from_file(&config.locator_keys_file)
        .map_err(|_| ProductionRelayCheckError::LocatorKeys)?;
    let x25519_path = config
        .relay_x25519_keys_file
        .as_ref()
        .ok_or(ProductionRelayCheckError::RelayX25519Keys)?;
    let boundary = PinnedX25519ProofBoundary::from_file(x25519_path)
        .map_err(|_| ProductionRelayCheckError::RelayX25519Keys)?;
    boundary
        .active_key_id()
        .map_err(|_| ProductionRelayCheckError::RelayX25519Keys)?;
    Ok(())
}

fn expected_policy_sha256_from_env() -> Result<String, ProductionRelayCheckError> {
    parse_expected_policy_sha256(env::var_os(EXPECTED_POLICY_SHA256_ENV))
}

pub(crate) fn parse_expected_policy_sha256(
    value: Option<OsString>,
) -> Result<String, ProductionRelayCheckError> {
    let value = value
        .ok_or(ProductionRelayCheckError::Configuration)?
        .into_string()
        .map_err(|_| ProductionRelayCheckError::Configuration)?;
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ProductionRelayCheckError::Configuration);
    }
    Ok(value)
}

#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProductionRelayCheckError {
    #[error("configuration")]
    Configuration,
    #[error("clock")]
    Clock,
    #[error("signed policy")]
    Policy,
    #[error("locator verification keys")]
    LocatorKeys,
    #[error("relay proof keys")]
    RelayX25519Keys,
}

pub struct ProductionRelayAuthConfig {
    home_region: RelayRegion,
    ticket_policy_file: PathBuf,
    locator_keys_file: PathBuf,
    relay_x25519_keys_file: Option<PathBuf>,
    policy_max_age_seconds: NonZeroU64,
    allow_ticket_binding_only: bool,
    accept_legacy_ticket_bearer: bool,
}

impl ProductionRelayAuthConfig {
    pub fn from_env(
        allow_ticket_binding_only: bool,
    ) -> Result<Self, ProductionRelayAuthConfigError> {
        let home_region = match required_unicode(HOME_REGION_ENV)?.as_str() {
            "cn" => RelayRegion::Cn,
            "global" => RelayRegion::Global,
            _ => return Err(ProductionRelayAuthConfigError::InvalidRegion),
        };
        let ticket_policy_file = required_absolute_path(TICKET_POLICY_FILE_ENV)?;
        let locator_keys_file = required_absolute_path(LOCATOR_KEYS_FILE_ENV)?;
        let relay_x25519_keys_file = optional_absolute_path(RELAY_X25519_KEYS_FILE_ENV)?;
        let policy_max_age_seconds = match env::var_os(POLICY_MAX_AGE_ENV) {
            Some(value) => {
                let value = value
                    .into_string()
                    .map_err(|_| ProductionRelayAuthConfigError::NonUnicode(POLICY_MAX_AGE_ENV))?;
                value
                    .parse::<u64>()
                    .ok()
                    .and_then(NonZeroU64::new)
                    .filter(|value| value.get() <= MAX_POLICY_MAX_AGE_SECONDS)
                    .ok_or(ProductionRelayAuthConfigError::InvalidPolicyMaxAge)?
            }
            None => NonZeroU64::new(DEFAULT_POLICY_MAX_AGE_SECONDS)
                .expect("default policy max age is non-zero"),
        };
        let accept_legacy_ticket_bearer = legacy_ticket_bearer_from_env()?;
        Self::new(
            home_region,
            ticket_policy_file,
            locator_keys_file,
            relay_x25519_keys_file,
            policy_max_age_seconds,
            allow_ticket_binding_only,
        )
        .map(|config| config.with_legacy_ticket_bearer(accept_legacy_ticket_bearer))
    }

    pub fn new(
        home_region: RelayRegion,
        ticket_policy_file: PathBuf,
        locator_keys_file: PathBuf,
        relay_x25519_keys_file: Option<PathBuf>,
        policy_max_age_seconds: NonZeroU64,
        allow_ticket_binding_only: bool,
    ) -> Result<Self, ProductionRelayAuthConfigError> {
        if policy_max_age_seconds.get() > MAX_POLICY_MAX_AGE_SECONDS {
            return Err(ProductionRelayAuthConfigError::InvalidPolicyMaxAge);
        }
        for (name, path) in [
            (TICKET_POLICY_FILE_ENV, Some(ticket_policy_file.as_path())),
            (LOCATOR_KEYS_FILE_ENV, Some(locator_keys_file.as_path())),
            (
                RELAY_X25519_KEYS_FILE_ENV,
                relay_x25519_keys_file.as_deref(),
            ),
        ] {
            if path.is_some_and(|path| !path.is_absolute()) {
                return Err(ProductionRelayAuthConfigError::PathNotAbsolute(name));
            }
        }
        match (relay_x25519_keys_file.is_some(), allow_ticket_binding_only) {
            (false, false) => {
                return Err(ProductionRelayAuthConfigError::ChallengeProofRequired);
            }
            (true, true) => {
                return Err(ProductionRelayAuthConfigError::ConflictingProofPolicy);
            }
            _ => {}
        }
        Ok(Self {
            home_region,
            ticket_policy_file,
            locator_keys_file,
            relay_x25519_keys_file,
            policy_max_age_seconds,
            allow_ticket_binding_only,
            // Migration default: keep accepting the legacy bearer so clients
            // that predate the minimized relay token can still connect.
            accept_legacy_ticket_bearer: true,
        })
    }

    /// Select whether the legacy full collaboration ticket is still accepted
    /// as a relay bearer. See [`LEGACY_TICKET_BEARER_ENV`].
    pub const fn with_legacy_ticket_bearer(mut self, accepted: bool) -> Self {
        self.accept_legacy_ticket_bearer = accepted;
        self
    }

    pub const fn home_region(&self) -> RelayRegion {
        self.home_region
    }

    pub const fn reduced_assurance(&self) -> bool {
        self.allow_ticket_binding_only
    }

    pub const fn accepts_legacy_ticket_bearer(&self) -> bool {
        self.accept_legacy_ticket_bearer
    }
}

fn legacy_ticket_bearer_from_env() -> Result<bool, ProductionRelayAuthConfigError> {
    match env::var_os(LEGACY_TICKET_BEARER_ENV) {
        None => Ok(true),
        Some(value) => {
            let value = value.into_string().map_err(|_| {
                ProductionRelayAuthConfigError::NonUnicode(LEGACY_TICKET_BEARER_ENV)
            })?;
            match value.as_str() {
                "" | "accept" => Ok(true),
                "reject" => Ok(false),
                _ => Err(ProductionRelayAuthConfigError::InvalidLegacyTicketBearer),
            }
        }
    }
}

impl fmt::Debug for ProductionRelayAuthConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionRelayAuthConfig")
            .field("home_region", &self.home_region)
            .field("ticket_policy_file", &"[REDACTED]")
            .field("locator_keys_file", &"[REDACTED]")
            .field(
                "relay_x25519_keys_file",
                &self.relay_x25519_keys_file.as_ref().map(|_| "[REDACTED]"),
            )
            .field("policy_max_age_seconds", &self.policy_max_age_seconds)
            .field("allow_ticket_binding_only", &self.allow_ticket_binding_only)
            .field(
                "accept_legacy_ticket_bearer",
                &self.accept_legacy_ticket_bearer,
            )
            .finish()
    }
}

pub async fn run_production(
    relay_config: RelayConfig,
    auth_config: ProductionRelayAuthConfig,
) -> Result<(), ProductionRelayError> {
    let locator_verifier = PinnedEd25519LocatorVerifier::from_file(&auth_config.locator_keys_file)
        .map_err(ProductionRelayError::LocatorKeys)?;
    let verifier_config = CollabVerifierConfig::production();
    let fetcher = PinnedPolicyFileFetcher::new(
        &verifier_config,
        &auth_config.ticket_policy_file,
        auth_config.policy_max_age_seconds,
    );
    fetcher
        .validate_source(DEFAULT_MAX_COLLAB_JWKS_BYTES)
        .map_err(ProductionRelayError::TicketPolicyFile)?;
    let ticket_verifier =
        CollabTicketVerifier::new(verifier_config, fetcher, CollabJwksCacheLimits::default())?;

    if let Some(path) = auth_config.relay_x25519_keys_file {
        let proof_boundary = PinnedX25519ProofBoundary::from_file(path)
            .map_err(ProductionRelayError::RelayX25519Keys)?;
        let authenticator = CollabTicketRelayAuthenticator::new(
            ticket_verifier,
            auth_config.home_region,
            locator_verifier,
            proof_boundary,
        )
        .with_full_collab_ticket_accepted(auth_config.accept_legacy_ticket_bearer);
        run_with_authenticator(relay_config, Arc::new(authenticator)).await?;
    } else {
        debug_assert!(auth_config.allow_ticket_binding_only);
        let authenticator = CollabTicketRelayAuthenticator::new_ticket_binding_only(
            ticket_verifier,
            auth_config.home_region,
            locator_verifier,
        )
        .with_full_collab_ticket_accepted(auth_config.accept_legacy_ticket_bearer);
        run_with_authenticator(relay_config, Arc::new(authenticator)).await?;
    }
    Ok(())
}

fn required_unicode(name: &'static str) -> Result<String, ProductionRelayAuthConfigError> {
    let value = env::var_os(name).ok_or(ProductionRelayAuthConfigError::Missing(name))?;
    let value = value
        .into_string()
        .map_err(|_| ProductionRelayAuthConfigError::NonUnicode(name))?;
    if value.is_empty() {
        return Err(ProductionRelayAuthConfigError::Missing(name));
    }
    Ok(value)
}

fn required_absolute_path(name: &'static str) -> Result<PathBuf, ProductionRelayAuthConfigError> {
    let value = env::var_os(name).ok_or(ProductionRelayAuthConfigError::Missing(name))?;
    absolute_path(name, value)?.ok_or(ProductionRelayAuthConfigError::Missing(name))
}

fn optional_absolute_path(
    name: &'static str,
) -> Result<Option<PathBuf>, ProductionRelayAuthConfigError> {
    env::var_os(name)
        .map(|value| absolute_path(name, value))
        .transpose()
        .map(Option::flatten)
}

fn absolute_path(
    name: &'static str,
    value: OsString,
) -> Result<Option<PathBuf>, ProductionRelayAuthConfigError> {
    if value.is_empty() {
        return Ok(None);
    }
    let path = Path::new(&value);
    if !path.is_absolute() {
        return Err(ProductionRelayAuthConfigError::PathNotAbsolute(name));
    }
    Ok(Some(path.to_path_buf()))
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProductionRelayAuthConfigError {
    #[error("required production relay setting {0} is missing")]
    Missing(&'static str),
    #[error("production relay setting {0} is not valid Unicode")]
    NonUnicode(&'static str),
    #[error("production relay home region must be `cn` or `global`")]
    InvalidRegion,
    #[error("production relay setting {0} must be an absolute path")]
    PathNotAbsolute(&'static str),
    #[error("production relay policy max age must be between 1 and 3600 seconds")]
    InvalidPolicyMaxAge,
    #[error("production relay legacy ticket bearer policy must be `accept` or `reject`")]
    InvalidLegacyTicketBearer,
    #[error(
        "a relay X25519 key file is required unless \
         --allow-ticket-binding-only is explicitly selected"
    )]
    ChallengeProofRequired,
    #[error(
        "--allow-ticket-binding-only cannot be combined with a configured relay X25519 key file"
    )]
    ConflictingProofPolicy,
}

#[derive(Debug, thiserror::Error)]
pub enum ProductionRelayError {
    #[error("pinned ticket policy file is unavailable or unsafe: {0}")]
    TicketPolicyFile(#[source] CollabJwksFetchError),
    #[error("pinned locator verifier configuration is invalid: {0}")]
    LocatorKeys(#[source] PinnedVerifierError),
    #[error("relay X25519 proof-key configuration is invalid: {0}")]
    RelayX25519Keys(#[source] PinnedX25519KeyError),
    #[error("collaboration ticket verifier configuration is invalid: {0}")]
    TicketVerifier(#[from] CollabVerifierConfigError),
    #[error(transparent)]
    Relay(#[from] RelayServerError),
}
