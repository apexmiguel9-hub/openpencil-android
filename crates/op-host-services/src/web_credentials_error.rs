//! Typed failures for the browser credential snapshot the `--serve-web`
//! daemon accepts on `POST /api/settings/credentials`
//! (`web_credentials.rs`), plus the SSRF screen every web AI path reuses.
//!
//! Style follows `op_orchestrator::OrchestratorError`: a plain enum plus a
//! hand-written `Display`, no `thiserror` and no new dependency. Every variant
//! carries STRUCTURED fields and `Display` re-formats the sentence, so the
//! 400/413 bodies the browser reads — and the strings
//! `web_credentials_tests.rs` asserts on — are reproduced byte for byte while
//! callers can match on the reason instead of the prose.
//!
//! What the enum adds is the split this module's flat strings could not
//! express, and which its callers act on differently:
//!
//! - The four `Endpoint*` verdicts are the SSRF screen, and they are the ONLY
//!   failures the transient-credential chat paths (`ai_proxy`,
//!   `web_chat_standard`, `web_image_generate`) can hit, because those paths
//!   validate an endpoint without ever persisting a snapshot. Everything else
//!   is a persistence-merge verdict.
//! - [`WebCredentialError::is_payload_too_large`] — the one verdict the route
//!   answers `413 Payload Too Large` with rather than `400`. Before this
//!   enum, `web_canvas_server` re-derived that decision by re-measuring
//!   `body.len()` against the cap at the call site, which duplicated the
//!   threshold; now the error itself carries the verdict.
//!
//! ## The seam that still reports `String`, and why
//!
//! [`super::web_credentials::validate_web_provider_base_url`] keeps a
//! `Result<reqwest::Url, String>` signature. It is consumed with `?` from
//! `ai_proxy.rs` and `web_chat_standard.rs`, whose enclosing functions return
//! `Result<_, String>` and belong to other modules this pass does not own; its
//! typed core (`validate_web_provider_base_url_with_allowlist`) reports this
//! enum, and the public wrapper adapts. Convert the wrapper when those two
//! modules take a typed error.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebCredentialError {
    /// The request body is over
    /// [`crate::web_credentials::MAX_CREDENTIAL_BODY_BYTES`]. Answered `413`,
    /// not `400` — see [`WebCredentialError::is_payload_too_large`].
    PayloadTooLarge,
    /// The body is not the credential JSON this build accepts (bad syntax,
    /// unknown field, wrong value type). Deliberately terse: the payload
    /// carries API keys, so serde's message is NOT echoed back.
    InvalidPayload,
    /// The snapshot declares a schema version this build does not merge.
    UnsupportedPayloadVersion,
    /// One of the two entry lists is over the per-request cap.
    TooManyEntries,
    /// The snapshot names the same local built-in agent id twice, so the
    /// merge would silently drop one.
    DuplicateBuiltinAgentIds,
    /// Same condition for image profiles.
    DuplicateImageProfileIds,
    /// `active_image_gen_profile_id` names a profile the snapshot does not
    /// contain, so the merge would leave a dangling selection.
    ActiveImageProfileNotInSnapshot,
    /// An Openverse OAuth block was sent with both halves blank — clearing is
    /// expressed by omitting the block, not by sending it empty.
    OpenverseCredentialsBothEmpty,
    /// The merged store would exceed the per-daemon total cap. Distinct from
    /// [`WebCredentialError::TooManyEntries`], which bounds ONE request.
    StoreTooManyEntries,
    /// The agent's `kind` is not one this build can dial.
    UnsupportedBuiltinAgentKind,
    /// The profile's `provider` is not one this build can dial.
    UnsupportedImageGenProvider,
    /// A required id field (`field`) is blank.
    RequiredIdEmpty { field: String },
    /// A local id field (`field`) is empty, over the length cap, or uses a
    /// character outside `[A-Za-z0-9._-]` — the charset that keeps a scoped
    /// id unambiguous.
    InvalidLocalId { field: String },
    /// A text / URL / credential field (`field`) is over its byte cap.
    FieldTooLong { field: String },
    /// The endpoint is not a parseable URL.
    EndpointInvalid,
    /// The endpoint parsed but is refused: a non-HTTP(S) scheme, embedded
    /// credentials, a query/fragment, or — absent an explicit allowlist entry
    /// — a private, loopback, link-local, or cloud-metadata target.
    EndpointNotAllowed,
    /// A plain-HTTP endpoint that is not explicitly allowlisted.
    EndpointRequiresHttps,
    /// The endpoint passed the URL-shape screen but is not on the
    /// `OPENPENCIL_WEB_AI_ENDPOINT_ALLOWLIST`, which persisted browser
    /// credentials require on top of the shape check.
    EndpointNotExplicitlyAllowed,
}

impl WebCredentialError {
    /// Whether the route answers `413 Payload Too Large` instead of `400`.
    ///
    /// Owning the verdict here keeps
    /// [`crate::web_credentials::MAX_CREDENTIAL_BODY_BYTES`] compared in
    /// exactly one place; the route used to re-measure the body itself.
    pub fn is_payload_too_large(&self) -> bool {
        matches!(self, WebCredentialError::PayloadTooLarge)
    }
}

impl fmt::Display for WebCredentialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WebCredentialError::PayloadTooLarge => {
                f.write_str("credential payload exceeds 256 KiB")
            }
            WebCredentialError::InvalidPayload => f.write_str("invalid credential payload"),
            WebCredentialError::UnsupportedPayloadVersion => {
                f.write_str("unsupported credential payload version")
            }
            WebCredentialError::TooManyEntries => {
                f.write_str("credential payload has too many entries")
            }
            WebCredentialError::DuplicateBuiltinAgentIds => {
                f.write_str("credential payload contains duplicate built-in agent ids")
            }
            WebCredentialError::DuplicateImageProfileIds => {
                f.write_str("credential payload contains duplicate image profile ids")
            }
            WebCredentialError::ActiveImageProfileNotInSnapshot => {
                f.write_str("active image profile is not in the browser snapshot")
            }
            WebCredentialError::OpenverseCredentialsBothEmpty => {
                f.write_str("Openverse credentials must not both be empty")
            }
            WebCredentialError::StoreTooManyEntries => {
                f.write_str("credential store has too many entries")
            }
            WebCredentialError::UnsupportedBuiltinAgentKind => {
                f.write_str("unsupported built-in agent kind")
            }
            WebCredentialError::UnsupportedImageGenProvider => {
                f.write_str("unsupported image generation provider")
            }
            WebCredentialError::RequiredIdEmpty { field } => {
                write!(f, "{field} must not be empty")
            }
            WebCredentialError::InvalidLocalId { field } => write!(f, "{field} is invalid"),
            WebCredentialError::FieldTooLong { field } => write!(f, "{field} is too long"),
            WebCredentialError::EndpointInvalid => {
                f.write_str("browser provider endpoint is invalid")
            }
            WebCredentialError::EndpointNotAllowed => {
                f.write_str("browser provider endpoint is not allowed")
            }
            WebCredentialError::EndpointRequiresHttps => {
                f.write_str("browser provider endpoint must use HTTPS")
            }
            WebCredentialError::EndpointNotExplicitlyAllowed => {
                f.write_str("browser provider endpoint is not explicitly allowed")
            }
        }
    }
}

impl std::error::Error for WebCredentialError {}
