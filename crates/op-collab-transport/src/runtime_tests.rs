use super::*;
use static_assertions::assert_not_impl_any;

assert_not_impl_any!(TransportTransfer: Clone);

#[test]
fn ticket_transfer_debug_does_not_expose_bytes() {
    let transfer = TransportTransfer {
        class: TransferClass::Ticket,
        transfer_id: 9,
        bytes: Zeroizing::new(vec![211; 17]),
        _reservation: None,
    };
    let debug = format!("{transfer:?}");
    assert!(debug.contains("class: Ticket"));
    assert!(debug.contains("transfer_id: 9"));
    assert!(debug.contains("encoded_len: 17"));
    assert!(!debug.contains("211, 211"));
}

#[test]
fn ticket_deadlines_are_monotonic_and_use_eighty_percent_for_renewal() {
    let identity = verify_initial_ticket(
        &|_: &[u8], expected: &[u8; 32], _: u64| {
            crate::VerifiedTicketClaims::new(
                "https://issuer.example".into(),
                "00000000-0000-0000-0000-000000000001".into(),
                "00000000-0000-0000-0000-000000000002".into(),
                *expected,
                1_250,
            )
        },
        b"ticket",
        &[7; 32],
        "https://issuer.example",
        PeerIdentityPolicy::SameAccount {
            subject: "00000000-0000-0000-0000-000000000001",
        },
        1_000,
    )
    .unwrap();
    let now = Instant::now();
    assert_eq!(
        ticket_deadlines(&identity, 1_000, now).unwrap(),
        (
            now + Duration::from_millis(200),
            now + Duration::from_millis(250)
        )
    );
    assert!(matches!(
        ticket_deadlines(&identity, 1_250, now),
        Err(AdmissionError::TicketExpired)
    ));
}

mod heartbeat_idle {
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::time::Duration;

    use op_collab::{Epoch, Role, SessionId};

    use super::*;
    use crate::{
        accept_secure_tcp, connect_secure_tcp, encrypt_record, write_ciphertext_record,
        AdmissionHello, DeviceStaticKey, JoinIntent, ServerPrelude, TimeoutConfig,
        VerifiedTicketClaims, TRANSPORT_HEARTBEAT_PLAINTEXT,
    };

    const ISSUER: &str = "https://issuer.example";
    const SUBJECT: &str = "00000000-0000-0000-0000-000000000001";
    const OWNER_DEVICE: &str = "00000000-0000-0000-0000-000000000002";
    const GUEST_DEVICE: &str = "00000000-0000-0000-0000-000000000003";
    const DISCOVERY_ID: &str = "00112233445566778899aabbccddeeff";
    const NOW_UNIX_MS: u64 = 1_000;
    const HEARTBEAT_INTERVAL: Duration = Duration::from_millis(20);

    fn config() -> TransportConfig {
        TransportConfig {
            timeouts: TimeoutConfig {
                heartbeat: Duration::from_millis(50),
                // The transfer-idle deadline is what this module tests; 2s
                // keeps the run short while leaving the handshake windows
                // (capped at idle by config validation) wide enough that a
                // loaded CI runner cannot flake the connect/admission phase.
                idle: Duration::from_secs(2),
                read_write: Duration::from_secs(2),
                admission: Duration::from_secs(2),
                ..TimeoutConfig::default()
            },
            ..TransportConfig::default()
        }
    }

    fn prelude() -> ServerPrelude {
        ServerPrelude::new(
            DISCOVERY_ID.to_owned(),
            SessionId::from("session"),
            Epoch(1),
        )
        .unwrap()
    }

    fn verifier(owner_static: [u8; 32], guest_static: [u8; 32]) -> impl TicketVerifier {
        move |ticket: &[u8], expected: &[u8; 32], _now: u64| {
            let (static_key, device) = match ticket {
                b"owner-ticket" => (owner_static, OWNER_DEVICE),
                b"guest-ticket" => (guest_static, GUEST_DEVICE),
                _ => return Err(AdmissionError::Verification),
            };
            if static_key != *expected {
                return Err(AdmissionError::StaticKeyMismatch);
            }
            VerifiedTicketClaims::new(
                ISSUER.into(),
                SUBJECT.into(),
                device.into(),
                static_key,
                61_000,
            )
        }
    }

    #[test]
    fn heartbeat_only_peers_trip_the_receive_transfer_idle_deadline() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let config = config();
        let owner_key = DeviceStaticKey::from_private([21_u8; 32]).unwrap();
        let guest_key = DeviceStaticKey::from_private([22_u8; 32]).unwrap();
        let owner_public = *owner_key.public_key();
        let guest_public = *guest_key.public_key();
        let (stop, stopped) = mpsc::channel::<()>();

        let owner = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut connection = accept_secure_tcp(stream, &owner_key, &prelude(), config).unwrap();
            let hello = AdmissionHello::new(b"owner-ticket".to_vec(), JoinIntent::New).unwrap();
            connection
                .exchange_admission_responder(
                    &hello,
                    &verifier(owner_public, guest_public),
                    ISSUER,
                    PeerIdentityPolicy::SameAccount { subject: SUBJECT },
                    NOW_UNIX_MS,
                    Instant::now(),
                )
                .unwrap();
            connection.authorize_remote(Role::Editor).unwrap();
            connection.activate(Instant::now()).unwrap();
            // A peer that keeps the link warm but never makes any transfer
            // progress. Heartbeats stay just under the record rate limit.
            let mut sent = 0_u32;
            while stopped.try_recv().is_err() {
                let Ok(ciphertext) = encrypt_record(
                    connection.noise.transport_mut(),
                    TRANSPORT_HEARTBEAT_PLAINTEXT,
                ) else {
                    break;
                };
                if write_ciphertext_record(&mut connection.stream, &ciphertext).is_err() {
                    break;
                }
                sent += 1;
                std::thread::sleep(HEARTBEAT_INTERVAL);
            }
            sent
        });

        let (_, mut connection) = connect_secure_tcp(address, &guest_key, None, config).unwrap();
        let hello = AdmissionHello::new(b"guest-ticket".to_vec(), JoinIntent::New).unwrap();
        connection
            .exchange_admission_initiator(
                &hello,
                &verifier(owner_public, guest_public),
                ISSUER,
                PeerIdentityPolicy::SameAccount { subject: SUBJECT },
                NOW_UNIX_MS,
                Instant::now(),
            )
            .unwrap();
        connection.authorize_remote(Role::Owner).unwrap();
        connection.activate(Instant::now()).unwrap();

        let started = Instant::now();
        let error = connection
            .receive_transfer()
            .expect_err("heartbeat traffic must not pin the blocking receive");
        let elapsed = started.elapsed();
        let _ = stop.send(());
        let heartbeats = owner.join().unwrap();

        assert!(
            matches!(error, RuntimeError::IdleTimeout),
            "expected an idle timeout, got {error:?}"
        );
        assert!(
            elapsed >= config.timeouts.idle,
            "returned before the idle deadline: {elapsed:?}"
        );
        assert!(
            elapsed < config.timeouts.idle * 8,
            "idle deadline was re-armed by heartbeats: {elapsed:?}"
        );
        assert!(
            heartbeats > 1,
            "the peer must have kept sending traffic throughout"
        );
    }
}
