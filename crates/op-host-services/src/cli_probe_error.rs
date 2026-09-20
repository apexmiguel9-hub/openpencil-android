//! Typed failures for the external-CLI probes: the connect probes in
//! `cli_provider_probe.rs` and the model-catalog queries in
//! `cli_model_discovery.rs`. Both drive the same bounded subprocess runner
//! (`cli_probe_support::bounded_cli_output`) and both used to report a bare
//! `String`, so they share one enum.
//!
//! Style follows `op_orchestrator::OrchestratorError`: a plain enum plus a
//! hand-written `Display`, no `thiserror` and no new dependency. The English
//! sentences are re-formatted from STRUCTURED fields so the wording lives in
//! one place; the pre-composed variants below are the ones whose text is not
//! this module's to author, and they carry it verbatim. Either way the text a
//! user sees in the Settings provider card is unchanged byte for byte.
//!
//! What the enum adds is the distinction those probes never had. "Could not
//! list models" collapsed five genuinely different outcomes into one string:
//! the CLI is missing, the CLI hung (usually mid first-run OAuth), the CLI
//! exited non-zero, the CLI answered but wants credentials, and the CLI
//! answered with something this parser does not recognise. Only the last is a
//! parser bug; the middle three are the user's to fix and each has a
//! different fix. Callers that only need the sentence still just `Display` it.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliProbeError {
    /// The CLI executable is not on `PATH` (and not at any of the known
    /// install locations `resolve_cli` checks).
    NotFound { provider: &'static str },
    /// The probe hit its deadline. The message is composed by
    /// [`crate::cli_probe_support::diagnose_timeout`], which echoes the CLI's
    /// OWN prompt back when it recognises one — that text is the CLI's, not
    /// ours, so it is carried verbatim rather than re-derived from fields.
    /// Deliberately untranslated for the same reason.
    Timeout(String),
    /// The bounded runner could not run the process at all (spawn failed, or
    /// it could not be reaped). Distinct from [`CliProbeError::Timeout`]: no
    /// deadline was reached, so there is no partial output to diagnose.
    NotResponding { provider: &'static str },
    /// The CLI exited non-zero with nothing useful on stderr. `login_command`
    /// is `Some` for the CLIs whose usual cause is an unauthenticated first
    /// run, and appends the "run it once" hint the old message carried.
    QueryFailed {
        provider: &'static str,
        login_command: Option<&'static str>,
    },
    /// The CLI exited non-zero and said why. Its own words are the most
    /// useful thing we can show, so they are passed through untouched.
    CliReported(String),
    /// The CLI ran and printed a catalog, but every line of it reads as an
    /// auth prompt — it needs credentials before it will answer.
    AuthRequired {
        provider: &'static str,
        login_command: &'static str,
    },
    /// The CLI exited cleanly and printed nothing to parse.
    NoCatalog { provider: &'static str },
    /// The CLI printed something, but no line of it looks like a model id or
    /// display name. Unlike every other variant this one is likely OUR bug —
    /// a catalog format the parser has not learned yet.
    UnrecognizedCatalog { provider: &'static str },
    /// An already-translated message from the `op_i18n` catalog. The locale
    /// is only known at the raise site (it comes from the editor state), so
    /// translation happens there and the finished sentence travels in this
    /// variant rather than the enum growing a locale field it would have to
    /// thread through every constructor.
    Localized(String),
}

impl fmt::Display for CliProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliProbeError::NotFound { provider } => write!(f, "{provider} CLI not found"),
            CliProbeError::Timeout(message)
            | CliProbeError::CliReported(message)
            | CliProbeError::Localized(message) => f.write_str(message),
            CliProbeError::NotResponding { provider } => {
                write!(f, "{provider} model query failed or timed out")
            }
            CliProbeError::QueryFailed {
                provider,
                login_command,
            } => {
                write!(f, "{provider} model query failed")?;
                match login_command {
                    Some(command) => write!(f, ". Run {command} once to authenticate."),
                    None => Ok(()),
                }
            }
            CliProbeError::AuthRequired {
                provider,
                login_command,
            } => write!(
                f,
                "{provider} model query requires authentication. \
                 Run {login_command} once to sign in."
            ),
            CliProbeError::NoCatalog { provider } => {
                write!(f, "{provider} model query returned no model catalog")
            }
            CliProbeError::UnrecognizedCatalog { provider } => {
                write!(f, "{provider} returned an unrecognized model catalog")
            }
        }
    }
}

impl std::error::Error for CliProbeError {}
