//! Classification and credential-free reporting for relay control-plane
//! failures.
//!
//! Every control-plane call used to end in `map_err(|_| RelayUnavailable)`, so
//! an expired collaboration ticket, a rate-limited hub, an unreachable network
//! and a malformed response all reached the user as the same sentence — "the
//! public relay is temporarily unavailable" — and left nothing behind to tell
//! them apart. Two of those are not temporary, and one is the user's own
//! sign-in state.
//!
//! The classifier below keeps each cause on its own notice, and the reporter
//! writes one line per failure so a support log identifies the stage without
//! shipping any credential: only the failure enum (which carries no payload)
//! and a `&'static str` variant tag are ever formatted. The error's own
//! `Display` is deliberately never used — it is free to grow payload fields,
//! and this line must stay safe when it does.

use op_collab_relay_control_plane::{PairingClaimError, RelayLocatorIssueError};

use crate::runtime::types::CollabRuntimeFailure;

/// Map a control-plane error onto the notice the user should actually see,
/// reporting the stage on the way out.
pub(super) fn control_plane_failure(
    stage: &'static str,
    error: RelayLocatorIssueError,
) -> CollabRuntimeFailure {
    let failure = match error {
        // A refused ticket is a sign-in problem: the existing "session
        // expired" notice is the honest one, and it does not invite waiting.
        RelayLocatorIssueError::PublishUnauthorized => CollabRuntimeFailure::TicketRejected,
        RelayLocatorIssueError::PublishRateLimited => CollabRuntimeFailure::RelayRateLimited,
        _ => CollabRuntimeFailure::RelayUnavailable,
    };
    report(stage, failure, issue_tag(&error));
    failure
}

/// Report a stage failure whose classification the caller already made.
pub(super) fn report_control_plane_failure(
    stage: &'static str,
    failure: CollabRuntimeFailure,
    error: &PairingClaimError,
) {
    report(stage, failure, claim_tag(error));
}

fn report(stage: &'static str, failure: CollabRuntimeFailure, tag: &'static str) {
    eprintln!(
        "[collab] RelayControlPlaneFailed {{ stage: {stage}, failure: {failure:?}, error: {tag} }}"
    );
}

/// Variant tags only — never the error's `Display`, which may carry payload.
const fn issue_tag(error: &RelayLocatorIssueError) -> &'static str {
    match error {
        RelayLocatorIssueError::Protocol(_) => "Protocol",
        RelayLocatorIssueError::ZeroLifetime => "ZeroLifetime",
        RelayLocatorIssueError::LifetimeTooLong { .. } => "LifetimeTooLong",
        RelayLocatorIssueError::InvalidRequestLength { .. } => "InvalidRequestLength",
        RelayLocatorIssueError::UnsupportedRequestVersion { .. } => "UnsupportedRequestVersion",
        RelayLocatorIssueError::NonZeroRequestPadding => "NonZeroRequestPadding",
        RelayLocatorIssueError::InvalidRequestUtf8 => "InvalidRequestUtf8",
        RelayLocatorIssueError::ClockBeforeUnixEpoch => "ClockBeforeUnixEpoch",
        RelayLocatorIssueError::ZeroCurrentTime => "ZeroCurrentTime",
        RelayLocatorIssueError::OwnerBindingRejected => "OwnerBindingRejected",
        RelayLocatorIssueError::ExpiryOverflow => "ExpiryOverflow",
        RelayLocatorIssueError::InvalidRandomGeneration => "InvalidRandomGeneration",
        RelayLocatorIssueError::Signer(_) => "Signer",
        RelayLocatorIssueError::ResponseBindingMismatch { .. } => "ResponseBindingMismatch",
        RelayLocatorIssueError::InvalidControlPlaneEndpoint => "InvalidControlPlaneEndpoint",
        RelayLocatorIssueError::InsecureControlPlaneEndpoint => "InsecureControlPlaneEndpoint",
        RelayLocatorIssueError::InvalidPublishCredential => "InvalidPublishCredential",
        RelayLocatorIssueError::PublishTransportUnavailable => "PublishTransportUnavailable",
        RelayLocatorIssueError::PublishUnauthorized => "PublishUnauthorized",
        RelayLocatorIssueError::PublishRateLimited => "PublishRateLimited",
        RelayLocatorIssueError::PublishResponseRejected => "PublishResponseRejected",
        RelayLocatorIssueError::PublishResponseTooLarge { .. } => "PublishResponseTooLarge",
    }
}

const fn claim_tag(error: &PairingClaimError) -> &'static str {
    match error {
        PairingClaimError::NotFound => "NotFound",
        PairingClaimError::Rejected => "Rejected",
        PairingClaimError::Unauthorized => "Unauthorized",
        PairingClaimError::RateLimited => "RateLimited",
        PairingClaimError::TransportUnavailable => "TransportUnavailable",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refused_ticket_is_not_reported_as_a_transient_relay_outage() {
        // The whole point of the split: "sign in again" and "try again
        // later" must not share a notice.
        assert_eq!(
            control_plane_failure("publish_route", RelayLocatorIssueError::PublishUnauthorized),
            CollabRuntimeFailure::TicketRejected
        );
        assert_eq!(
            control_plane_failure("publish_route", RelayLocatorIssueError::PublishRateLimited),
            CollabRuntimeFailure::RelayRateLimited
        );
    }

    #[test]
    fn unclassified_control_plane_errors_stay_relay_unavailable() {
        for error in [
            RelayLocatorIssueError::PublishTransportUnavailable,
            RelayLocatorIssueError::PublishResponseRejected,
            RelayLocatorIssueError::OwnerBindingRejected,
        ] {
            assert_eq!(
                control_plane_failure("publish_route", error),
                CollabRuntimeFailure::RelayUnavailable
            );
        }
    }

    #[test]
    fn every_tag_is_a_bare_variant_name() {
        // A tag that ever grows punctuation is a tag that started formatting
        // payload, which is exactly what must not reach a support log.
        let tags = [
            issue_tag(&RelayLocatorIssueError::PublishUnauthorized),
            issue_tag(&RelayLocatorIssueError::LifetimeTooLong {
                actual: 1,
                maximum: 2,
            }),
            issue_tag(&RelayLocatorIssueError::ResponseBindingMismatch { field: "route" }),
            claim_tag(&PairingClaimError::Unauthorized),
        ];
        for tag in tags {
            assert!(
                tag.chars()
                    .all(|character| character.is_ascii_alphanumeric()),
                "{tag:?} must be a bare variant name"
            );
        }
    }
}
