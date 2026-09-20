//! Preserve actionable self-check feedback across the end-of-run salvage pass.

use crate::plan::{RetryFeedback, Subtask};
use crate::retry::{is_non_retryable, is_self_check_rejection};
use crate::types::SubtaskOutcome;

pub(super) fn should_salvage(outcome: Option<&SubtaskOutcome>) -> bool {
    !outcome
        .and_then(|outcome| outcome.error.as_deref())
        .is_some_and(is_non_retryable)
}

pub(super) fn subtask_for_salvage(outcome: Option<&SubtaskOutcome>, fallback: &Subtask) -> Subtask {
    let mut subtask = outcome
        .and_then(|outcome| outcome.subtask.clone())
        .unwrap_or_else(|| fallback.clone());
    let latest_error = outcome.and_then(|outcome| outcome.error.as_deref());
    if latest_error.is_some_and(is_self_check_rejection) {
        subtask.retry_feedback = latest_error
            .map(str::to_string)
            .map(RetryFeedback::SelfCheck);
    }
    subtask
}

pub(super) fn finalize_failed_salvage(outcome: &mut SubtaskOutcome) -> String {
    let error = outcome
        .error
        .clone()
        .unwrap_or_else(|| "salvage attempt still empty".into());
    if is_self_check_rejection(&error) {
        if let Some(subtask) = outcome.subtask.as_mut() {
            subtask.retry_feedback = Some(RetryFeedback::SelfCheck(error.clone()));
        }
    }
    error
}
