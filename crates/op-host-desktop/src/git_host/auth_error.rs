//! Typed failures for the Git panel's credential setup (`repo_ops.rs`):
//! generating and binding an SSH key for the `origin` host, and storing an
//! HTTPS `username:token` pair for it.
//!
//! Style follows `op_orchestrator::OrchestratorError`: a plain enum plus a
//! hand-written `Display`, no `thiserror` and no new dependency. `Display`
//! reproduces the exact sentence the stringly code produced, so the
//! `openpencil-desktop: ssh auth setup failed: …` and
//! `openpencil-desktop: store HTTPS credential failed: …` stderr lines are
//! unchanged byte for byte.
//!
//! Both flows were written as `Result`-returning closures typed
//! `Result<_, String>` purely so their guard chain could use `?`. The enum
//! keeps that shape while making the guard set explicit: every variant below
//! except [`GitAuthError::KeyGeneration`] / [`GitAuthError::StoreCredential`]
//! is a PRECONDITION this host checks before touching the credential store —
//! including the load-bearing one, [`GitAuthError::OriginNotHttps`], which
//! exists because the store is host-keyed, so writing an HTTPS credential for
//! an SSH `origin` would shadow and break that host's SSH credential.
//!
//! `op-git` is a crate this pass does not own; its messages are carried with
//! `e.to_string()`.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum GitAuthError {
    /// Both credential stores (auth + SSH) are unavailable — no config
    /// directory, so nothing can be persisted. Reported by the SSH flow,
    /// which needs both.
    CredentialStoresUnavailable,
    /// The auth store alone is unavailable. Reported by the HTTPS flow,
    /// which only needs that one, and worded accordingly.
    CredentialStoreUnavailable,
    /// No repository is bound to the session, so there is no `origin` to
    /// authenticate against.
    NoRepository,
    /// The bound repository has no `origin` remote.
    NoOriginRemote,
    /// `origin` is not an HTTPS remote, so an HTTPS credential must not be
    /// stored for its host.
    OriginNotHttps,
    /// The `origin` URL parsed but carries no host to key the credential on.
    OriginHasNoHost,
    /// The Remotes-section draft is not in `username:token` form.
    CredentialFormat,
    /// The draft split, but one of the halves is blank after trimming.
    MissingCredentialFields,
    /// Generating a fresh SSH key pair failed.
    KeyGeneration(String),
    /// Writing the credential into the host-keyed store failed.
    StoreCredential(String),
}

impl fmt::Display for GitAuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GitAuthError::CredentialStoresUnavailable => {
                f.write_str("credential stores are unavailable")
            }
            GitAuthError::CredentialStoreUnavailable => {
                f.write_str("credential store is unavailable")
            }
            GitAuthError::NoRepository => f.write_str("no repository is bound"),
            GitAuthError::NoOriginRemote => f.write_str("no `origin` remote — set one first"),
            GitAuthError::OriginNotHttps => {
                f.write_str("the origin remote is not HTTPS — use the SSH button instead")
            }
            GitAuthError::OriginHasNoHost => f.write_str("the origin URL has no host"),
            GitAuthError::CredentialFormat => f.write_str("credential must be `username:token`"),
            GitAuthError::MissingCredentialFields => {
                f.write_str("both a username and a token are required")
            }
            GitAuthError::KeyGeneration(message) | GitAuthError::StoreCredential(message) => {
                f.write_str(message)
            }
        }
    }
}

impl std::error::Error for GitAuthError {}
