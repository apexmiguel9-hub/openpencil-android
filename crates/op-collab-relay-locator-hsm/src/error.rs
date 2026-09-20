#[derive(Debug, thiserror::Error)]
pub enum SignerError {
    #[error("invalid signer configuration: {0}")]
    Config(String),
    #[error("secure file validation failed: {0}")]
    SecureFile(String),
    #[error("PKCS#11 operation failed: {0}")]
    Pkcs11(String),
    #[error("signing request was rejected")]
    Rejected,
    #[error("signer service is unavailable: {0}")]
    Unavailable(String),
    #[error("I/O operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON document is invalid: {0}")]
    Json(#[from] serde_json::Error),
}

pub type SignerResult<T> = Result<T, SignerError>;
