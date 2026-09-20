//! Production HTTP boundary for relay locator publication.

mod config;
mod hsm;
mod http;
mod pairing_store;
mod production;

pub use config::{
    LocatorHttpLimits, LocatorServerConfig, LocatorServerConfigError, DEFAULT_LOCATOR_LISTEN,
};
#[cfg(unix)]
pub use hsm::UnixHsmRelayLocatorSigner;
pub use hsm::{
    ExpectedUnixPeer, HsmSignerConfigError, HsmSignerError, HSM_SIGNING_PROTOCOL_VERSION,
    HSM_SIGN_REQUEST_BYTES, HSM_SIGN_RESPONSE_BYTES,
};
pub use http::{
    serve_listener_until, LocatorPublisher, LocatorServerError, PairingEndpoints, MAX_HTTP_HEADERS,
    MAX_HTTP_HEADER_BYTES,
};
pub use pairing_store::InMemoryPairingStore;
pub use production::{
    build_production_pairing, build_production_publisher, check_production,
    ProductionLocatorCheckError, ProductionLocatorConfig, ProductionLocatorConfigError,
    LOCATOR_PUBLIC_KEYS_FILE_ENV,
};

#[cfg(test)]
mod tests;
