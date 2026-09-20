//! Typed failure for parsing a `spawn_agents` tool call's raw arguments
//! (`sub_agent_session::parse_spawn_args`).
//!
//! Style follows `op_orchestrator::OrchestratorError`: a plain enum plus a
//! hand-written `Display`, no `thiserror` and no new dependency. `Display`
//! reproduces the exact sentence the stringly code produced, which matters
//! more here than in most of this crate: the message is not just logged, it
//! is serialized into the `{"success": false, "error": …}` envelope the model
//! reads back (`chat_session::handle_spawn_agents`), and
//! `sub_agent_session_tests.rs` asserts on both wordings.
//!
//! What the enum adds is whose fault the call was: the first two variants are
//! this desktop shim's own normalisation refusing malformed args, while
//! [`SpawnArgsError::Invalid`] is the shared validator
//! (`op_mcp::spawn_agents_tool::parse_spawn_config`) rejecting a
//! well-formed-but-wrong spec. Only the latter reflects the agent catalog's
//! rules, and it is the only one worth re-prompting the model about.
//!
//! `serde_json` and `op-mcp` are crates this pass does not own; their
//! messages are carried with `e.to_string()` / verbatim.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SpawnArgsError {
    /// `args_json` is not parseable JSON at all.
    NotJson(String),
    /// The JSON parsed but has no `config` field to normalise.
    MissingConfig,
    /// The shared validator refused the normalised config. Its message is
    /// carried verbatim.
    Invalid(String),
}

impl fmt::Display for SpawnArgsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpawnArgsError::NotJson(message) => {
                write!(f, "spawn_agents args must be a JSON object: {message}")
            }
            SpawnArgsError::MissingConfig => {
                f.write_str("spawn_agents requires a non-empty config array")
            }
            SpawnArgsError::Invalid(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for SpawnArgsError {}
