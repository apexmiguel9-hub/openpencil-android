use super::*;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::time::Duration;

const FIRST_MESSAGE: Duration = Duration::from_millis(200);

fn limits() -> ConnectionLimits {
    ConnectionLimits {
        max_pending_handshakes: 2,
        max_pending_handshakes_per_ip: 1,
        max_active_connections: 1,
        ..ConnectionLimits::default()
    }
}

fn timeouts() -> TimeoutConfig {
    TimeoutConfig {
        handshake_first_message: FIRST_MESSAGE,
        ..TimeoutConfig::default()
    }
}

fn limiter(limits: ConnectionLimits) -> ConnectionLimiter {
    ConnectionLimiter::with_timeouts(limits, timeouts()).unwrap()
}

fn address(last: u8) -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(198, 51, 100, last))
}

#[test]
fn pending_limits_are_exact_and_drop_releases_them() {
    let limiter = limiter(limits());
    let now = Instant::now();
    let first_ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let second_ip = IpAddr::V6(Ipv6Addr::LOCALHOST);
    let first = limiter.try_begin_handshake_at(first_ip, now).unwrap();
    assert!(matches!(
        limiter.try_begin_handshake_at(first_ip, now),
        Err(ConnectionLimitError::PendingHandshakesPerIpFull)
    ));
    let second = limiter.try_begin_handshake_at(second_ip, now).unwrap();
    assert!(matches!(
        limiter.try_begin_handshake_at(address(1), now),
        Err(ConnectionLimitError::PendingHandshakesFull)
    ));
    drop(first);
    assert_eq!(limiter.counts_at(now).unwrap().pending_handshakes, 1);
    drop(second);
    assert_eq!(limiter.counts_at(now).unwrap().pending_handshakes, 0);
}

#[test]
fn activation_is_bounded_and_raii_releases_both_phases() {
    let limiter = limiter(limits());
    let now = Instant::now();
    let first = limiter
        .try_begin_handshake_at(IpAddr::V4(Ipv4Addr::LOCALHOST), now)
        .unwrap();
    let active = first.activate().unwrap();
    assert_eq!(
        limiter.counts_at(now).unwrap(),
        ConnectionCounts {
            pending_handshakes: 0,
            pending_guards: 0,
            active_connections: 1,
        }
    );

    let pending = limiter
        .try_begin_handshake_at(IpAddr::V6(Ipv6Addr::LOCALHOST), now)
        .unwrap();
    assert!(matches!(
        pending.activate(),
        Err(ConnectionLimitError::ActiveConnectionsFull)
    ));
    assert_eq!(limiter.counts_at(now).unwrap().pending_handshakes, 0);
    drop(active);
    assert_eq!(limiter.counts_at(now).unwrap().active_connections, 0);
}

#[test]
fn live_silent_guards_stay_charged_until_the_socket_worker_drops_them() {
    let limiter = limiter(limits());
    let start = Instant::now();
    let silent_first = limiter.try_begin_handshake_at(address(1), start).unwrap();
    let silent_second = limiter.try_begin_handshake_at(address(2), start).unwrap();
    assert!(matches!(
        limiter.try_begin_handshake_at(address(3), start),
        Err(ConnectionLimitError::PendingHandshakesFull)
    ));

    let after_deadline = start + FIRST_MESSAGE * 10;
    assert!(silent_first.holds_global_seat_at(after_deadline));
    assert!(silent_second.holds_global_seat_at(after_deadline));
    let counts = limiter.counts_at(after_deadline).unwrap();
    assert_eq!(counts.pending_handshakes, 2);
    assert_eq!(counts.pending_guards, 2);
    assert!(matches!(
        limiter.try_begin_handshake_at(address(3), after_deadline),
        Err(ConnectionLimitError::PendingHandshakesFull)
    ));

    drop(silent_first);
    let replacement = limiter
        .try_begin_handshake_at(address(3), after_deadline)
        .unwrap();
    drop(silent_second);
    drop(replacement);
    assert_eq!(
        limiter.counts_at(after_deadline).unwrap(),
        ConnectionCounts {
            pending_handshakes: 0,
            pending_guards: 0,
            active_connections: 0,
        }
    );
}

#[test]
fn valid_handshake_progress_preserves_the_continuously_held_seat() {
    let limiter = limiter(limits());
    let start = Instant::now();
    let proven = limiter.try_begin_handshake_at(address(1), start).unwrap();
    let silent = limiter.try_begin_handshake_at(address(2), start).unwrap();
    proven.note_handshake_progress_at(start).unwrap();

    let long_after = start + FIRST_MESSAGE * 10;
    assert!(proven.holds_global_seat_at(long_after));
    assert!(silent.holds_global_seat_at(long_after));
    assert_eq!(
        limiter.counts_at(long_after).unwrap().pending_handshakes,
        2,
        "every live socket worker must keep charging the global pool"
    );
    assert!(matches!(
        limiter.try_begin_handshake_at(address(3), long_after),
        Err(ConnectionLimitError::PendingHandshakesFull)
    ));
    drop(silent);
}

#[test]
fn progress_and_activation_fail_if_a_guard_ever_loses_its_seat() {
    let limiter = limiter(limits());
    let start = Instant::now();
    let pending = limiter.try_begin_handshake_at(address(1), start).unwrap();
    {
        let mut state = limiter.inner.state.lock().unwrap();
        release_pending(&mut state, pending.slot);
    }

    assert_eq!(
        pending.note_handshake_progress_at(start),
        Err(ConnectionLimitError::PendingHandshakeSeatLost)
    );
    assert!(matches!(
        pending.activate(),
        Err(ConnectionLimitError::PendingHandshakeSeatLost)
    ));
}

#[test]
fn default_ceiling_keeps_honest_joins_working_under_a_silent_flood() {
    let limits = ConnectionLimits::default();
    let limiter = ConnectionLimiter::new(limits).unwrap();
    let now = Instant::now();
    let flood: Vec<_> = (0..4_u8)
        .flat_map(|source| (0..limits.max_pending_handshakes_per_ip).map(move |_| address(source)))
        .map(|peer| limiter.try_begin_handshake_at(peer, now).unwrap())
        .collect();
    assert_eq!(flood.len(), 4 * limits.max_pending_handshakes_per_ip);

    // Four addresses at their per-address maximum still leave space under the
    // reviewed global ceiling for honest guests.
    let guests: Vec<_> = (100..164_u8)
        .map(|guest| {
            limiter
                .try_begin_handshake_at(address(guest), now)
                .expect("honest guests still get a pending seat during the flood")
        })
        .collect();
    assert_eq!(
        limiter.counts_at(now).unwrap().pending_handshakes,
        flood.len() + guests.len()
    );
    drop(guests);
    drop(flood);
}
