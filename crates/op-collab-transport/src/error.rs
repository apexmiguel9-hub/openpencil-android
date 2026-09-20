use std::{fmt, time::Duration};

use crate::TransferClass;

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ConfigError {
    #[error("transport configuration field `{field}` is outside its safe range")]
    InvalidValue { field: &'static str },
}

#[derive(Debug, thiserror::Error)]
pub enum RecordError {
    #[error("record IO failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("record length {actual} is outside 1..={maximum}")]
    InvalidLength { actual: usize, maximum: usize },
    #[error("Noise record encryption failed: {0}")]
    Encrypt(#[source] snow::Error),
    #[error("Noise record authentication failed: {0}")]
    Decrypt(#[source] snow::Error),
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ChunkError {
    #[error("authenticated chunk header has invalid length {0}")]
    HeaderLength(usize),
    #[error("authenticated chunk header version {0} is unsupported")]
    UnsupportedVersion(u8),
    #[error("authenticated chunk header contains non-zero reserved bits")]
    ReservedBits,
    #[error("authenticated chunk class {0} is unknown")]
    UnknownClass(u8),
    #[error("transfer id must be non-zero")]
    ZeroTransferId,
    #[error("transfer length {actual} exceeds {class:?} limit {maximum}")]
    TransferTooLarge {
        class: TransferClass,
        actual: usize,
        maximum: usize,
    },
    #[error("transfer length must be non-zero")]
    EmptyTransfer,
    #[error("chunk count {actual} does not match expected {expected}")]
    InvalidChunkCount { actual: u32, expected: u32 },
    #[error("chunk index {actual} does not match expected {expected}")]
    UnexpectedChunkIndex { actual: u32, expected: u32 },
    #[error("chunk belongs to a different in-flight transfer")]
    InFlightMismatch,
    #[error("transfer id {actual} is not newer than completed id {previous}")]
    ReplayedTransferId { actual: u64, previous: u64 },
    #[error("chunk payload length {actual} does not match expected {expected}")]
    InvalidPayloadLength { actual: usize, expected: usize },
    #[error("transfer timed out after {0:?}")]
    TimedOut(Duration),
    #[error("inbound reassembly budget rejected a {class:?} transfer of {requested} bytes")]
    InboundBudgetExhausted {
        class: TransferClass,
        requested: usize,
    },
    #[error("transfer length arithmetic overflow")]
    LengthOverflow,
}

#[derive(Debug, thiserror::Error)]
pub enum PreludeError {
    #[error("server prelude IO failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("server prelude is truncated or malformed")]
    Malformed,
    #[error("server prelude magic is invalid")]
    Magic,
    #[error("transport protocol version {actual} is unsupported; expected {expected}")]
    UnsupportedTransportVersion { actual: u16, expected: u16 },
    #[error("collaboration protocol version {actual} is unsupported; expected {expected}")]
    UnsupportedCollabVersion { actual: u16, expected: u16 },
    #[error("server prelude field `{field}` is empty or too long")]
    InvalidIdentifier { field: &'static str },
    #[error("server prelude is {actual} bytes; maximum is {maximum}")]
    TooLarge { actual: usize, maximum: usize },
    #[error("server prelude is not valid UTF-8")]
    Utf8,
}

#[derive(Debug, thiserror::Error)]
pub enum NoiseTransportError {
    #[error("Noise configuration failed: {0}")]
    Configuration(#[source] snow::Error),
    #[error("Noise handshake IO failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Noise handshake failed: {0}")]
    Handshake(#[source] snow::Error),
    #[error("Noise handshake frame has invalid length {0}")]
    HandshakeFrameLength(usize),
    #[error("Noise handshake did not expose a 32-byte remote static key")]
    MissingRemoteStatic,
    #[error("Noise handshake lost its pending-connection seat")]
    PendingSeatUnavailable,
}

#[derive(Debug, thiserror::Error)]
pub enum FrameTransportError {
    #[error("collaboration frame codec rejected the transfer: {0}")]
    Protocol(#[from] op_collab::ProtocolError),
    #[error("encoded {class:?} frame is {actual} bytes; maximum is {maximum}")]
    TransferTooLarge {
        class: TransferClass,
        actual: usize,
        maximum: usize,
    },
    #[error("declared {declared:?} transfer carries a {actual:?} frame")]
    ClassMismatch {
        declared: TransferClass,
        actual: TransferClass,
    },
    #[error("pre-encoded frame was validated under different wire limits")]
    WireLimitsMismatch,
    #[error("sensitive Ticket transfer cannot use shared or coalescing storage")]
    SensitiveTransferNotShareable,
    #[error("Ticket transfer does not carry a RenewTicket frame")]
    TicketFrameExpected,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum AdmissionError {
    #[error("ticket verification failed")]
    Verification,
    #[error("ticket issuer does not match the authenticated session owner")]
    WrongIssuer,
    #[error("ticket subject does not match the authenticated session owner")]
    WrongSubject,
    #[error("ticket X25519 binding does not match the Noise remote static")]
    StaticKeyMismatch,
    #[error("ticket has expired")]
    Expired,
    #[error("ticket renewal changed the authenticated identity")]
    RenewalIdentityChanged,
    #[error("ticket renewal did not extend the expiry")]
    RenewalDidNotExtend,
    #[error("admission state transition is invalid")]
    InvalidState,
    #[error("the admission deadline expired")]
    AdmissionTimedOut,
    #[error("the authenticated ticket reached its expiry")]
    TicketExpired,
    #[error("admission payload is malformed")]
    Malformed,
    #[error("admission payload is {actual} bytes; maximum is {maximum}")]
    TooLarge { actual: usize, maximum: usize },
    #[error("admission field `{field}` is empty or too long")]
    InvalidIdentifier { field: &'static str },
}

#[derive(Debug, thiserror::Error)]
pub enum KeyStoreError {
    #[error("secure file key storage is unsupported on this platform")]
    UnsupportedPlatform,
    #[error("the operating-system credential store is unavailable")]
    PlatformStoreUnavailable,
    #[error("the operating-system credential store is locked or access was denied")]
    PlatformStoreAccessDenied,
    #[error("the operating-system credential store operation failed")]
    PlatformStoreFailure,
    #[error("the operating-system credential store entry is malformed or ambiguous")]
    UnsafePlatformEntry,
    #[error("device static key IO failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("device static key directory is a symlink or not a directory")]
    UnsafeDirectory,
    #[error("device static key path is a symlink or not a regular file")]
    UnsafeFile,
    #[error("device static key has invalid length {0}")]
    InvalidLength(usize),
    #[error("device static key is all zero")]
    AllZero,
    #[error("X25519 peer key produces a non-contributory shared secret")]
    NonContributoryPeer,
    #[error("secure random generation failed")]
    Random,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum QueueError {
    #[error("bounded transfer queue is full")]
    Full,
    #[error("queued byte budget would be exceeded")]
    ByteBudget,
    #[error("queue item is larger than the configured byte budget")]
    ItemTooLarge,
    #[error("shared queue budget is unavailable")]
    Unavailable,
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error(transparent)]
    InvalidConfig(#[from] ConfigError),
    #[error("TCP connection failed: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Noise(#[from] NoiseTransportError),
    #[error(transparent)]
    Prelude(#[from] PreludeError),
    #[error(transparent)]
    Record(#[from] RecordError),
    #[error(transparent)]
    Chunk(#[from] ChunkError),
    #[error(transparent)]
    Frame(#[from] FrameTransportError),
    #[error(transparent)]
    Admission(#[from] AdmissionError),
    #[error(transparent)]
    Queue(#[from] QueueError),
    #[error("expected {expected:?} transfer, received {actual:?}")]
    UnexpectedClass {
        expected: TransferClass,
        actual: TransferClass,
    },
    #[error("authenticated inbound {0:?} transfer is forbidden in this connection direction")]
    ForbiddenInboundClass(TransferClass),
    #[error("outbound transfer id space is exhausted")]
    TransferIdExhausted,
    #[error("connection rate limit exceeded")]
    RateLimited,
    #[error("the server prelude discovery id does not match the selected endpoint")]
    DiscoveryIdMismatch,
    #[error("the connection was idle past its configured deadline")]
    IdleTimeout,
    #[error("a partial ciphertext record made no read progress before its deadline")]
    ReadTimeout,
    #[error("an encrypted record made no write progress before its deadline")]
    WriteTimeout,
    #[error("the secure connection is no longer usable after a fatal error")]
    ConnectionClosed,
}

/// Credential-free phase classification for a [`RuntimeError`].
///
/// The variants deliberately carry no protocol fields, identifiers, lengths,
/// endpoints, third-party errors, or cryptographic material. When the failure
/// originated in IO, [`RuntimeErrorDiagnostic::io_kind`] provides the only
/// additional detail that is safe to emit in a diagnostic log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RuntimeErrorPhase {
    Configuration,
    Tcp,
    Prelude,
    Noise,
    Record,
    Chunk,
    Frame,
    Admission,
    Queue,
    Transfer,
    RateLimit,
    Discovery,
    Idle,
    Read,
    Write,
    Closed,
}

impl fmt::Display for RuntimeErrorPhase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Configuration => "Configuration",
            Self::Tcp => "Tcp",
            Self::Prelude => "Prelude",
            Self::Noise => "Noise",
            Self::Record => "Record",
            Self::Chunk => "Chunk",
            Self::Frame => "Frame",
            Self::Admission => "Admission",
            Self::Queue => "Queue",
            Self::Transfer => "Transfer",
            Self::RateLimit => "RateLimit",
            Self::Discovery => "Discovery",
            Self::Idle => "Idle",
            Self::Read => "Read",
            Self::Write => "Write",
            Self::Closed => "Closed",
        })
    }
}

/// A log-safe diagnostic derived from a [`RuntimeError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct RuntimeErrorDiagnostic {
    pub phase: RuntimeErrorPhase,
    pub io_kind: Option<std::io::ErrorKind>,
}

impl fmt::Display for RuntimeErrorDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.io_kind {
            Some(kind) => write!(formatter, "{}({kind:?})", self.phase),
            None => self.phase.fmt(formatter),
        }
    }
}

impl RuntimeError {
    /// Returns a credential-free diagnostic suitable for local logging.
    pub fn safe_diagnostic(&self) -> RuntimeErrorDiagnostic {
        match self {
            Self::InvalidConfig(_) => diagnostic(RuntimeErrorPhase::Configuration, None),
            Self::Io(error) => diagnostic(RuntimeErrorPhase::Tcp, Some(error.kind())),
            Self::Noise(error) => match error {
                NoiseTransportError::Configuration(_) => diagnostic(RuntimeErrorPhase::Noise, None),
                NoiseTransportError::Io(error) => {
                    diagnostic(RuntimeErrorPhase::Noise, Some(error.kind()))
                }
                NoiseTransportError::Handshake(_) => diagnostic(RuntimeErrorPhase::Noise, None),
                NoiseTransportError::HandshakeFrameLength(_) => {
                    diagnostic(RuntimeErrorPhase::Noise, None)
                }
                NoiseTransportError::MissingRemoteStatic => {
                    diagnostic(RuntimeErrorPhase::Noise, None)
                }
                NoiseTransportError::PendingSeatUnavailable => {
                    diagnostic(RuntimeErrorPhase::Noise, None)
                }
            },
            Self::Prelude(error) => match error {
                PreludeError::Io(error) => {
                    diagnostic(RuntimeErrorPhase::Prelude, Some(error.kind()))
                }
                PreludeError::Malformed => diagnostic(RuntimeErrorPhase::Prelude, None),
                PreludeError::Magic => diagnostic(RuntimeErrorPhase::Prelude, None),
                PreludeError::UnsupportedTransportVersion { .. } => {
                    diagnostic(RuntimeErrorPhase::Prelude, None)
                }
                PreludeError::UnsupportedCollabVersion { .. } => {
                    diagnostic(RuntimeErrorPhase::Prelude, None)
                }
                PreludeError::InvalidIdentifier { .. } => {
                    diagnostic(RuntimeErrorPhase::Prelude, None)
                }
                PreludeError::TooLarge { .. } => diagnostic(RuntimeErrorPhase::Prelude, None),
                PreludeError::Utf8 => diagnostic(RuntimeErrorPhase::Prelude, None),
            },
            Self::Record(error) => match error {
                RecordError::Io(error) => diagnostic(RuntimeErrorPhase::Record, Some(error.kind())),
                RecordError::InvalidLength { .. } => diagnostic(RuntimeErrorPhase::Record, None),
                RecordError::Encrypt(_) => diagnostic(RuntimeErrorPhase::Record, None),
                RecordError::Decrypt(_) => diagnostic(RuntimeErrorPhase::Record, None),
            },
            Self::Chunk(_) => diagnostic(RuntimeErrorPhase::Chunk, None),
            Self::Frame(_) => diagnostic(RuntimeErrorPhase::Frame, None),
            Self::Admission(error) => match error {
                AdmissionError::Verification => diagnostic(RuntimeErrorPhase::Admission, None),
                AdmissionError::WrongIssuer => diagnostic(RuntimeErrorPhase::Admission, None),
                AdmissionError::WrongSubject => diagnostic(RuntimeErrorPhase::Admission, None),
                AdmissionError::StaticKeyMismatch => diagnostic(RuntimeErrorPhase::Admission, None),
                AdmissionError::Expired => diagnostic(RuntimeErrorPhase::Admission, None),
                AdmissionError::RenewalIdentityChanged => {
                    diagnostic(RuntimeErrorPhase::Admission, None)
                }
                AdmissionError::RenewalDidNotExtend => {
                    diagnostic(RuntimeErrorPhase::Admission, None)
                }
                AdmissionError::InvalidState => diagnostic(RuntimeErrorPhase::Admission, None),
                AdmissionError::AdmissionTimedOut => diagnostic(RuntimeErrorPhase::Admission, None),
                AdmissionError::TicketExpired => diagnostic(RuntimeErrorPhase::Admission, None),
                AdmissionError::Malformed => diagnostic(RuntimeErrorPhase::Admission, None),
                AdmissionError::TooLarge { .. } => diagnostic(RuntimeErrorPhase::Admission, None),
                AdmissionError::InvalidIdentifier { .. } => {
                    diagnostic(RuntimeErrorPhase::Admission, None)
                }
            },
            Self::Queue(_) => diagnostic(RuntimeErrorPhase::Queue, None),
            Self::UnexpectedClass { .. } => diagnostic(RuntimeErrorPhase::Transfer, None),
            Self::ForbiddenInboundClass(_) => diagnostic(RuntimeErrorPhase::Transfer, None),
            Self::TransferIdExhausted => diagnostic(RuntimeErrorPhase::Transfer, None),
            Self::RateLimited => diagnostic(RuntimeErrorPhase::RateLimit, None),
            Self::DiscoveryIdMismatch => diagnostic(RuntimeErrorPhase::Discovery, None),
            Self::IdleTimeout => diagnostic(RuntimeErrorPhase::Idle, None),
            Self::ReadTimeout => diagnostic(RuntimeErrorPhase::Read, None),
            Self::WriteTimeout => diagnostic(RuntimeErrorPhase::Write, None),
            Self::ConnectionClosed => diagnostic(RuntimeErrorPhase::Closed, None),
        }
    }
}

const fn diagnostic(
    phase: RuntimeErrorPhase,
    io_kind: Option<std::io::ErrorKind>,
) -> RuntimeErrorDiagnostic {
    RuntimeErrorDiagnostic { phase, io_kind }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_diagnostic_covers_every_runtime_error_variant() {
        let cases = [
            (
                RuntimeError::InvalidConfig(ConfigError::InvalidValue { field: "sensitive" }),
                RuntimeErrorPhase::Configuration,
            ),
            (
                RuntimeError::Io(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "sensitive TCP context",
                )),
                RuntimeErrorPhase::Tcp,
            ),
            (
                RuntimeError::Noise(NoiseTransportError::PendingSeatUnavailable),
                RuntimeErrorPhase::Noise,
            ),
            (
                RuntimeError::Prelude(PreludeError::Malformed),
                RuntimeErrorPhase::Prelude,
            ),
            (
                RuntimeError::Record(RecordError::InvalidLength {
                    actual: 65_537,
                    maximum: 65_536,
                }),
                RuntimeErrorPhase::Record,
            ),
            (
                RuntimeError::Chunk(ChunkError::ReplayedTransferId {
                    actual: 9_876_543_210,
                    previous: 1_234_567_890,
                }),
                RuntimeErrorPhase::Chunk,
            ),
            (
                RuntimeError::Frame(FrameTransportError::SensitiveTransferNotShareable),
                RuntimeErrorPhase::Frame,
            ),
            (
                RuntimeError::Admission(AdmissionError::Verification),
                RuntimeErrorPhase::Admission,
            ),
            (
                RuntimeError::Queue(QueueError::Full),
                RuntimeErrorPhase::Queue,
            ),
            (
                RuntimeError::UnexpectedClass {
                    expected: TransferClass::Ticket,
                    actual: TransferClass::Control,
                },
                RuntimeErrorPhase::Transfer,
            ),
            (
                RuntimeError::ForbiddenInboundClass(TransferClass::Snapshot),
                RuntimeErrorPhase::Transfer,
            ),
            (
                RuntimeError::TransferIdExhausted,
                RuntimeErrorPhase::Transfer,
            ),
            (RuntimeError::RateLimited, RuntimeErrorPhase::RateLimit),
            (
                RuntimeError::DiscoveryIdMismatch,
                RuntimeErrorPhase::Discovery,
            ),
            (RuntimeError::IdleTimeout, RuntimeErrorPhase::Idle),
            (RuntimeError::ReadTimeout, RuntimeErrorPhase::Read),
            (RuntimeError::WriteTimeout, RuntimeErrorPhase::Write),
            (RuntimeError::ConnectionClosed, RuntimeErrorPhase::Closed),
        ];

        for (error, expected_phase) in cases {
            let actual = error.safe_diagnostic();
            assert_eq!(actual.phase, expected_phase);
            assert_eq!(
                actual.io_kind.is_some(),
                expected_phase == RuntimeErrorPhase::Tcp
            );
        }
    }

    #[test]
    fn safe_diagnostic_preserves_only_the_four_io_kinds() {
        let cases = [
            (
                RuntimeError::Io(secret_io(
                    std::io::ErrorKind::PermissionDenied,
                    "TCP_SECRET",
                )),
                RuntimeErrorPhase::Tcp,
                std::io::ErrorKind::PermissionDenied,
            ),
            (
                RuntimeError::Noise(NoiseTransportError::Io(secret_io(
                    std::io::ErrorKind::ConnectionReset,
                    "NOISE_SECRET",
                ))),
                RuntimeErrorPhase::Noise,
                std::io::ErrorKind::ConnectionReset,
            ),
            (
                RuntimeError::Prelude(PreludeError::Io(secret_io(
                    std::io::ErrorKind::UnexpectedEof,
                    "PRELUDE_SECRET",
                ))),
                RuntimeErrorPhase::Prelude,
                std::io::ErrorKind::UnexpectedEof,
            ),
            (
                RuntimeError::Record(RecordError::Io(secret_io(
                    std::io::ErrorKind::BrokenPipe,
                    "RECORD_SECRET",
                ))),
                RuntimeErrorPhase::Record,
                std::io::ErrorKind::BrokenPipe,
            ),
        ];

        for (error, phase, io_kind) in cases {
            assert_eq!(
                error.safe_diagnostic(),
                RuntimeErrorDiagnostic {
                    phase,
                    io_kind: Some(io_kind),
                }
            );
        }
    }

    #[test]
    fn safe_diagnostic_formats_do_not_leak_sensitive_error_details() {
        let secrets = [
            "TCP_SECRET",
            "NOISE_SECRET",
            "PRELUDE_SECRET",
            "RECORD_SECRET",
        ];
        let errors = [
            RuntimeError::Io(secret_io(std::io::ErrorKind::Other, secrets[0])),
            RuntimeError::Noise(NoiseTransportError::Io(secret_io(
                std::io::ErrorKind::Other,
                secrets[1],
            ))),
            RuntimeError::Prelude(PreludeError::Io(secret_io(
                std::io::ErrorKind::Other,
                secrets[2],
            ))),
            RuntimeError::Record(RecordError::Io(secret_io(
                std::io::ErrorKind::Other,
                secrets[3],
            ))),
        ];

        for (error, secret) in errors.into_iter().zip(secrets) {
            let diagnostic = error.safe_diagnostic();
            let debug = format!("{diagnostic:?}");
            let display = diagnostic.to_string();
            assert!(!debug.contains(secret));
            assert!(!display.contains(secret));
            assert!(debug.contains("Other"));
            assert!(display.contains("Other"));
        }
    }

    #[test]
    fn safe_diagnostic_drops_lengths_ids_and_identifier_values() {
        let errors = [
            RuntimeError::InvalidConfig(ConfigError::InvalidValue {
                field: "SECRET_FIELD_NAME",
            }),
            RuntimeError::Prelude(PreludeError::TooLarge {
                actual: 4_294_967_291,
                maximum: 4_294_967_290,
            }),
            RuntimeError::Chunk(ChunkError::ReplayedTransferId {
                actual: 9_876_543_210,
                previous: 1_234_567_890,
            }),
        ];

        for error in errors {
            let diagnostic = error.safe_diagnostic();
            let rendered = format!("{diagnostic:?} {diagnostic}");
            assert!(!rendered.contains("SECRET_FIELD_NAME"));
            assert!(!rendered.contains("4294967291"));
            assert!(!rendered.contains("4294967290"));
            assert!(!rendered.contains("9876543210"));
            assert!(!rendered.contains("1234567890"));
        }
    }

    #[test]
    fn static_key_mismatch_is_reduced_to_the_admission_phase() {
        assert_eq!(
            RuntimeError::Admission(AdmissionError::StaticKeyMismatch).safe_diagnostic(),
            RuntimeErrorDiagnostic {
                phase: RuntimeErrorPhase::Admission,
                io_kind: None,
            }
        );
    }

    fn secret_io(kind: std::io::ErrorKind, message: &'static str) -> std::io::Error {
        std::io::Error::new(kind, message)
    }
}
