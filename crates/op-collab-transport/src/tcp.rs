use std::cell::Cell;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::rc::Rc;
use std::time::{Duration, Instant};

use polling::{Event, Events, Poller};
use socket2::{Domain, Protocol, SockRef, Socket, TcpKeepalive, Type};

use crate::noise::run_xx_responder_observed;
use crate::{
    read_server_prelude, run_xx_initiator, write_server_prelude, ConfigError, DeviceStaticKey,
    EncodedServerPrelude, PendingHandshakeGuard, RuntimeError, SecureConnection, ServerPrelude,
    TransportConfig,
};

/// Connects a raw TCP socket for a manual `SocketAddr` endpoint.
pub fn connect_manual(
    address: SocketAddr,
    config: TransportConfig,
) -> Result<TcpStream, RuntimeError> {
    let config = config.validate()?;
    let stream = TcpStream::connect_timeout(&address, config.timeouts.connect)?;
    prepare_tcp_stream(&stream, config)?;
    Ok(stream)
}

/// Connects, reads and binds the raw server prelude, then completes Noise XX.
///
/// `expected_discovery_id` should be supplied for an mDNS-selected endpoint.
/// Manual-address connections pass `None` and inspect the returned prelude.
pub fn connect_secure_tcp(
    address: SocketAddr,
    local_static: &DeviceStaticKey,
    expected_discovery_id: Option<&str>,
    config: TransportConfig,
) -> Result<(EncodedServerPrelude, SecureConnection<TcpStream>), RuntimeError> {
    connect_secure_tcp_inner(
        address,
        local_static,
        expected_discovery_id,
        config,
        None,
        None,
        None,
    )
}

/// Connects and completes Noise within one caller-owned monotonic deadline.
///
/// This is used when a discovery result contains several addresses: each
/// attempt receives a fair slice of one overall join budget, so unreachable or
/// silent addresses cannot multiply the configured connect/handshake timeout.
pub fn connect_secure_tcp_until(
    address: SocketAddr,
    local_static: &DeviceStaticKey,
    expected_discovery_id: Option<&str>,
    config: TransportConfig,
    deadline: Instant,
) -> Result<(EncodedServerPrelude, SecureConnection<TcpStream>), RuntimeError> {
    connect_secure_tcp_inner(
        address,
        local_static,
        expected_discovery_id,
        config,
        Some(deadline),
        None,
        None,
    )
}

/// Connects and completes Noise within a deadline while allowing a host-owned
/// lifecycle token to interrupt the TCP connect and register the live socket.
///
/// `on_connected` runs after TCP establishment but before any prelude or Noise
/// bytes are read. A host can retain a cloned socket and call
/// [`TcpStream::shutdown`] when `is_cancelled` becomes true, which also
/// interrupts blocking prelude, Noise, and admission reads.
pub fn connect_secure_tcp_until_cancellable(
    address: SocketAddr,
    local_static: &DeviceStaticKey,
    expected_discovery_id: Option<&str>,
    config: TransportConfig,
    deadline: Instant,
    is_cancelled: &dyn Fn() -> bool,
    on_connected: &mut dyn FnMut(&TcpStream) -> std::io::Result<()>,
) -> Result<(EncodedServerPrelude, SecureConnection<TcpStream>), RuntimeError> {
    connect_secure_tcp_inner(
        address,
        local_static,
        expected_discovery_id,
        config,
        Some(deadline),
        Some(is_cancelled),
        Some(on_connected),
    )
}

type ConnectedHook<'a> = &'a mut dyn FnMut(&TcpStream) -> std::io::Result<()>;

