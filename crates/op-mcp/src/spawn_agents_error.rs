//! Typed failures for the `spawn_agents` config validator
//! (`spawn_agents_tool.rs`).
//!
//! Style follows `ProgramError`: plain enums plus hand-written `Display`
//! impls, no `thiserror` and no new dependency. Every variant's `Display`
//! reproduces the exact sentence the stringly-typed validator produced —
//! the messages ship verbatim to the model as the `spawn_agents`
//! `InvalidArgument` payload, the desktop sub-agent session surfaces them
//! into the transcript, and the tool's own tests assert substrings of them.
//!
//! What the enums buy over `String` is the CLASSIFICATION plus the index:
//! `Field { index, cause }` keeps "which agent" and "which field" as data
//! instead of a `format!("spawn_agents config[{i}]: {e}")` prefix that has
//! to be re-parsed to be acted on.

use std::fmt;

/// A single field-level fault inside one config item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpawnFieldError {
    /// The field is present but is not a JSON string. `value` is the JSON
    /// rendering of what arrived.
    NotAString { key: String, value: String },
    /// A required field is absent.
    Required { key: String },
    /// An array element is not a JSON string.
    ElementNotAString {
        key: String,
        index: usize,
        value: String,
    },
    /// The field is present but is not a JSON array.
    NotAnArray { key: String, value: String },
}

impl fmt::Display for SpawnFieldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpawnFieldError::NotAString { key, value } => {
                write!(f, "{key} must be a string, got {value}")
            }
            SpawnFieldError::Required { key } => write!(f, "{key} is required"),
            SpawnFieldError::ElementNotAString { key, index, value } => {
                write!(f, "{key}[{index}] must be a string, got {value}")
            }
            SpawnFieldError::NotAnArray { key, value } => {
                write!(f, "{key} must be an array, got {value}")
            }
        }
    }
}

impl std::error::Error for SpawnFieldError {}

/// Everything `parse_spawn_config` can refuse on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpawnConfigError {
    /// `config` is missing, blank, or an empty array.
    EmptyConfig,
    /// `config` is not parseable JSON.
    ConfigNotJson(String),
    /// `config` parsed but is not a JSON array.
    ConfigNotArray,
    /// More sub-agents than the per-call cap allows.
    TooManyAgents { count: usize, max: usize },
    /// A config entry is not a JSON object.
    ItemNotObject { index: usize },
    /// A field inside config entry `index` is malformed.
    Field {
        index: usize,
        cause: SpawnFieldError,
    },
    /// `prompt` is present but blank.
    EmptyPrompt { index: usize },
    /// `styleguideName` is present but blank.
    EmptyStyleguideName { index: usize },
}

impl fmt::Display for SpawnConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpawnConfigError::EmptyConfig => {
                f.write_str("spawn_agents requires a non-empty config array")
            }
            SpawnConfigError::ConfigNotJson(detail) => {
                write!(f, "config must be a JSON array: {detail}")
            }
            SpawnConfigError::ConfigNotArray => f.write_str("config must be a JSON array"),
            SpawnConfigError::TooManyAgents { count, max } => write!(
                f,
                "spawn_agents config exceeds the {max}-agent cap (got {count}); Pencil recommends ≤ 8–10 agents per call"
            ),
            SpawnConfigError::ItemNotObject { index } => {
                write!(f, "spawn_agents config[{index}] must be an object")
            }
            SpawnConfigError::Field { index, cause } => {
                write!(f, "spawn_agents config[{index}]: {cause}")
            }
            SpawnConfigError::EmptyPrompt { index } => {
                write!(f, "spawn_agents config[{index}]: prompt must be non-empty")
            }
            SpawnConfigError::EmptyStyleguideName { index } => write!(
                f,
                "spawn_agents config[{index}]: styleguideName must be non-empty (sub-agents cannot search styleguides — pass the name explicitly)"
            ),
        }
    }
}

impl std::error::Error for SpawnConfigError {}

/// Boundary bridge for `parse_spawn_config`, whose `Result<_, String>`
/// signature is pinned by `op_host_desktop::sub_agent_session::parse_spawn_args`
/// returning it as a tail expression (no `?`, so no `From` coercion happens
/// there). `Display` reproduces the exact sentence, so the text the model
/// receives is unchanged.
impl From<SpawnConfigError> for String {
    fn from(error: SpawnConfigError) -> String {
        error.to_string()
    }
}
