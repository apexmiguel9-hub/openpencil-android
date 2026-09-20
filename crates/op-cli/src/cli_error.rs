//! The one error type every `op` subcommand fails with.
//!
//! Style mirrors `op_orchestrator::OrchestratorError`: a plain enum plus a
//! hand-written `Display` — no `thiserror`, no extra dependency. The
//! variants name the REAL failure kinds the CLI can hit (bad arguments, a
//! malformed payload, filesystem trouble, a daemon that will not come up, a
//! transport failure, a tool that reported an error, a document that will not
//! load). Each variant's payload is the exact user-facing sentence, so
//! `Display` is transparent: `main` still prints `op: {e}` byte-for-byte as
//! before and the message-shape tests keep passing.
//!
//! The classification is what's new and load-bearing — a caller (or a future
//! exit-code table) can now branch on the KIND instead of grepping the text.

use crate::skill_install_error::SkillInstallError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CliError {
    /// Missing, malformed, or mutually exclusive command-line arguments.
    /// The payload is the usage line or validation sentence shown to the user.
    Usage(String),
    /// A JSON payload (argument, file, or server reply) failed to parse,
    /// failed to serialize, or had the wrong shape.
    Payload(String),
    /// A filesystem read / write / create failed.
    Io(String),
    /// Locating, spawning, or waiting for the OpenPencil desktop / daemon
    /// process failed, or a conflicting server already owns the port.
    Daemon(String),
    /// Talking to a running MCP server over HTTP failed (connect, timeout,
    /// or a non-2xx status).
    Transport(String),
    /// The remote MCP tool ran and reported its own failure. The payload is
    /// the tool's verbatim text — never reworded, so agents parsing `op`
    /// output see exactly what the server said.
    Tool(String),
    /// A document could not be read, validated, converted, or imported.
    Document(String),
    /// Installing or uninstalling the bundled agent skill failed.
    Skill(SkillInstallError),
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::Usage(m)
            | CliError::Payload(m)
            | CliError::Io(m)
            | CliError::Daemon(m)
            | CliError::Transport(m)
            | CliError::Tool(m)
            | CliError::Document(m) => f.write_str(m),
            CliError::Skill(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for CliError {}

impl From<SkillInstallError> for CliError {
    fn from(error: SkillInstallError) -> Self {
        CliError::Skill(error)
    }
}

impl CliError {
    /// Shorthand for the very common `--flag`-shaped usage complaint.
    pub(crate) fn usage(message: impl Into<String>) -> Self {
        CliError::Usage(message.into())
    }
}
