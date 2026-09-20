//! Bootstrap-source selection: the build-injected production hubs and the
//! environment override that wins over them.

use std::sync::Arc;

use op_collab_relay_protocol::RelayRegion;

use super::super::types::{CollabRuntimeError, CollabRuntimeFailure};
use super::{
    EnvironmentRelayBootstrapProvider, RelayBootstrapProvider, BOOTSTRAP_URL_ENV, MAX_URL_BYTES,
};

/// Built-in production hubs, injected at **build time** by the release
/// pipeline — the open-source tree deliberately carries no production
/// endpoint. Both hubs serve the same signed bootstrap document (verified
/// against the embedded union root), so the region preference is a
/// reachability choice, not a trust choice. A build without injection keeps
/// the public relay purely `OPENPENCIL_COLLAB_BOOTSTRAP_URL`-configured.
const BUILTIN_BOOTSTRAP_URL_CN: Option<&str> =
    option_env!("OPENPENCIL_BUILD_COLLAB_BOOTSTRAP_URL_CN");
const BUILTIN_BOOTSTRAP_URL_GLOBAL: Option<&str> =
    option_env!("OPENPENCIL_BUILD_COLLAB_BOOTSTRAP_URL_GLOBAL");

const fn builtin_bootstrap_url(region: RelayRegion) -> Option<&'static str> {
    match region {
        RelayRegion::Cn => BUILTIN_BOOTSTRAP_URL_CN,
        RelayRegion::Global => BUILTIN_BOOTSTRAP_URL_GLOBAL,
    }
}

/// Resolve the bootstrap provider: the environment override wins when set
/// (and stays fail-closed on an invalid value); otherwise the build-injected
/// hub for the preferred service region is used. Neither present means the
/// relay is not configured.
pub(in crate::runtime) fn bootstrap_provider(
    preferred_region: RelayRegion,
) -> Result<Arc<dyn RelayBootstrapProvider>, CollabRuntimeError> {
    let misconfigured = || CollabRuntimeError::new(CollabRuntimeFailure::RelayNotConfigured);
    match std::env::var_os(BOOTSTRAP_URL_ENV) {
        // A present-but-invalid override is a misconfiguration and stays a
        // hard error; silently falling back to a built-in hub would hide it.
        Some(raw) => {
            let raw = raw
                .to_str()
                .filter(|value| !value.is_empty() && value.len() <= MAX_URL_BYTES)
                .ok_or_else(misconfigured)?;
            let provider =
                EnvironmentRelayBootstrapProvider::new(raw).map_err(|_| misconfigured())?;
            Ok(Arc::new(provider))
        }
        None => {
            let url = builtin_bootstrap_url(preferred_region).ok_or_else(misconfigured)?;
            // An injected hub that fails the endpoint policy is a broken
            // build, not an outage: the value cannot become valid by waiting,
            // so it reports as unconfigured rather than temporarily
            // unavailable. `build.rs` refuses such a value at compile time;
            // this keeps the runtime copy honest if one ever reaches here.
            let provider =
                EnvironmentRelayBootstrapProvider::new(url).map_err(|_| misconfigured())?;
            Ok(Arc::new(provider))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::parse_bootstrap_endpoint_with_policy;
    use super::*;

    #[test]
    fn distinct_endpoints_get_distinct_cache_records() {
        use super::super::bootstrap_cache::endpoint_cache_file;
        let first = endpoint_cache_file("https://cn.example/api/v1/collaboration/bootstrap");
        let second = endpoint_cache_file("https://global.example/api/v1/collaboration/bootstrap");
        // Distinct files keep each hub's anti-rollback generation floor
        // intact across region switches; the URL itself must not leak into
        // the file name.
        assert_ne!(first, second);
        for name in [&first, &second] {
            assert!(name.starts_with("collaboration-bootstrap-v1-"));
            assert!(name.ends_with(".json"));
            assert!(!name.contains("example"));
        }
    }

    /// When the release pipeline injects hub URLs, they must be distinct and
    /// satisfy the strict production endpoint policy even with the
    /// development-http escape hatch disabled. An uninjected (open-source)
    /// build has nothing to validate and must instead stay unconfigured.
    #[test]
    fn injected_bootstrap_urls_pass_the_endpoint_policy_or_stay_absent() {
        match (BUILTIN_BOOTSTRAP_URL_CN, BUILTIN_BOOTSTRAP_URL_GLOBAL) {
            (Some(cn), Some(global)) => {
                assert_ne!(cn, global);
                for url in [cn, global] {
                    let (endpoint, development_http) =
                        parse_bootstrap_endpoint_with_policy(url, false).unwrap();
                    assert_eq!(endpoint.scheme(), "https");
                    assert!(!development_http);
                }
            }
            // Partial injection is a broken release configuration: one
            // region would silently report "not configured".
            (Some(_), None) | (None, Some(_)) => {
                panic!("both hub URLs must be injected together")
            }
            (None, None) => {
                for region in [RelayRegion::Cn, RelayRegion::Global] {
                    assert!(builtin_bootstrap_url(region).is_none());
                }
            }
        }
    }
}
