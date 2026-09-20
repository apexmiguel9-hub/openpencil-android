use std::{
    ffi::OsStr,
    fmt,
    net::SocketAddr,
    num::{NonZeroU32, NonZeroUsize},
    time::Duration,
};

pub const DEFAULT_LOCATOR_LISTEN: &str = "127.0.0.1:8092";
pub const MAX_CONFIGURED_TIMEOUT: Duration = Duration::from_secs(30);
/// Environment override for the per-client publish rate ceiling.
pub const LOCATOR_CLIENT_RATE_PER_SECOND_ENV: &str =
    "OPENPENCIL_COLLAB_LOCATOR_CLIENT_RATE_PER_SECOND";
/// Largest per-client publish rate the environment override accepts.
pub const MAX_CLIENT_RATE_PER_SECOND: u32 = 10_000;

#[derive(Clone)]
pub struct LocatorHttpLimits {
    pub max_connections: NonZeroUsize,
    pub max_in_flight: NonZeroUsize,
    pub max_auth_in_flight: NonZeroUsize,
    /// Global publish ceiling shared by every client, in requests per second.
    pub max_requests_per_second: NonZeroU32,
    /// Per-client publish ceiling, keyed by the connecting peer address, in
    /// requests per second. Bounded by `max_requests_per_second`.
    pub max_client_requests_per_second: NonZeroU32,
    pub header_timeout: Duration,
    pub body_timeout: Duration,
    pub auth_timeout: Duration,
    pub shutdown_grace: Duration,
}

impl Default for LocatorHttpLimits {
    fn default() -> Self {
        Self {
            max_connections: NonZeroUsize::new(256).expect("non-zero"),
            max_in_flight: NonZeroUsize::new(64).expect("non-zero"),
            max_auth_in_flight: NonZeroUsize::new(16).expect("non-zero"),
            max_requests_per_second: NonZeroU32::new(100).expect("non-zero"),
            max_client_requests_per_second: NonZeroU32::new(10).expect("non-zero"),
            header_timeout: Duration::from_secs(5),
            body_timeout: Duration::from_secs(5),
            auth_timeout: Duration::from_secs(5),
            shutdown_grace: Duration::from_secs(10),
        }
    }
}

impl LocatorHttpLimits {
    pub fn validate(&self) -> Result<(), LocatorServerConfigError> {
        for (field, timeout) in [
            ("header_timeout", self.header_timeout),
            ("body_timeout", self.body_timeout),
            ("auth_timeout", self.auth_timeout),
            ("shutdown_grace", self.shutdown_grace),
        ] {
            if timeout.is_zero() || timeout > MAX_CONFIGURED_TIMEOUT {
                return Err(LocatorServerConfigError::InvalidTimeout { field });
            }
        }
        if self.max_auth_in_flight > self.max_in_flight {
            return Err(LocatorServerConfigError::InvalidConcurrency);
        }
        if self.max_client_requests_per_second > self.max_requests_per_second {
            return Err(LocatorServerConfigError::InvalidRateLimit);
        }
        Ok(())
    }

    /// Applies the environment override for the per-client publish rate.
    ///
    /// Absent means "keep the configured value"; present must parse as a
    /// positive number no larger than [`MAX_CLIENT_RATE_PER_SECOND`], matching
    /// the optional bounded-number handling of the other locator settings.
    pub fn apply_env_overrides(&mut self) -> Result<(), LocatorServerConfigError> {
        let configured = std::env::var_os(LOCATOR_CLIENT_RATE_PER_SECOND_ENV);
        if let Some(value) = client_rate_per_second(configured.as_deref())? {
            self.max_client_requests_per_second = value;
        }
        Ok(())
    }
}

/// Parses the per-client rate override without touching the process
/// environment, so the bound and positivity rules stay unit-testable.
pub(crate) fn client_rate_per_second(
    configured: Option<&OsStr>,
) -> Result<Option<NonZeroU32>, LocatorServerConfigError> {
    let Some(configured) = configured else {
        return Ok(None);
    };
    configured
        .to_str()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value <= MAX_CLIENT_RATE_PER_SECOND)
        .and_then(NonZeroU32::new)
        .map(Some)
        .ok_or(LocatorServerConfigError::InvalidEnvNumber {
            name: LOCATOR_CLIENT_RATE_PER_SECOND_ENV,
        })
}

impl fmt::Debug for LocatorHttpLimits {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocatorHttpLimits")
            .field("max_connections", &self.max_connections)
            .field("max_in_flight", &self.max_in_flight)
            .field("max_auth_in_flight", &self.max_auth_in_flight)
            .field("max_requests_per_second", &self.max_requests_per_second)
            .field(
                "max_client_requests_per_second",
                &self.max_client_requests_per_second,
            )
            .field("header_timeout", &self.header_timeout)
            .field("body_timeout", &self.body_timeout)
            .field("auth_timeout", &self.auth_timeout)
            .field("shutdown_grace", &self.shutdown_grace)
            .finish()
    }
}

#[derive(Clone, Debug)]
pub struct LocatorServerConfig {
    pub listen: SocketAddr,
    pub limits: LocatorHttpLimits,
}

impl LocatorServerConfig {
    pub fn new(
        listen: SocketAddr,
        limits: LocatorHttpLimits,
    ) -> Result<Self, LocatorServerConfigError> {
        limits.validate()?;
        Ok(Self { listen, limits })
    }
}

#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
pub enum LocatorServerConfigError {
    #[error("locator server timeout {field} is outside the allowed range")]
    InvalidTimeout { field: &'static str },
    #[error("locator authentication concurrency exceeds total request concurrency")]
    InvalidConcurrency,
    #[error("locator per-client rate limit exceeds the global rate limit")]
    InvalidRateLimit,
    #[error("locator server setting {name} is not a valid bounded number")]
    InvalidEnvNumber { name: &'static str },
}