fn connect_secure_tcp_inner(
    address: SocketAddr,
    local_static: &DeviceStaticKey,
    expected_discovery_id: Option<&str>,
    config: TransportConfig,
    overall_deadline: Option<Instant>,
    is_cancelled: Option<&dyn Fn() -> bool>,
    mut on_connected: Option<ConnectedHook<'_>>,
) -> Result<(EncodedServerPrelude, SecureConnection<TcpStream>), RuntimeError> {
    let config = config.validate()?;
    let connect_timeout = match overall_deadline {
        Some(deadline) => remaining_timeout(deadline, config.timeouts.connect)?,
        None => config.timeouts.connect,
    };
    let mut stream = match is_cancelled {
        Some(is_cancelled) => connect_tcp_cancellable(address, connect_timeout, is_cancelled)?,
        None => TcpStream::connect_timeout(&address, connect_timeout)?,
    };
    configure_tcp_common(&stream, config)?;
    if is_cancelled.is_some_and(|cancelled| cancelled()) {
        return Err(cancelled_io_error().into());
    }
    if let Some(on_connected) = on_connected.as_mut() {
        on_connected(&stream)?;
    }
    let deadline = handshake_deadline(config, overall_deadline)?;
    let (prelude, noise) = {
        let mut io = DeadlineTcp::new(&mut stream, deadline);
        let prelude = read_server_prelude(&mut io)?;
        if expected_discovery_id
            .is_some_and(|expected| expected != prelude.prelude().discovery_id())
        {
            return Err(RuntimeError::DiscoveryIdMismatch);
        }
        let noise = run_xx_initiator(&mut io, local_static, &prelude)?;
        (prelude, noise)
    };
    prepare_tcp_stream(&stream, config)?;
    let connection = SecureConnection::new(stream, noise, config, Instant::now())?;
    Ok((prelude, connection))
}

const CONNECT_CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(10);
const CONNECT_EVENT_KEY: usize = 1;

fn connect_tcp_cancellable(
    address: SocketAddr,
    timeout: Duration,
    is_cancelled: &dyn Fn() -> bool,
) -> std::io::Result<TcpStream> {
    if is_cancelled() {
        return Err(cancelled_io_error());
    }
    let deadline = Instant::now().checked_add(timeout).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "collaboration connect timeout overflowed",
        )
    })?;
    let domain = if address.is_ipv4() {
        Domain::IPV4
    } else {
        Domain::IPV6
    };
    let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))?;
    socket.set_nonblocking(true)?;
    match socket.connect(&address.into()) {
        Ok(()) => {}
        Err(error) if connect_is_in_progress(&error) => {
            wait_for_connect(&socket, deadline, is_cancelled)?;
        }
        Err(error) => return Err(error),
    }
    if is_cancelled() {
        return Err(cancelled_io_error());
    }
    socket.set_nonblocking(false)?;
    Ok(socket.into())
}

fn connect_is_in_progress(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
    ) || error.raw_os_error() == Some(libc::EINPROGRESS)
        || (cfg!(target_os = "windows") && error.raw_os_error() == Some(10_036))
}

fn wait_for_connect(
    socket: &Socket,
    deadline: Instant,
    is_cancelled: &dyn Fn() -> bool,
) -> std::io::Result<()> {
    let poller = Poller::new()?;
    // SAFETY: `socket` remains alive until after `poller` is dropped.
    unsafe {
        poller.add(socket, Event::writable(CONNECT_EVENT_KEY))?;
    }
    let mut events = Events::new();
    loop {
        if is_cancelled() {
            return Err(cancelled_io_error());
        }
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or_else(connect_timeout_error)?;
        events.clear();
        poller.wait(
            &mut events,
            Some(remaining.min(CONNECT_CANCEL_POLL_INTERVAL)),
        )?;
        if events.is_empty() {
            continue;
        }
        if let Some(error) = socket.take_error()? {
            return Err(error);
        }
        match socket.peer_addr() {
            Ok(_) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotConnected => {
                poller.modify(socket, Event::writable(CONNECT_EVENT_KEY))?;
            }
            Err(error) => return Err(error),
        }
    }
}

fn connect_timeout_error() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        "collaboration connection deadline expired",
    )
}

fn cancelled_io_error() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::Interrupted,
        "collaboration connection cancelled",
    )
}

