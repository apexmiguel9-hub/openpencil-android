//! Typed failures for the `batch_design` operations DSL: the insert-forest
//! parser in `batch_design_dsl.rs` ([`InsertDslError`]) and the
//! direct-vs-insert front door in `batch_design.rs::parse_operations`
//! ([`OperationsError`]).
//!
//! Style follows `ProgramError`: plain enums plus hand-written `Display`
//! impls, no `thiserror` and no new dependency. Every variant's `Display`
//! reproduces the exact sentence the stringly-typed parsers produced,
//! because those sentences ship verbatim to the model as the `batch_design`
//! `InvalidArgument` payload (and several tests assert on them).
//!
//! What the enums buy over `String` is the CLASSIFICATION: [`OperationsError`]
//! now says WHICH parser refused a program — the single-line direct-op
//! parser (already typed as `ProgramError`) or the multi-line insert-forest
//! parser — instead of collapsing both into one opaque message; and
//! [`InsertDslError`] separates grammar faults from JSON faults from
//! forest-shape faults (cycles, non-containers, multi-parent targets).

use std::fmt;

use super::batch_program_error::ProgramError;

/// Everything the `I(parent, node)` insert-forest parser can refuse on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InsertDslError {
    /// The program contained no operation lines at all.
    NoOperations,
    /// Two operations bound the same name.
    DuplicateBinding(String),
    /// An operation's node body is not valid JSON.
    InvalidNodeJson { binding: String, detail: String },
    /// An operation's node body parsed as JSON but is not a `PenNode`.
    InvalidPenNode { binding: String, detail: String },
    /// An operation named itself as its own parent.
    SelfParent(String),
    /// Operations referenced more than one pre-existing document node as a
    /// parent; one call inserts under a single existing parent.
    MultipleExistingParents,
    /// Every operation is a child of another operation — no root to insert.
    NoRootInsert,
    /// The parent references form a cycle.
    ParentCycle,
    /// An operation is a child of a binding whose node cannot hold children.
    NotAContainer(String),
    /// The `name =` prefix is not a legal binding identifier.
    InvalidBinding(String),
    /// The line is not an `I(parent, node)` call.
    UnsupportedOperation(String),
    /// `I()` was called without both a parent and a node body.
    MissingArguments(String),
    /// `I()`'s node body is empty.
    EmptyNodeJson(String),
    /// A quoted parent reference is not a valid JSON string.
    InvalidQuotedParentRef(String),
}

impl fmt::Display for InsertDslError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InsertDslError::NoOperations => {
                f.write_str("operations must contain at least one I(parent, node) operation")
            }
            InsertDslError::DuplicateBinding(binding) => {
                write!(f, "duplicate binding {binding:?}")
            }
            InsertDslError::InvalidNodeJson { binding, detail } => {
                write!(f, "{binding}: invalid node JSON: {detail}")
            }
            InsertDslError::InvalidPenNode { binding, detail } => {
                write!(f, "{binding}: invalid PenNode payload: {detail}")
            }
            InsertDslError::SelfParent(binding) => {
                write!(f, "{binding} cannot be inserted under itself")
            }
            InsertDslError::MultipleExistingParents => {
                f.write_str("operations can target only one existing parent per call")
            }
            InsertDslError::NoRootInsert => {
                f.write_str("operations must include at least one root insert")
            }
            InsertDslError::ParentCycle => f.write_str("operations contain a parent cycle"),
            InsertDslError::NotAContainer(binding) => write!(
                f,
                "binding {binding:?} cannot receive children because it is not a container"
            ),
            InsertDslError::InvalidBinding(binding) => write!(f, "invalid binding {binding:?}"),
            InsertDslError::UnsupportedOperation(binding) => {
                write!(
                    f,
                    "{binding}: only I(parent, node) operations are supported"
                )
            }
            InsertDslError::MissingArguments(binding) => {
                write!(f, "{binding}: I() requires parent and node JSON")
            }
            InsertDslError::EmptyNodeJson(binding) => write!(f, "{binding}: node JSON is empty"),
            InsertDslError::InvalidQuotedParentRef(detail) => {
                write!(f, "invalid quoted parent ref: {detail}")
            }
        }
    }
}

impl std::error::Error for InsertDslError {}

/// Which of the two `operations` parsers refused the program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OperationsError {
    /// The single-line direct-op parser (`U`/`D`/`M`/`C`/`R`/`G`) refused.
    Direct(ProgramError),
    /// The multi-line `I(parent, node)` insert-forest parser refused.
    Insert(InsertDslError),
}

impl fmt::Display for OperationsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OperationsError::Direct(error) => error.fmt(f),
            OperationsError::Insert(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for OperationsError {}

impl From<ProgramError> for OperationsError {
    fn from(error: ProgramError) -> Self {
        OperationsError::Direct(error)
    }
}

impl From<InsertDslError> for OperationsError {
    fn from(error: InsertDslError) -> Self {
        OperationsError::Insert(error)
    }
}
