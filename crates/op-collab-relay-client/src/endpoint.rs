use std::net::IpAddr;

use thiserror::Error;
use url::{Host, Url};

const MAX_ENDPOINT_BYTES: usize = 2_048;
const RELAY_TUNNEL_PATH: &str = "/v1/tunnel";

/// A relay URL whose transport policy has already been validated.
#[derive(Clone, Eq, PartialEq)]
pub struct RelayEndpoint {
    url: Url,
}

impl RelayEndpoint {
    pub fn parse(value: &str) -> Result<Self, RelayEndpointError> {
        if value.is_empty() || value.len() > MAX_ENDPOINT_BYTES {
            return Err(RelayEndpointError::InvalidLength);
        }
        let url = Url::parse(value).map_err(|_| RelayEndpointError::InvalidUrl)?;
        if !url.username().is_empty() || url.password().is_some() {
            return Err(RelayEndpointError::CredentialsNotAllowed);
        }
        if url.fragment().is_some() {
            return Err(RelayEndpointError::FragmentNotAllowed);
        }
        if url.query().is_some() {
            return Err(RelayEndpointError::QueryNotAllowed);
        }
        if url.path() != RELAY_TUNNEL_PATH {
            return Err(RelayEndpointError::InvalidPath);
        }
        if url.host().is_none() {
            return Err(RelayEndpointError::MissingHost);
        }
        match url.scheme() {
            "wss" => {}
            "ws" if development_ws_allowed(&url) => {}
            "ws" => return Err(RelayEndpointError::InsecureEndpoint),
            _ => return Err(RelayEndpointError::UnsupportedScheme),
        }
        Ok(Self { url })
    }

    pub(crate) fn as_str(&self) -> &str {
        self.url.as_str()
    }

    pub fn is_encrypted(&self) -> bool {
        self.url.scheme() == "wss"
    }

    pub(crate) fn is_numeric_loopback(&self) -> bool {
        match self.url.host() {
            Some(Host::Ipv4(ip)) => IpAddr::V4(ip).is_loopback(),
            Some(Host::Ipv6(ip)) => IpAddr::V6(ip).is_loopback(),
            Some(Host::Domain(_)) | None => false,
        }
    }
}

impl TryFrom<&str> for RelayEndpoint {
    type Error = RelayEndpointError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl std::fmt::Debug for RelayEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RelayEndpoint")
            .field("url", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RelayEndpointError {
    #[error("relay endpoint length is invalid")]
    InvalidLength,
    #[error("relay endpoint URL is invalid")]
    InvalidUrl,
    #[error("relay endpoint credentials are not allowed")]
    CredentialsNotAllowed,
    #[error("relay endpoint fragments are not allowed")]
    FragmentNotAllowed,
    #[error("relay endpoint query parameters are not allowed")]
    QueryNotAllowed,
    #[error("relay endpoint path must be /v1/tunnel")]
    InvalidPath,
    #[error("relay endpoint has no host")]
    MissingHost,
    #[error("relay endpoint scheme is unsupported")]
    UnsupportedScheme,
    #[error("unencrypted relay endpoints are not allowed")]
    InsecureEndpoint,
}

fn development_ws_allowed(url: &Url) -> bool {
    if !cfg!(any(test, debug_assertions)) {
        return false;
    }
    RelayEndpoint { url: url.clone() }.is_numeric_loopback()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_shape_requires_websocket_tls() {
        assert!(RelayEndpoint::parse("wss://relay.example.cn/v1/tunnel").is_ok());
        assert_eq!(
            RelayEndpoint::parse("https://relay.example.cn/v1/tunnel"),
            Err(RelayEndpointError::UnsupportedScheme)
        );
        assert_eq!(
            RelayEndpoint::parse("ws://relay.example.cn/v1/tunnel"),
            Err(RelayEndpointError::InsecureEndpoint)
        );
    }

    #[test]
    fn debug_ws_accepts_only_numeric_loopback() {
        assert!(RelayEndpoint::parse("ws://127.0.0.1:30123/v1/tunnel").is_ok());
        assert!(RelayEndpoint::parse("ws://[::1]:30123/v1/tunnel").is_ok());
        assert_eq!(
            RelayEndpoint::parse("ws://localhost:30123/v1/tunnel"),
            Err(RelayEndpointError::InsecureEndpoint)
        );
    }

    #[test]
    fn numeric_loopback_check_rejects_remote_wss_and_domain_aliases() {
        assert!(RelayEndpoint::parse("wss://127.0.0.1/v1/tunnel")
            .unwrap()
            .is_numeric_loopback());
        assert!(!RelayEndpoint::parse("wss://relay.example/v1/tunnel")
            .unwrap()
            .is_numeric_loopback());
        assert!(!RelayEndpoint::parse("wss://localhost/v1/tunnel")
            .unwrap()
            .is_numeric_loopback());
    }

    #[test]
    fn endpoint_debug_is_redacted() {
        let endpoint = RelayEndpoint::parse("wss://secret-host.example/v1/tunnel").unwrap();
        let debug = format!("{endpoint:?}");
        assert!(!debug.contains("secret-host"));
        assert!(!debug.contains("/v1/tunnel"));
        assert!(debug.contains("[REDACTED]"));
    }

    #[test]
    fn endpoint_rejects_url_borne_secrets() {
        assert_eq!(
            RelayEndpoint::parse("wss://relay.example/v1/tunnel?token=secret"),
            Err(RelayEndpointError::QueryNotAllowed)
        );
        assert_eq!(
            RelayEndpoint::parse("wss://user:secret@relay.example/v1/tunnel"),
            Err(RelayEndpointError::CredentialsNotAllowed)
        );
        assert_eq!(
            RelayEndpoint::parse("wss://relay.example/cn-tunnel"),
            Err(RelayEndpointError::InvalidPath)
        );
    }
}