/// Writes the raw prelude and completes the responder side of Noise XX.
///
/// The caller should hold a [`crate::PendingHandshakeGuard`] before invoking
/// this function so connection-flood limits apply before cryptographic work.
/// Prefer [`accept_secure_tcp_guarded`], which also tells the guard when the
/// peer proves liveness.
pub fn accept_secure_tcp(
    stream: TcpStream,
    local_static: &DeviceStaticKey,
    prelude: &ServerPrelude,
    config: TransportConfig,
) -> Result<SecureConnection<TcpStream>, RuntimeError> {
    accept_secure_tcp_inner(stream, local_static, prelude, config, None, &mut || Ok(()))
}

/// Accepts a connection and reports its first valid handshake message to
/// `pending`.
///
/// This is the sanctioned owner-side accept path. The first valid Noise
/// message has an actual `timeouts.handshake_first_message` socket deadline.
/// A silent peer is disconnected and its worker returns before the continuously
/// held pending guard can release its global seat. Valid progress switches the
/// I/O deadline to the full handshake window.
pub fn accept_secure_tcp_guarded(
    stream: TcpStream,
    local_static: &DeviceStaticKey,
    prelude: &ServerPrelude,
    config: TransportConfig,
    pending: &PendingHandshakeGuard,
) -> Result<SecureConnection<TcpStream>, RuntimeError> {
    accept_secure_tcp_inner(
        stream,
        local_static,
        prelude,
        config,
        Some(config.timeouts.handshake_first_message),
        &mut || {
            pending
                .note_handshake_progress()
                .map_err(|_| crate::NoiseTransportError::PendingSeatUnavailable)
        },
    )
}

fn accept_secure_tcp_inner(
    mut stream: TcpStream,
    local_static: &DeviceStaticKey,
    prelude: &ServerPrelude,
    config: TransportConfig,
    first_message_timeout: Option<Duration>,
    on_first_message: &mut dyn FnMut() -> Result<(), crate::NoiseTransportError>,
) -> Result<SecureConnection<TcpStream>, RuntimeError> {
    let config = config.validate()?;
    // The handshake below bounds its reads with `set_read_timeout`, which the
    // OS honours only on a blocking socket. Owner listeners poll with
    // `set_nonblocking(true)`, and BSD/macOS `accept` INHERITS that flag onto
    // the accepted stream (Linux does not), so without this reset the very
    // first responder read returns `WouldBlock` instantly and the owner tears
    // down every inbound connection before the initiator's first Noise frame
    // can arrive. The connect path already normalizes the same way.
    stream.set_nonblocking(false)?;
    configure_tcp_common(&stream, config)?;
    let started_at = Instant::now();
    let deadline =
        started_at
            .checked_add(config.timeouts.handshake)
            .ok_or(ConfigError::InvalidValue {
                field: "timeouts.handshake",
            })?;
    let initial_deadline = match first_message_timeout {
        Some(timeout) => started_at
            .checked_add(timeout)
            .ok_or(ConfigError::InvalidValue {
                field: "timeouts.handshake_first_message",
            })?
            .min(deadline),
        None => deadline,
    };
    let noise = {
        let mut io = DeadlineTcp::new(&mut stream, initial_deadline);
        let deadline_handle = io.deadline_handle();
        let encoded = write_server_prelude(&mut io, prelude)?;
        run_xx_responder_observed(&mut io, local_static, &encoded, &mut || {
            on_first_message()?;
            deadline_handle.set(deadline);
            Ok(())
        })?
    };
    prepare_tcp_stream(&stream, config)?;
    SecureConnection::new(stream, noise, config, Instant::now())
}

/// Applies steady-state TCP options after the bounded handshake phase.
pub fn prepare_tcp_stream(stream: &TcpStream, config: TransportConfig) -> Result<(), RuntimeError> {
    let config = config.validate()?;
    configure_tcp_common(stream, config)?;
    stream.set_read_timeout(Some(config.timeouts.read_write))?;
    stream.set_write_timeout(Some(config.timeouts.read_write))?;
    Ok(())
}

