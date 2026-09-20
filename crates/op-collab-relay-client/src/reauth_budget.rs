use std::time::Duration;

use tokio::time::Instant;

use crate::error::{RelayFailureKind, TunnelError};
use crate::limits::RelayLimits;

/// Halving divisor applied to the idle window to obtain the minimum spacing
/// between two server-initiated reauthentications.
const MINIMUM_INTERVAL_DIVISOR: u32 = 2;

/// Headroom multiplier applied to the honest per-lifetime challenge count.
const ATTEMPT_HEADROOM_FACTOR: u64 = 2;

/// Per-connection ceiling on server-initiated reauthentication.
///
/// Every text frame the relay sends is a reauthentication challenge, and
/// answering one mints a fresh bearer through the caller's credential
/// provider — on the desktop that is the local authentication bridge and the
/// hub ticket-mint path. Without a bound, a compromised relay can drive one
/// mint per frame for the whole 24 h tunnel lifetime and use the client as a
/// free amplifier against its own authentication backend. This is not a
/// credential-exposure problem (the relay already holds a valid bearer), so
/// the bound is sized purely to keep the amplification factor small while
/// never interfering with the protocol's real reauthentication cadence.
///
/// # Sizing
///
/// Server-initiated reauthentication is driven by admission-ticket rotation,
/// not by traffic: the relay challenges once per accepted ticket, as that
/// ticket's deadline approaches. The client never sees ticket lifetimes, so
/// both halves of the bound are derived from the two connection limits it does
/// own, [`RelayLimits::idle`] and [`RelayLimits::lifetime`].
///
/// * **Minimum spacing = `idle / 2`** (60 s with the default 2 min idle
///   window). `idle` is the longest a live tunnel may stay silent, so a ticket
///   whose entire lifetime were shorter than one idle window could not survive
///   a single quiet period — no legitimate rotation cadence can be faster than
///   `idle`. Halving it leaves 2x headroom for clock jitter and for a relay
///   that challenges slightly early.
/// * **Maximum count = `2 * (lifetime / idle)`** (2 * (86_400 / 120) = 1_440
///   with the defaults). An honest relay challenges at most once per rotation,
///   and rotation is at least `idle` apart, so a full-length tunnel needs at
///   most `lifetime / idle` = 720 challenges — exactly half the cap.
///
/// A legitimate relay therefore uses at most half of each bound over a
/// full-length tunnel, while a malicious relay is held to one bearer mint per
/// minute and 1_440 mints per connection. The count cap is not redundant with
/// the spacing rule: it is time-independent, so it still holds if the
/// monotonic clock stalls or if a caller configures a longer lifetime.
pub(crate) struct ReauthBudget {
    remaining: u32,
    minimum_interval: Duration,
    last_admitted: Option<Instant>,
}

impl ReauthBudget {
    pub(crate) fn new(limits: RelayLimits) -> Self {
        Self {
            remaining: maximum_attempts(limits),
            minimum_interval: minimum_interval(limits),
            last_admitted: None,
        }
    }

    /// Charges one server-initiated reauthentication against the budget.
    ///
    /// Exceeding either half of the bound fails the connection closed with a
    /// typed failure kind; the frame is never silently ignored, because a
    /// relay that pushes past the bound is misbehaving and the tunnel must not
    /// continue as if nothing happened.
    pub(crate) fn admit(&mut self, now: Instant) -> Result<(), TunnelError> {
        if self
            .last_admitted
            .is_some_and(|previous| now.saturating_duration_since(previous) < self.minimum_interval)
        {
            return Err(TunnelError::Failure(RelayFailureKind::ReauthTooFrequent));
        }
        self.remaining = self.remaining.checked_sub(1).ok_or(TunnelError::Failure(
            RelayFailureKind::ReauthBudgetExhausted,
        ))?;
        self.last_admitted = Some(now);
        Ok(())
    }
}

impl std::fmt::Debug for ReauthBudget {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ReauthBudget")
            .field("remaining", &self.remaining)
            .field("minimum_interval", &self.minimum_interval)
            .finish()
    }
}

fn minimum_interval(limits: RelayLimits) -> Duration {
    limits.idle / MINIMUM_INTERVAL_DIVISOR
}

fn maximum_attempts(limits: RelayLimits) -> u32 {
    // `max(1)` keeps a degenerate zero idle window from dividing by zero and
    // guarantees at least one server-initiated reauthentication is admitted.
    let idle_seconds = limits.idle.as_secs().max(1);
    let honest_attempts = limits.lifetime.as_secs() / idle_seconds;
    let bounded = honest_attempts
        .saturating_mul(ATTEMPT_HEADROOM_FACTOR)
        .max(1);
    u32::try_from(bounded).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_bounds_match_the_documented_arithmetic() {
        let limits = RelayLimits::default();
        assert_eq!(minimum_interval(limits), Duration::from_secs(60));
        assert_eq!(maximum_attempts(limits), 1_440);
    }

    #[test]
    fn normal_reauth_cadence_over_a_full_tunnel_lifetime_stays_within_budget() {
        let limits = RelayLimits::default();
        let mut budget = ReauthBudget::new(limits);
        let started_at = Instant::now();
        // The honest cadence is one challenge per ticket rotation, and rotation
        // cannot be faster than one idle window. Walk the whole 24 h lifetime
        // at exactly that fastest honest cadence.
        let honest_attempts = limits.lifetime.as_secs() / limits.idle.as_secs();
        assert_eq!(honest_attempts, 720);
        for attempt in 1..=honest_attempts {
            let now = started_at + limits.idle * u32::try_from(attempt).expect("attempt fits");
            assert_eq!(
                budget.admit(now),
                Ok(()),
                "attempt {attempt} must be admitted"
            );
        }
    }

    #[test]
    fn reauth_budget_rejects_a_repeat_inside_the_minimum_interval() {
        let limits = RelayLimits::default();
        let mut budget = ReauthBudget::new(limits);
        let started_at = Instant::now();
        assert_eq!(budget.admit(started_at), Ok(()));
        let too_soon = started_at + minimum_interval(limits) - Duration::from_millis(1);
        assert_eq!(
            budget.admit(too_soon),
            Err(TunnelError::Failure(RelayFailureKind::ReauthTooFrequent))
        );
        assert_eq!(budget.admit(started_at + minimum_interval(limits)), Ok(()));
    }

    #[test]
    fn reauth_budget_fails_closed_after_the_per_connection_cap() {
        let limits = RelayLimits {
            idle: Duration::from_secs(10),
            lifetime: Duration::from_secs(20),
            ..RelayLimits::default()
        };
        // 2 * (20 / 10) = 4 admitted challenges, spaced at least 10 / 2 = 5 s.
        assert_eq!(maximum_attempts(limits), 4);
        let mut budget = ReauthBudget::new(limits);
        let started_at = Instant::now();
        for attempt in 0..4 {
            let now = started_at + minimum_interval(limits) * attempt;
            assert_eq!(budget.admit(now), Ok(()));
        }
        assert_eq!(
            budget.admit(started_at + minimum_interval(limits) * 4),
            Err(TunnelError::Failure(
                RelayFailureKind::ReauthBudgetExhausted
            ))
        );
    }
}
