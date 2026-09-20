//! Typed failures for the orchestrator's `design.md` generation turn
//! (`design_md_llm.rs`) — the LLM call that captures the current canvas as a
//! reusable design system before a follow-on screen is drawn.
//!
//! Style follows `op_orchestrator::OrchestratorError`: a plain enum plus a
//! hand-written `Display`, no `thiserror` and no new dependency. The two
//! verdict variants have no fields and `Display` writes their exact former
//! sentences, so the text is reproduced byte for byte.
//!
//! What the enum adds is the distinction the single `String` erased: whether
//! the TRANSPORT failed ([`DesignMdError::Llm`]) or the MODEL misbehaved
//! ([`DesignMdError::EmptyOutput`] / [`DesignMdError::NotADesignSystemDocument`]
//! — the two cases where the call succeeded and only the markdown was
//! unusable, so retrying the same prompt is worth it). Today `design_session`
//! discards the error either way (design.md generation is a best-effort
//! enrichment step; a failure must not abort the design run), so this
//! conversion changes no behaviour — it makes the two cases nameable when that
//! call site grows a policy.
//!
//! One inbound seam speaks `String`: `op_orchestrator::LlmError`'s `message`
//! field, which belongs to a crate this pass does not own and is carried
//! verbatim by [`DesignMdError::Llm`].

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesignMdError {
    /// The LLM stream ended in an error (provider refusal, timeout, abort,
    /// transport failure). Carries `op_orchestrator::LlmError::message`
    /// verbatim.
    Llm(String),
    /// The stream completed but produced no usable text after cleaning.
    EmptyOutput,
    /// The model returned text, but not a design-system document — it does
    /// not open with the `# Design System:` heading the downstream
    /// `parse_design_md` contract requires.
    NotADesignSystemDocument,
}

impl fmt::Display for DesignMdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DesignMdError::Llm(message) => f.write_str(message),
            DesignMdError::EmptyOutput => f.write_str("design.md generation returned empty output"),
            DesignMdError::NotADesignSystemDocument => {
                f.write_str("design.md generation did not return a Design System document")
            }
        }
    }
}

impl std::error::Error for DesignMdError {}