/// Smallest keepalive period the OS socket knobs can express.
///
/// `TCP_KEEPIDLE` / `TCP_KEEPINTVL` on Linux and `TCP_KEEPALIVE` on macOS are
/// whole-second options, so socket2 hands them `Duration::as_secs()`. Anything
/// under a second therefore reaches the kernel as `0`, and the two platforms
/// disagree about what that means: Linux rejects it with `EINVAL`, which fails
/// every connect and accept before a single protocol byte moves, while macOS
/// accepts the call and silently keeps its 2-hour default instead of the
/// requested period. Neither is the configured behaviour.
const MIN_OS_KEEPALIVE_PERIOD: Duration = Duration::from_secs(1);

/// Rounds a configured period up to the whole-second granularity the OS
/// keepalive options accept, never below [`MIN_OS_KEEPALIVE_PERIOD`].
///
/// `TransportConfig::validate` accepts any non-zero `timeouts.heartbeat`, and
/// the protocol's own heartbeat and idle deadlines keep that full `Duration`
/// precision in `SecureConnection` and `ConnectionDriver`. The kernel keepalive
/// is only a coarse backstop underneath them, so rounding up here costs nothing
/// while keeping the config-to-socket mapping total on every platform.
fn os_keepalive_period(period: Duration) -> Duration {
    let whole_seconds = period
        .as_secs()
        .saturating_add(u64::from(period.subsec_nanos() > 0));
    Duration::from_secs(whole_seconds.max(MIN_OS_KEEPALIVE_PERIOD.as_secs()))
}

fn configure_tcp_common(stream: &TcpStream, config: TransportConfig) -> Result<(), RuntimeError> {
    stream.set_nodelay(true)?;
    let socket = SockRef::from(stream);
    socket.set_keepalive(true)?;
    let period = os_keepalive_period(config.timeouts.heartbeat);
    let keepalive = TcpKeepalive::new().with_time(period).with_interval(period);
    socket.set_tcp_keepalive(&keepalive)?;
    Ok(())
}

fn handshake_deadline(
    config: TransportConfig,
    overall_deadline: Option<Instant>,
) -> Result<Instant, RuntimeError> {
    let configured = Instant::now()
        .checked_add(config.timeouts.handshake)
        .ok_or(ConfigError::InvalidValue {
            field: "timeouts.handshake",
        })
        .map_err(RuntimeError::from)?;
    Ok(overall_deadline.map_or(configured, |overall| overall.min(configured)))
}

fn remaining_timeout(deadline: Instant, configured: Duration) -> std::io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .map(|remaining| remaining.min(configured))
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "collaboration connection deadline expired",
            )
        })
}

struct DeadlineTcp<'a> {
    stream: &'a mut TcpStream,
    deadline: Rc<Cell<Instant>>,
}

impl<'a> DeadlineTcp<'a> {
    fn new(stream: &'a mut TcpStream, deadline: Instant) -> Self {
        Self {
            stream,
            deadline: Rc::new(Cell::new(deadline)),
        }
    }

    fn deadline_handle(&self) -> Rc<Cell<Instant>> {
        Rc::clone(&self.deadline)
    }

    fn remaining(&self) -> std::io::Result<Duration> {
        self.deadline
            .get()
            .checked_duration_since(Instant::now())
            .filter(|duration| !duration.is_zero())
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "Noise handshake deadline expired",
                )
            })
    }
}

impl Read for DeadlineTcp<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.stream.set_read_timeout(Some(self.remaining()?))?;
        self.stream.read(buffer)
    }
}

impl Write for DeadlineTcp<'_> {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.stream.set_write_timeout(Some(self.remaining()?))?;
        self.stream.write(buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.set_write_timeout(Some(self.remaining()?))?;
        self.stream.flush()
    }
}

