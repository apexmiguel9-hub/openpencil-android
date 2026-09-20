//! Typed failure for the Design-MD auto-generate session
//! (`design_md_host.rs`) — the worker that asks the selected chat model to
//! write a `design.md` for the open document.
//!
//! Style follows `op_orchestrator::OrchestratorError`: a plain enum plus a
//! hand-written `Display`, no `thiserror` and no new dependency. `Display`
//! reproduces the exact sentence the stringly code produced, so the
//! `openpencil-desktop: design.md auto-generate failed: …` stderr line is
//! unchanged byte for byte.
//!
//! What the enum adds is the difference between "the model answered with an
//! error", "the model answered with nothing usable", and "the worker never
//! answered at all" — three conditions the poll path received as one
//! indistinguishable `String` while it cleared the same `generating` flag for
//! all of them.
//!
//! [`DesignMdError::Provider`] carries the message from
//! `op_ai::chat_provider::ChatDelta::Error`, which is a `String` on a trait
//! this pass does not own; it is stored verbatim.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DesignMdError {
    /// The worker thread would not spawn. Delivered through the normal poll
    /// path so the panel's `generating` flag still clears.
    WorkerSpawn(String),
    /// The provider streamed a `ChatDelta::Error`. Its message is carried
    /// verbatim.
    Provider(String),
    /// The turn completed but the cleaned markdown was empty, so there is
    /// nothing to bind to the document.
    EmptyOutput,
    /// The worker dropped its sender without reporting — a panic in the
    /// provider bridge.
    WorkerVanished,
}

impl fmt::Display for DesignMdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DesignMdError::WorkerSpawn(message) => {
                write!(f, "design.md worker failed to start: {message}")
            }
            DesignMdError::Provider(message) => f.write_str(message),
            DesignMdError::EmptyOutput => f.write_str("design.md generation returned empty output"),
            DesignMdError::WorkerVanished => f.write_str("design.md generation worker vanished"),
        }
    }
}

impl std::error::Error for DesignMdError {}
