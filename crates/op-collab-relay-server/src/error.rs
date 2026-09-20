#[derive(Debug, thiserror::Error)]
pub enum RelayServerError {
    #[error("failed to bind relay listener at {address}: {source}")]
    Bind {
        address: std::net::SocketAddr,
        #[source]
        source: std::io::Error,
    },
    #[error("relay listener failed: {0}")]
    Accept(#[source] std::io::Error),
    #[error("relay configuration is invalid: {0}")]
    Config(#[from] crate::config::ConfigError),
    #[error(
        "no production relay authenticator is configured; \
         capability-only routing requires --allow-unauthenticated-dev"
    )]
    AuthenticationRequired,
}
