//! Typed failures for the web shell's standard chat/design turn
//! (`web_chat_standard.rs`) — everything that can go wrong BEFORE the turn
//! starts streaming: applying the request's document/selection snapshot, and
//! resolving the three providers the route dispatches through.
//!
//! Style follows `op_orchestrator::OrchestratorError`: a plain enum plus a
//! hand-written `Display`, no `thiserror` and no new dependency. The
//! self-owned variants spell their sentence out; the rest are transparent
//! because the text belongs to a module this pass does not own. Either way
//! the bytes reaching `write_error_event` — and therefore the browser's SSE
//! `error` frame — are unchanged.
//!
//! What the enum adds is the split between a REQUEST fault (a mismatched or
//! disallowed transient credential, an unparseable document) and a
//! CONFIGURATION fault ([`WebChatStandardError::NoModelConfigured`] — the
//! request was fine, the daemon simply has no provider to answer with). Both
//! previously arrived as the same anonymous `String` at the same
//! `write_error_event` call, so nothing could tell "you sent something bad"
//! from "this deployment isn't set up".
//!
//! Three inbound seams still speak `String`, all of them modules outside this
//! conversion pass, and all adapted here with `e.to_string()` so their
//! wording rides through verbatim:
//!
//! - `web_credentials::validate_web_provider_base_url` →
//!   [`WebChatStandardError::EndpointRejected`]
//! - `op_pen_loader::load_canonical` → [`WebChatStandardError::Document`]
//! - `ai_proxy::proxy_provider_for_request_with_chat_session` →
//!   [`WebChatStandardError::ProviderResolve`]

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WebChatStandardError {
    /// The daemon is shutting down and will not durably accept a document
    /// write. The conversation still answers; the document is untouched.
    ShuttingDown,
    /// The request-scoped credential names a different model than the turn
    /// it rides with. Refused rather than reconciled — the credential is
    /// browser-supplied and the mismatch is unresolvable.
    TransientModelMismatch,
    /// The transient credential's `base_url` failed URL-shape screening.
    /// Text is `web_credentials`' own verdict.
    EndpointRejected(String),
    /// The transient credential's endpoint is private / loopback / reserved
    /// and the operator has not allowlisted it.
    EndpointNotAllowlisted,
    /// The request carried a document that the canonical loader refused.
    /// Text is `op_pen_loader`'s own message.
    Document(String),
    /// No built-in provider could be resolved for this request — neither a
    /// transient credential nor daemon settings supply one.
    NoModelConfigured,
    /// Provider resolution itself failed. Text is `ai_proxy`'s own message.
    ProviderResolve(String),
    /// A live collaboration session refuses AI writes. The M1 protocol has no
    /// way to sequence a write the local user did not make, so the desktop
    /// refuses the same turn — this keeps the daemon's answer identical rather
    /// than letting the AI route fork the shared document.
    CollabRefused(crate::web_canvas_server::DaemonMutationRefusal),
}

impl fmt::Display for WebChatStandardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WebChatStandardError::ShuttingDown => f.write_str(
                "this daemon is stopping; the reply was produced but the document was not changed",
            ),
            WebChatStandardError::TransientModelMismatch => {
                f.write_str("transient credential model does not match the request")
            }
            WebChatStandardError::EndpointNotAllowlisted => f.write_str(
                "provider endpoint is not allowed: private, loopback, and reserved addresses require an OPENPENCIL_WEB_AI_ENDPOINT_ALLOWLIST entry",
            ),
            WebChatStandardError::NoModelConfigured => f.write_str("no model configured"),
            WebChatStandardError::CollabRefused(refusal) => write!(f, "{refusal}"),
            WebChatStandardError::EndpointRejected(message)
            | WebChatStandardError::Document(message)
            | WebChatStandardError::ProviderResolve(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for WebChatStandardError {}
