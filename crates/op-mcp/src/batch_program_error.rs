//! Typed per-line failures for the `batch_design` DSL program executor
//! (`batch_program.rs`).
//!
//! Style follows `op_orchestrator::OrchestratorError`: a plain enum plus a
//! hand-written `Display`, no `thiserror` and no new dependency. Each
//! variant's `Display` reproduces the exact sentence the stringly-typed
//! executor produced, because those sentences ship verbatim to the model in
//! the envelope's `errors[]` array (and several tests assert on them).
//!
//! What the enum buys over `String` is the CLASSIFICATION: the executor —
//! and anything downstream, e.g. a future retry ladder in `program_gen.rs` —
//! can now tell a grammar mistake from a missing node from an id-space
//! exhaustion without pattern-matching prose.
//!
//! `batch_direct_ops.rs` (the single-line `U`/`D`/`M`/`C`/`R`/`G` parser)
//! reports this enum too, so the two paths classify the same failure the
//! same way. It used to hand the executor a bare `String` that
//! `batch_program.rs` re-labelled as [`ProgramError::Rejected`] wholesale —
//! which mislabelled its grammar and JSON failures as protocol rejections.
//! Those now arrive already classified (`Syntax` / `Json` / `InvalidNode` /
//! `InvalidValue` / `ValueOutOfRange` / `Rejected`), and `Rejected` is once
//! again reserved for a genuine semantic refusal.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProgramError {
    /// The line matched none of the DSL operation grammars.
    UnparsableLine(String),
    /// An operation's argument list is malformed, has the wrong arity, or a
    /// scalar argument has the wrong JSON type.
    Syntax(String),
    /// A JSON argument could not be parsed even after the lenient
    /// agent-typo repair pipeline, or parsed to the wrong JSON kind.
    Json(String),
    /// A body parsed as JSON but is not a valid `PenNode`.
    InvalidNode(String),
    /// A referenced node, path, kit, or component does not exist.
    NotFound(String),
    /// A scalar argument parsed but its VALUE is outside the accepted set
    /// (a bad hex spelling, a mode/placement keyword that is not one of the
    /// allowed literals). Distinct from `Syntax`, which is about shape, and
    /// from `Rejected`, which is about the design protocol.
    InvalidValue(String),
    /// A numeric argument parsed but does not fit the command's field
    /// (`i32` geometry, a `usize` sibling index).
    ValueOutOfRange(String),
    /// The operation parsed and resolved, but a structural / semantic rule
    /// of the design protocol refuses it (placement contracts, sizing
    /// requirements, layout preconditions).
    Rejected(String),
    /// The simulated apply refused the command — the host would refuse it
    /// too, so the line cannot ship.
    ApplyRejected(String),
    /// An operation that must yield a node yielded none. The payload is the
    /// operation label as it appears in the message (`Insert`, `Copy`,
    /// `Replace`, `G()`).
    ProducedNoNode(&'static str),
    /// The document's node id space is exhausted, so no fresh id can be
    /// minted for the remapped subtree.
    IdSpaceExhausted,
}

impl fmt::Display for ProgramError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProgramError::UnparsableLine(line) => write!(f, "Cannot parse operation: {line}"),
            ProgramError::Syntax(m)
            | ProgramError::Json(m)
            | ProgramError::InvalidNode(m)
            | ProgramError::NotFound(m)
            | ProgramError::InvalidValue(m)
            | ProgramError::ValueOutOfRange(m)
            | ProgramError::Rejected(m)
            | ProgramError::ApplyRejected(m) => f.write_str(m),
            ProgramError::ProducedNoNode(op) => write!(f, "{op} produced no node"),
            ProgramError::IdSpaceExhausted => f.write_str("node id space exhausted"),
        }
    }
}

impl std::error::Error for ProgramError {}

/// Boundary bridge kept for the tool-outcome layer, which still hands the
/// model a `String` payload. Nothing in-crate needs it to `?` any more —
/// `batch_design.rs`'s `parse_operations` now carries
/// `OperationsError`, which wraps this enum directly — but the conversion
/// stays available (and `Display`-faithful) so a new boundary can render a
/// `ProgramError` without reaching for `to_string()` by hand.
impl From<ProgramError> for String {
    fn from(error: ProgramError) -> String {
        error.to_string()
    }
}
