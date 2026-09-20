use op_collab_relay_protocol::RelayProtocolError;

#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
pub enum RelayLocatorSignerError {
    #[error("locator signing service is unavailable")]
    Unavailable,
    #[error("locator signing request was rejected")]
    Rejected,
    #[error("locator signing service failed")]
    Internal,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum RelayLocatorIssueError {
    #[error(transparent)]
    Protocol(#[from] RelayProtocolError),
    #[error("requested relay locator lifetime must be non-zero")]
    ZeroLifetime,
    #[error("requested relay locator lifetime {actual} exceeds maximum {maximum}")]
    LifetimeTooLong { actual: u64, maximum: u64 },
    #[error("owner publish request has length {actual}; expected exactly {expected} bytes")]
    InvalidRequestLength { actual: usize, expected: usize },
    #[error("owner publish request version {actual} is unsupported; expected {expected}")]
    UnsupportedRequestVersion { actual: u8, expected: u8 },
    #[error("owner publish request padding must be zero")]
    NonZeroRequestPadding,
    #[error("owner publish request contains invalid UTF-8")]
    InvalidRequestUtf8,
    #[error("system time is before the Unix epoch")]
    ClockBeforeUnixEpoch,
    #[error("current Unix time must be non-zero")]
    ZeroCurrentTime,
    #[error("owner ticket binding was rejected")]
    OwnerBindingRejected,
    #[error("relay locator expiry overflowed")]
    ExpiryOverflow,
    #[error("secure random output could not produce a valid non-zero generation")]
    InvalidRandomGeneration,
    #[error(transparent)]
    Signer(#[from] RelayLocatorSignerError),
    #[error("signed locator response does not match pending owner publish field {field}")]
    ResponseBindingMismatch { field: &'static str },
    #[error("relay locator control-plane endpoint is invalid")]
    InvalidControlPlaneEndpoint,
    #[error("relay locator control-plane endpoint is not permitted")]
    InsecureControlPlaneEndpoint,
    #[error("relay locator publish credential is invalid")]
    InvalidPublishCredential,
    #[error("relay locator control-plane transport is unavailable")]
    PublishTransportUnavailable,
    /// The control plane refused the collaboration ticket (401/403). Kept
    /// apart from the generic rejection so the caller can say "sign in
    /// again" instead of "the relay is temporarily unavailable" — the
    /// latter never becomes true by waiting.
    #[error("relay locator control-plane rejected the collaboration ticket")]
    PublishUnauthorized,
    /// The control plane is shedding load (429). Retrying later genuinely
    /// helps here, which is what separates it from the two above.
    #[error("relay locator control-plane rate limited the request")]
    PublishRateLimited,
    #[error("relay locator control-plane response was rejected")]
    PublishResponseRejected,
    #[error("relay locator control-plane response exceeds {maximum} bytes")]
    PublishResponseTooLarge { maximum: usize },
}