#[cfg(test)]
#[path = "tcp_keepalive_tests.rs"]
mod keepalive_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        read_ciphertext_record, AdmissionHello, AdmissionPhase, JoinIntent, PeerIdentityPolicy,
        TicketVerifier, TransferClass, VerifiedTicketClaims,
    };
    use op_collab::{Bye, ByeReason, CollabMessage, Epoch, FrameEnvelope, Role, SessionId};
    use std::net::TcpListener;
    use std::sync::mpsc;

    const ISSUER: &str = "https://issuer.example";
    const SUBJECT: &str = "00000000-0000-0000-0000-000000000001";
    const OWNER_DEVICE: &str = "00000000-0000-0000-0000-000000000002";
    const GUEST_DEVICE: &str = "00000000-0000-0000-0000-000000000003";
    const NOW_UNIX_MS: u64 = 1_000;

    fn server_prelude() -> ServerPrelude {
        ServerPrelude::new(
            "00112233445566778899aabbccddeeff".to_owned(),
            SessionId::from("session"),
            Epoch(1),
        )
        .unwrap()
    }

    fn verifier(owner_static: [u8; 32], guest_static: [u8; 32]) -> impl TicketVerifier {
        move |ticket: &[u8], expected: &[u8; 32], _now: u64| {
            let (static_key, device, expiry) = match ticket {
                b"owner-ticket" => (owner_static, OWNER_DEVICE, 61_000),
                b"owner-renewed" => (owner_static, OWNER_DEVICE, 121_000),
                b"guest-ticket" => (guest_static, GUEST_DEVICE, 61_000),
                _ => return Err(crate::AdmissionError::Verification),
            };
            if static_key != *expected {
                return Err(crate::AdmissionError::StaticKeyMismatch);
            }
            VerifiedTicketClaims::new(
                ISSUER.into(),
                SUBJECT.into(),
                device.into(),
                static_key,
                expiry,
            )
        }
    }

    #[test]
    fn tcp_noise_mutual_admission_activation_frame_and_rate_limit() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let mut config = TransportConfig::default();
        config.rate.byte_burst = crate::MAX_NOISE_PLAINTEXT_BYTES as u64;
        config.rate.bytes_per_second = 1;
        config.validate().unwrap();

        let owner_key = DeviceStaticKey::from_private([2_u8; 32]).unwrap();
        let guest_key = DeviceStaticKey::from_private([1_u8; 32]).unwrap();
        let owner_public = *owner_key.public_key();
        let guest_public = *guest_key.public_key();
        let (ready_tx, ready_rx) = mpsc::channel();

        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut connection =
                accept_secure_tcp(stream, &owner_key, &server_prelude(), config).unwrap();
            let local = AdmissionHello::new(b"owner-ticket".to_vec(), JoinIntent::New).unwrap();
            let (_, identity) = connection
                .exchange_admission_responder(
                    &local,
                    &verifier(owner_public, guest_public),
                    ISSUER,
                    PeerIdentityPolicy::SameAccount { subject: SUBJECT },
                    NOW_UNIX_MS,
                    Instant::now(),
                )
                .unwrap();
            assert_eq!(identity.claims().device_id(), GUEST_DEVICE);
            connection.authorize_remote(Role::Editor).unwrap();
            connection.activate(Instant::now()).unwrap();
            assert_eq!(connection.admission_state().phase(), AdmissionPhase::Active);
            let frame = connection.receive_frame().unwrap();
            ready_tx.send(()).unwrap();
            let mut stream = connection.into_inner();
            let unexpected = read_ciphertext_record(&mut stream);
            (frame, unexpected)
        });

        let (prelude, mut connection) = connect_secure_tcp(
            address,
            &guest_key,
            Some("00112233445566778899aabbccddeeff"),
            config,
        )
        .unwrap();
        assert_eq!(prelude.prelude().session_id().as_ref(), "session");
        let local = AdmissionHello::new(b"guest-ticket".to_vec(), JoinIntent::New).unwrap();
        let client_now = Instant::now();
        let (_, identity) = connection
            .exchange_admission_initiator(
                &local,
                &verifier(owner_public, guest_public),
                ISSUER,
                PeerIdentityPolicy::SameAccount { subject: SUBJECT },
                NOW_UNIX_MS,
                client_now,
            )
            .unwrap();
        assert_eq!(identity.claims().device_id(), OWNER_DEVICE);
        connection.authorize_remote(Role::Owner).unwrap();
        connection.activate(Instant::now()).unwrap();

        let frame = FrameEnvelope::new(
            SessionId::from("session"),
            Epoch(1),
            CollabMessage::Bye(Bye {
                reason: ByeReason::Normal,
            }),
        );
        connection.send_frame(&frame, Instant::now()).unwrap();
        ready_rx.recv().unwrap();
        let payload = vec![0_u8; crate::MAX_CHUNK_PAYLOAD + 1];
        assert!(matches!(
            connection.send_transfer(TransferClass::Txn, &payload, Instant::now()),
            Err(RuntimeError::RateLimited)
        ));
        assert!(matches!(
            connection.check_ticket_expiry(client_now + Duration::from_secs(60)),
            Err(RuntimeError::Admission(
                crate::AdmissionError::TicketExpired
            ))
        ));
        drop(connection);

        let (received, unexpected) = server.join().unwrap();
        assert_eq!(received.body().kind(), "bye");
        assert!(
            unexpected.is_err(),
            "rate rejection wrote a partial transfer"
        );
    }

    #[test]
    fn selected_discovery_id_mismatch_fails_before_noise() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let config = TransportConfig::default();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            accept_secure_tcp(
                stream,
                &DeviceStaticKey::from_private([3_u8; 32]).unwrap(),
                &server_prelude(),
                config,
            )
            .is_err()
        });
        let result = connect_secure_tcp(
            address,
            &DeviceStaticKey::from_private([4_u8; 32]).unwrap(),
            Some("ffffffffffffffffffffffffffffffff"),
            config,
        );
        assert!(matches!(result, Err(RuntimeError::DiscoveryIdMismatch)));
        assert!(server.join().unwrap());
    }

    /// Owner listeners poll with `set_nonblocking(true)`, and BSD/macOS
    /// `accept` inherits that flag onto the accepted stream. The handshake
    /// bounds its reads with `set_read_timeout`, which the OS honours only on
    /// a blocking socket, so an inherited flag made the first responder read
    /// return `WouldBlock` instantly and killed every inbound connection
    /// before the initiator's first Noise frame could arrive. The accept path
    /// must therefore normalize blocking mode itself.
    #[test]
    fn accept_completes_on_a_stream_inheriting_the_listener_nonblocking_flag() {
        let config = TransportConfig::default();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let owner_key = DeviceStaticKey::from_private([31_u8; 32]).unwrap();
        let server = std::thread::spawn(move || {
            let (stream, _) = loop {
                match listener.accept() {
                    Ok(pair) => break pair,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("accept failed: {error}"),
                }
            };
            // Simulate the platform that hands back an inherited flag even
            // where the host OS would not, so the guard is exercised on every
            // target rather than only on BSD/macOS.
            stream.set_nonblocking(true).unwrap();
            accept_secure_tcp(stream, &owner_key, &server_prelude(), config).is_ok()
        });

        // Hold the initiator back so the responder's first read provably runs
        // against an empty receive buffer. Without this the kernel may have
        // already buffered the initiator's frame, and the read would succeed
        // even on a nonblocking socket — letting the test pass with the
        // blocking-mode reset removed.
        std::thread::sleep(Duration::from_millis(120));
        let connected = connect_secure_tcp(
            address,
            &DeviceStaticKey::from_private([32_u8; 32]).unwrap(),
            Some("00112233445566778899aabbccddeeff"),
            config,
        );
        assert!(
            server.join().unwrap(),
            "a nonblocking accepted stream must still complete the handshake"
        );
        assert!(connected.is_ok(), "the initiator must complete too");
    }

    #[test]
    fn a_guarded_accept_keeps_the_pending_seat_after_the_first_handshake_message() {
        use crate::{ConnectionLimiter, ConnectionLimits, TimeoutConfig};

        let config = TransportConfig {
            timeouts: TimeoutConfig {
                handshake_first_message: Duration::from_millis(200),
                ..TimeoutConfig::default()
            },
            ..TransportConfig::default()
        };
        let limiter =
            ConnectionLimiter::with_timeouts(ConnectionLimits::default(), config.timeouts).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let owner_key = DeviceStaticKey::from_private([11_u8; 32]).unwrap();
        let limiter_for_owner = limiter.clone();
        let server = std::thread::spawn(move || {
            let (stream, peer) = listener.accept().unwrap();
            let pending = limiter_for_owner.try_begin_handshake(peer.ip()).unwrap();
            let accepted =
                accept_secure_tcp_guarded(stream, &owner_key, &server_prelude(), config, &pending);
            (accepted.is_ok(), pending)
        });

        let (_, connection) = connect_secure_tcp(
            address,
            &DeviceStaticKey::from_private([12_u8; 32]).unwrap(),
            Some("00112233445566778899aabbccddeeff"),
            config,
        )
        .unwrap();
        let (accepted, pending) = server.join().unwrap();
        assert!(accepted);

        let long_after = Instant::now() + Duration::from_secs(30);
        assert!(
            pending.holds_global_seat_at(long_after),
            "a peer that completed the handshake keeps its pending seat"
        );
        drop(connection);
    }

    #[test]
    fn silent_guarded_accept_exits_at_first_message_deadline_before_releasing_its_seat() {
        use crate::{ConnectionLimiter, ConnectionLimits, TimeoutConfig};

        let config = TransportConfig {
            connections: ConnectionLimits {
                max_pending_handshakes: 1,
                max_pending_handshakes_per_ip: 1,
                ..ConnectionLimits::default()
            },
            timeouts: TimeoutConfig {
                handshake: Duration::from_secs(2),
                handshake_first_message: Duration::from_millis(100),
                ..TimeoutConfig::default()
            },
            ..TransportConfig::default()
        };
        let limiter =
            ConnectionLimiter::with_timeouts(config.connections, config.timeouts).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let owner_key = DeviceStaticKey::from_private([13_u8; 32]).unwrap();
        let limiter_for_owner = limiter.clone();
        let (started_tx, started_rx) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let (stream, peer) = listener.accept().unwrap();
            let pending = limiter_for_owner.try_begin_handshake(peer.ip()).unwrap();
            started_tx.send(()).unwrap();
            let result =
                accept_secure_tcp_guarded(stream, &owner_key, &server_prelude(), config, &pending);
            drop(pending);
            result
        });

        let silent = TcpStream::connect(address).unwrap();
        started_rx.recv().unwrap();
        assert_eq!(limiter.counts().unwrap().pending_handshakes, 1);
        assert!(matches!(
            limiter.try_begin_handshake("198.51.100.9".parse().unwrap()),
            Err(crate::ConnectionLimitError::PendingHandshakesFull)
        ));

        let started = Instant::now();
        assert!(server.join().unwrap().is_err());
        assert!(started.elapsed() < config.timeouts.handshake);
        assert_eq!(limiter.counts().unwrap().pending_handshakes, 0);
        let replacement = limiter
            .try_begin_handshake("198.51.100.9".parse().unwrap())
            .unwrap();
        drop(replacement);
        drop(silent);
    }

    #[test]
    fn expired_overall_connect_deadline_fails_before_socket_io() {
        let result = connect_secure_tcp_until(
            "127.0.0.1:9".parse().unwrap(),
            &DeviceStaticKey::from_private([4_u8; 32]).unwrap(),
            None,
            TransportConfig::default(),
            Instant::now() - Duration::from_millis(1),
        );
        assert!(matches!(
            result,
            Err(RuntimeError::Io(error)) if error.kind() == std::io::ErrorKind::TimedOut
        ));
    }
}
