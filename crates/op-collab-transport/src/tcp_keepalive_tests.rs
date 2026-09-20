//! Guards for the config-to-socket keepalive mapping.
//!
//! `timeouts.heartbeat` is an arbitrary-precision `Duration` that
//! `TransportConfig::validate` accepts as long as it is non-zero, but the OS
//! keepalive options behind it are whole-second knobs. Feeding them the raw
//! duration made every sub-second heartbeat unusable: Linux failed each connect
//! and accept with `EINVAL` before any protocol byte moved, and macOS quietly
//! discarded the request and kept its own default. These tests pin both ends of
//! that mapping — the rounding itself, and what the kernel ends up holding.

use super::*;
use std::net::TcpListener;

#[test]
fn sub_second_heartbeats_round_up_to_the_os_keepalive_granularity() {
    assert_eq!(
        os_keepalive_period(Duration::from_millis(1)),
        Duration::from_secs(1)
    );
    assert_eq!(
        os_keepalive_period(Duration::from_millis(50)),
        Duration::from_secs(1)
    );
    assert_eq!(
        os_keepalive_period(Duration::from_millis(999)),
        Duration::from_secs(1)
    );
}

#[test]
fn whole_second_heartbeats_reach_the_socket_unchanged() {
    assert_eq!(
        os_keepalive_period(Duration::from_secs(1)),
        Duration::from_secs(1)
    );
    assert_eq!(
        os_keepalive_period(Duration::from_secs(30)),
        Duration::from_secs(30)
    );
}

#[test]
fn fractional_heartbeats_round_up_rather_than_truncating_toward_zero() {
    // Truncation is what socket2 does with `Duration::as_secs()`, so a period
    // that lands between two seconds must be lifted before it gets there.
    assert_eq!(
        os_keepalive_period(Duration::from_millis(1_900)),
        Duration::from_secs(2)
    );
    assert_eq!(
        os_keepalive_period(Duration::from_millis(30_001)),
        Duration::from_secs(31)
    );
}

/// The OS keepalive option names differ per platform but share the same
/// whole-second contract.
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod socket_readback {
    use super::*;
    use std::os::fd::AsRawFd;

    #[cfg(target_os = "linux")]
    const KEEPALIVE_TIME: libc::c_int = libc::TCP_KEEPIDLE;
    #[cfg(target_os = "macos")]
    const KEEPALIVE_TIME: libc::c_int = libc::TCP_KEEPALIVE;

    fn keepalive_seconds(stream: &TcpStream, option: libc::c_int) -> libc::c_int {
        let mut value: libc::c_int = -1;
        let mut length = std::mem::size_of::<libc::c_int>() as libc::socklen_t;
        let result = unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::IPPROTO_TCP,
                option,
                (&raw mut value).cast(),
                &raw mut length,
            )
        };
        assert_eq!(
            result,
            0,
            "getsockopt failed: {}",
            std::io::Error::last_os_error()
        );
        value
    }

    fn config_with_heartbeat(heartbeat: Duration) -> TransportConfig {
        // `idle` must stay above `heartbeat` for the config to validate.
        let window = heartbeat.saturating_mul(8);
        TransportConfig {
            timeouts: crate::TimeoutConfig {
                heartbeat,
                idle: window,
                read_write: window,
                admission: window,
                ..crate::TimeoutConfig::default()
            },
            ..TransportConfig::default()
        }
    }

    /// The value the kernel actually holds is the only honest evidence here.
    /// Linux rejects a zero-second keepalive outright, so the bug surfaced there
    /// as a failed `configure_tcp_common`; macOS accepted the call and kept its
    /// 7200 s default, so the requested period vanished without any error. This
    /// asserts the applied period on both.
    #[test]
    fn a_sub_second_heartbeat_still_installs_a_usable_keepalive_period() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();

        for (heartbeat, expected_seconds) in [
            (Duration::from_millis(50), 1),
            (Duration::from_millis(999), 1),
            (Duration::from_millis(1_900), 2),
            (Duration::from_secs(30), 30),
        ] {
            let config = config_with_heartbeat(heartbeat).validate().unwrap();
            configure_tcp_common(&stream, config)
                .unwrap_or_else(|error| panic!("{heartbeat:?} heartbeat was rejected: {error:?}"));
            assert_eq!(
                keepalive_seconds(&stream, KEEPALIVE_TIME),
                expected_seconds,
                "{heartbeat:?} heartbeat produced the wrong keepalive idle period"
            );
            assert_eq!(
                keepalive_seconds(&stream, libc::TCP_KEEPINTVL),
                expected_seconds,
                "{heartbeat:?} heartbeat produced the wrong keepalive probe interval"
            );
        }
    }
}

/// End-to-end shape of the original failure: with a sub-second heartbeat every
/// `connect_secure_tcp` and `accept_secure_tcp` on Linux returned
/// `Io(Os { code: 22, kind: InvalidInput })` out of the socket setup, long
/// before the Noise handshake could run.
#[test]
fn a_sub_second_heartbeat_config_still_completes_the_noise_handshake() {
    let config = TransportConfig {
        timeouts: crate::TimeoutConfig {
            heartbeat: Duration::from_millis(20),
            idle: Duration::from_millis(400),
            read_write: Duration::from_millis(400),
            admission: Duration::from_millis(400),
            ..crate::TimeoutConfig::default()
        },
        ..TransportConfig::default()
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let owner_key = DeviceStaticKey::from_private([41_u8; 32]).unwrap();
    let prelude = ServerPrelude::new(
        "00112233445566778899aabbccddeeff".to_owned(),
        op_collab::SessionId::from("session"),
        op_collab::Epoch(1),
    )
    .unwrap();

    let owner = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        accept_secure_tcp(stream, &owner_key, &prelude, config).map(|_| ())
    });
    let guest = connect_secure_tcp(
        address,
        &DeviceStaticKey::from_private([42_u8; 32]).unwrap(),
        None,
        config,
    );

    assert!(guest.is_ok(), "the initiator failed: {:?}", guest.err());
    let accepted = owner.join().unwrap();
    assert!(
        accepted.is_ok(),
        "the responder failed: {:?}",
        accepted.err()
    );
}
