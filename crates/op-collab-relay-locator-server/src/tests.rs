#![cfg(test)]

use std::{
    ffi::OsStr,
    net::{IpAddr, Ipv4Addr},
    num::{NonZeroU32, NonZeroUsize},
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use ed25519_dalek::Verifier as _;
use ed25519_dalek::{Signer as _, SigningKey};
use op_auth_bridge::{
    CollabJwksCacheLimits, CollabTicketVerifier, OpaqueCollabTicket, StaticTestJwksFetcher,
    TestCollabIssuer, TestCollabTicketSpec,
};
#[cfg(unix)]
use op_collab_relay_control_plane::RelayLocatorSigner;
use op_collab_relay_control_plane::{
    OwnerPublishDraft, OwnerPublishRequest, PairingClaimRequest, PairingCodeStore,
    PairingPublishRequest, RelayLocatorPublishServiceError, RelayPairingService,
    RelayPublishLifetime, SignedLocatorResponse, MAX_PAIRING_PUBLISH_REQUEST_BYTES,
    MAX_PUBLISH_AUTHORIZATION_BYTES, PAIRING_CLAIM_CONTENT_TYPE, PAIRING_CLAIM_PATH,
    PAIRING_PUBLISH_CONTENT_TYPE, PAIRING_PUBLISH_PATH, SEALED_INVITE_CONTENT_TYPE,
};
use op_collab_relay_protocol::{
    ExpectedDiscoveryId, LocatorKeyId, LocatorSignature, OwnerNoiseStatic, RelayRegion,
    UnsignedRelayLocatorV1,
};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

use crate::{
    config::{client_rate_per_second, LOCATOR_CLIENT_RATE_PER_SECOND_ENV},
    http::rate_limit::RateLimiter,
    serve_listener_until, InMemoryPairingStore, LocatorHttpLimits, LocatorPublisher,
    LocatorServerConfig, LocatorServerConfigError, PairingEndpoints,
};
#[cfg(unix)]
use crate::{
    ExpectedUnixPeer, UnixHsmRelayLocatorSigner, HSM_SIGN_REQUEST_BYTES, HSM_SIGN_RESPONSE_BYTES,
};

const OWNER_KEY: [u8; 32] = [0x42; 32];
const PAIRING_OWNER_KEY: [u8; 32] = [0x51; 32];
const PAIRING_GUEST_KEY: [u8; 32] = [0x52; 32];
const TEST_BEARER: &[u8] = b"header.payload.signature";
const TEST_KEY_ID: &str = "locator-test-key";

struct SigningPublisher {
    signing_key: SigningKey,
    delay: Duration,
}

impl SigningPublisher {
    fn immediate() -> Self {
        Self {
            signing_key: SigningKey::from_bytes(&[0x71; 32]),
            delay: Duration::ZERO,
        }
    }
}

impl LocatorPublisher for SigningPublisher {
    fn publish(
        &self,
        request_body: &[u8],
        opaque_ticket: &[u8],
    ) -> Result<SignedLocatorResponse, RelayLocatorPublishServiceError> {
        if opaque_ticket != TEST_BEARER {
            return Err(RelayLocatorPublishServiceError::AuthenticationFailed);
        }
        if !self.delay.is_zero() {
            std::thread::sleep(self.delay);
        }
        let request = OwnerPublishRequest::decode_binary(request_body)
            .map_err(|_| RelayLocatorPublishServiceError::RequestRejected)?;
        let now = unix_now();
        let key_id = LocatorKeyId::new(TEST_KEY_ID).expect("test locator key id");
        let claims = UnsignedRelayLocatorV1::new(
            request.home_region(),
            *request.route_id(),
            request.generation(),
            *request.owner_noise_static(),
            request.expected_discovery_id().clone(),
            now,
            now + request.desired_lifetime().seconds(),
            key_id,
        )
        .map_err(|_| RelayLocatorPublishServiceError::Unavailable)?;
        let signature = LocatorSignature::new(
            self.signing_key
                .sign(&claims.canonical_signing_bytes())
                .to_bytes(),
        )
        .map_err(|_| RelayLocatorPublishServiceError::Unavailable)?;
        SignedLocatorResponse::decode(&claims.attach_signature(signature).encode())
            .map_err(|_| RelayLocatorPublishServiceError::Unavailable)
    }
}

#[tokio::test]
async fn real_http_route_accepts_exact_publish_and_health_is_non_sensitive() {
    let server = TestServer::start(
        Arc::new(SigningPublisher::immediate()),
        LocatorHttpLimits::default(),
    )
    .await;
    let body = publish_request();
    let response = server.request(exact_publish_request(&body)).await;
    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(response.contains("\r\ncontent-type: application/vnd.openpencil.relay-locator-v1\r\n"));
    let response_body = http_body(&response);
    let locator = SignedLocatorResponse::decode(response_body).expect("signed locator");
    assert_eq!(locator.locator().claims().home_region(), RelayRegion::Cn);

    let health = server
        .request(b"GET /healthz HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n".to_vec())
        .await;
    assert!(health.starts_with("HTTP/1.1 204 No Content\r\n"));
    assert!(http_body(&health).is_empty());
    server.stop().await;
}

#[tokio::test]
async fn real_http_route_rejects_method_query_headers_bearer_and_body() {
    let server = TestServer::start(
        Arc::new(SigningPublisher::immediate()),
        LocatorHttpLimits::default(),
    )
    .await;
    let body = publish_request();
    let cases = [
        (
            b"GET /v1/locator HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n".to_vec(),
            "405 Method Not Allowed",
        ),
        (
            publish_wire(
                "/v1/locator?leak=1",
                &body,
                "Bearer header.payload.signature",
                true,
            ),
            "404 Not Found",
        ),
        (
            publish_wire("/v1/locator", &body, "Basic abc", true),
            "401 Unauthorized",
        ),
        (
            publish_wire("/v1/locator", &body, "Bearer bad,token", true),
            "401 Unauthorized",
        ),
        (
            publish_wire(
                "/v1/locator",
                &body,
                "Bearer header.payload.signature",
                false,
            ),
            "400 Bad Request",
        ),
    ];
    for (request, expected) in cases {
        let response = server.request(request).await;
        assert!(
            response.starts_with(&format!("HTTP/1.1 {expected}\r\n")),
            "{response}"
        );
        assert!(!response.contains("header.payload.signature"));
    }

    let maximum_bearer = format!(
        "Bearer {}",
        "a".repeat(MAX_PUBLISH_AUTHORIZATION_BYTES - "Bearer ".len())
    );
    let response = server
        .request(publish_wire("/v1/locator", &body, &maximum_bearer, true))
        .await;
    assert!(
        response.starts_with("HTTP/1.1 401 Unauthorized\r\n"),
        "the bounded 48 KiB ticket envelope must reach authentication: {response:?}"
    );

    let mut duplicate = exact_publish_request(&body);
    let marker = b"Authorization: Bearer header.payload.signature\r\n";
    let insertion = duplicate
        .windows(marker.len())
        .position(|window| window == marker)
        .expect("authorization");
    duplicate.splice(insertion..insertion, marker.iter().copied());
    let response = server.request(duplicate).await;
    assert!(response.starts_with("HTTP/1.1 401 Unauthorized\r\n"));

    let mut oversized = vec![0_u8; body.len() + 1];
    oversized[..body.len()].copy_from_slice(&body);
    let response = server.request(exact_publish_request(&oversized)).await;
    assert!(
        response.starts_with("HTTP/1.1 413 "),
        "the locator route's own body-limit layer must refuse before the handler: {response:?}"
    );
    server.stop().await;
}

#[tokio::test]
async fn real_http_pairing_publish_claim_round_trip_keeps_claim_budget() {
    let (pairing, owner_ticket, guest_ticket) = pairing_fixture(InMemoryPairingStore::default());
    let server = TestServer::start_with_pairing(
        Arc::new(SigningPublisher::immediate()),
        pairing,
        LocatorHttpLimits::default(),
    )
    .await;
    let code_id = [0x31; 16];
    let sealed = b"opaque-sealed-pairing-invite";
    let publish = PairingPublishRequest::new(PAIRING_OWNER_KEY, code_id, 60, sealed.to_vec())
        .expect("pairing publish")
        .encode_binary();
    let response = server
        .request(pairing_publish_wire(&publish, Some(owner_ticket.expose())))
        .await;
    assert!(
        response.starts_with("HTTP/1.1 204 No Content\r\n"),
        "{response:?}"
    );
    assert!(http_body(&response).is_empty());

    let claim = PairingClaimRequest::new(PAIRING_GUEST_KEY, code_id).encode_binary();
    let request = pairing_claim_wire(&claim, Some(guest_ticket.expose()));
    for _ in 0..2 {
        let response = server.request(request.clone()).await;
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response:?}");
        assert!(response.contains(&format!(
            "\r\ncontent-type: {SEALED_INVITE_CONTENT_TYPE}\r\n"
        )));
        assert_eq!(http_body(&response).as_bytes(), sealed);
    }
    server.stop().await;
}

#[tokio::test]
async fn real_http_pairing_rejects_unknown_expired_headers_bearer_and_oversize() {
    let store = InMemoryPairingStore::default();
    let expired_code_id = [0x41; 16];
    store
        .put(
            [0x0F; 32],
            expired_code_id,
            b"expired-sealed-invite".to_vec(),
            unix_now().saturating_sub(2),
            unix_now().saturating_sub(1),
        )
        .expect("seed expired code");
    let (pairing, owner_ticket, guest_ticket) = pairing_fixture(store);
    let server = TestServer::start_with_pairing(
        Arc::new(SigningPublisher::immediate()),
        pairing,
        LocatorHttpLimits::default(),
    )
    .await;

    for code_id in [[0x42; 16], expired_code_id] {
        let claim = PairingClaimRequest::new(PAIRING_GUEST_KEY, code_id).encode_binary();
        let response = server
            .request(pairing_claim_wire(&claim, Some(guest_ticket.expose())))
            .await;
        assert!(
            response.starts_with("HTTP/1.1 404 Not Found\r\n"),
            "{response:?}"
        );
    }

    let publish =
        PairingPublishRequest::new(PAIRING_OWNER_KEY, [0x43; 16], 60, b"sealed-invite".to_vec())
            .expect("pairing publish")
            .encode_binary();
    let response = server
        .request(pairing_wire(
            PAIRING_PUBLISH_PATH,
            &publish,
            "application/octet-stream",
            None,
            Some(owner_ticket.expose()),
        ))
        .await;
    assert!(response.starts_with("HTTP/1.1 400 Bad Request\r\n"));

    let response = server.request(pairing_publish_wire(&publish, None)).await;
    assert!(response.starts_with("HTTP/1.1 401 Unauthorized\r\n"));
    assert!(response.contains("\r\nwww-authenticate: Bearer\r\n"));

    let oversized = vec![0_u8; MAX_PAIRING_PUBLISH_REQUEST_BYTES + 1];
    let response = server
        .request(pairing_publish_wire(
            &oversized,
            Some(owner_ticket.expose()),
        ))
        .await;
    assert!(
        response.starts_with("HTTP/1.1 413 "),
        "the pairing route's own body-limit layer must refuse before the handler: {response:?}"
    );
    server.stop().await;
}

#[tokio::test]
async fn rate_auth_concurrency_and_auth_timeout_fail_closed() {
    let rate_limits = LocatorHttpLimits {
        max_requests_per_second: NonZeroU32::MIN,
        max_client_requests_per_second: NonZeroU32::MIN,
        ..LocatorHttpLimits::default()
    };
    let server = TestServer::start(Arc::new(SigningPublisher::immediate()), rate_limits).await;
    let request = exact_publish_request(&publish_request());
    let malformed = publish_wire(
        "/v1/locator",
        &publish_request(),
        "Bearer header.payload.signature",
        false,
    );
    assert!(server
        .request(malformed)
        .await
        .starts_with("HTTP/1.1 400 Bad Request\r\n"));
    assert!(server
        .request(request.clone())
        .await
        .starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(server
        .request(request)
        .await
        .starts_with("HTTP/1.1 429 Too Many Requests\r\n"));
    server.stop().await;

    let timeout_limits = LocatorHttpLimits {
        auth_timeout: Duration::from_millis(10),
        ..LocatorHttpLimits::default()
    };
    let server = TestServer::start(
        Arc::new(SigningPublisher {
            signing_key: SigningKey::from_bytes(&[0x71; 32]),
            delay: Duration::from_millis(100),
        }),
        timeout_limits,
    )
    .await;
    let response = server
        .request(exact_publish_request(&publish_request()))
        .await;
    assert!(response.starts_with("HTTP/1.1 503 Service Unavailable\r\n"));
    server.stop().await;
}

#[test]
fn per_client_rate_windows_are_independent_across_client_addresses() {
    let limiter = RateLimiter::new(
        NonZeroU32::new(100).expect("global rate"),
        NonZeroU32::new(2).expect("client rate"),
        NonZeroUsize::new(256).expect("connections"),
    );
    let now = Instant::now();
    assert!(limiter.allow(now, client(1)));
    assert!(limiter.allow(now, client(1)));
    assert!(!limiter.allow(now, client(1)));
    for _ in 0..2 {
        assert!(
            limiter.allow(now, client(2)),
            "a second address must carry its own budget"
        );
    }
    assert!(!limiter.allow(now, client(2)));
    assert!(
        limiter.allow(now + Duration::from_millis(1_001), client(1)),
        "a per-client window must recover once it elapses"
    );
}

#[test]
fn one_client_exhausting_its_budget_does_not_throttle_another_client() {
    let limiter = RateLimiter::new(
        NonZeroU32::new(1_000).expect("global rate"),
        NonZeroU32::new(4).expect("client rate"),
        NonZeroUsize::new(256).expect("connections"),
    );
    let now = Instant::now();
    for _ in 0..512 {
        let _ = limiter.allow(now, client(9));
    }
    assert!(!limiter.allow(now, client(9)));
    for _ in 0..4 {
        assert!(
            limiter.allow(now, client(10)),
            "one flooding address must not spend a legitimate owner's budget"
        );
    }
}

#[test]
fn tracked_client_windows_stay_bounded_under_many_distinct_addresses() {
    let limiter = RateLimiter::new(
        NonZeroU32::new(1_000_000).expect("global rate"),
        NonZeroU32::new(4).expect("client rate"),
        NonZeroUsize::new(1).expect("connections"),
    );
    let now = Instant::now();
    let capacity = limiter.max_tracked_clients();
    let mut admitted = 0_usize;
    for index in 0..4_096_u32 {
        if limiter.allow(now, IpAddr::V4(Ipv4Addr::from(index))) {
            admitted += 1;
        }
    }
    assert_eq!(admitted, capacity);
    assert_eq!(limiter.tracked_clients(), capacity);
    assert!(
        limiter.allow(now + Duration::from_millis(1_001), client(200)),
        "expired windows must be pruned so the map recovers"
    );
    assert_eq!(limiter.tracked_clients(), 1);
}

#[test]
fn global_rate_ceiling_still_bounds_total_capacity_across_clients() {
    let limiter = RateLimiter::new(
        NonZeroU32::new(3).expect("global rate"),
        NonZeroU32::new(2).expect("client rate"),
        NonZeroUsize::new(256).expect("connections"),
    );
    let now = Instant::now();
    for index in 1..=3 {
        assert!(limiter.allow(now, client(index)));
    }
    assert!(
        !limiter.allow(now, client(4)),
        "distinct sources under their own budget must still hit the global ceiling"
    );
    assert!(!limiter.allow(now, client(1)));
    assert!(limiter.allow(now + Duration::from_millis(1_001), client(4)));
}

#[test]
fn client_rate_environment_override_is_positive_and_bounded() {
    assert_eq!(client_rate_per_second(None), Ok(None));
    assert_eq!(
        client_rate_per_second(Some(OsStr::new("25"))),
        Ok(NonZeroU32::new(25))
    );
    assert_eq!(
        client_rate_per_second(Some(OsStr::new("10000"))),
        Ok(NonZeroU32::new(10_000))
    );
    for rejected in ["0", "-1", "", " 25", "10001", "abc"] {
        assert_eq!(
            client_rate_per_second(Some(OsStr::new(rejected))),
            Err(LocatorServerConfigError::InvalidEnvNumber {
                name: LOCATOR_CLIENT_RATE_PER_SECOND_ENV,
            }),
            "{rejected}"
        );
    }
}

#[test]
fn limits_reject_a_per_client_rate_above_the_global_rate() {
    let limits = LocatorHttpLimits {
        max_requests_per_second: NonZeroU32::new(4).expect("global rate"),
        max_client_requests_per_second: NonZeroU32::new(5).expect("client rate"),
        ..LocatorHttpLimits::default()
    };
    assert_eq!(
        limits.validate(),
        Err(LocatorServerConfigError::InvalidRateLimit)
    );
    let bounded = LocatorHttpLimits {
        max_client_requests_per_second: NonZeroU32::new(4).expect("client rate"),
        ..limits
    };
    assert_eq!(bounded.validate(), Ok(()));
    assert_eq!(LocatorHttpLimits::default().validate(), Ok(()));
}

fn client(last: u8) -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(203, 0, 113, last))
}

#[cfg(unix)]
#[test]
fn unix_hsm_protocol_authenticates_peer_and_returns_signature() {
    use std::{
        io::{Read as _, Write as _},
        os::unix::net::UnixListener,
        thread,
    };

    let directory = workspace_tempdir();
    let socket_path = directory.path().join("hsm.sock");
    let listener = UnixListener::bind(&socket_path).expect("HSM listener");
    let signing_key = SigningKey::from_bytes(&[0x33; 32]);
    let verifying_key = signing_key.verifying_key();
    let hsm = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut request = Vec::new();
        stream.read_to_end(&mut request).expect("request");
        assert_eq!(request.len(), HSM_SIGN_REQUEST_BYTES);
        assert_eq!(&request[..4], b"OPLS");
        assert_eq!(request[4], 1);
        assert_eq!(request[5], 1);
        let key_length = usize::from(request[6]);
        assert_eq!(&request[7..7 + key_length], TEST_KEY_ID.as_bytes());
        assert!(request[7 + key_length..71].iter().all(|byte| *byte == 0));
        let signature = signing_key.sign(&request[71..]).to_bytes();
        let mut response = [0_u8; HSM_SIGN_RESPONSE_BYTES];
        response[..4].copy_from_slice(b"OPLR");
        response[4] = 1;
        response[5] = 0;
        response[6..].copy_from_slice(&signature);
        stream.write_all(&response).expect("response");
    });
    let signer = UnixHsmRelayLocatorSigner::new(
        &socket_path,
        LocatorKeyId::new(TEST_KEY_ID).expect("key id"),
        current_peer(),
        Duration::from_secs(1),
    )
    .expect("signer");
    signer.validate_socket().expect("safe socket");
    let canonical = [0xA5; 268];
    let signature = signer
        .sign(&LocatorKeyId::new(TEST_KEY_ID).expect("key id"), &canonical)
        .expect("signature");
    verifying_key
        .verify(
            &canonical,
            &ed25519_dalek::Signature::from_bytes(signature.as_bytes()),
        )
        .expect("valid signature");
    hsm.join().expect("HSM");
    let debug = format!("{signer:?}");
    assert!(!debug.contains(socket_path.to_string_lossy().as_ref()));
}

#[cfg(unix)]
#[test]
fn unix_hsm_rejects_symlink_and_wrong_peer_identity() {
    use std::os::unix::{fs::symlink, net::UnixListener};

    let directory = workspace_tempdir();
    let socket_path = directory.path().join("hsm.sock");
    let link_path = directory.path().join("hsm-link.sock");
    let _listener = UnixListener::bind(&socket_path).expect("HSM listener");
    symlink(&socket_path, &link_path).expect("symlink");
    let key_id = LocatorKeyId::new(TEST_KEY_ID).expect("key id");
    let linked = UnixHsmRelayLocatorSigner::new(
        link_path,
        key_id.clone(),
        current_peer(),
        Duration::from_secs(1),
    )
    .expect("shape");
    assert!(linked.validate_socket().is_err());

    let current = current_peer();
    let wrong = UnixHsmRelayLocatorSigner::new(
        socket_path,
        key_id,
        ExpectedUnixPeer {
            uid: current.uid.wrapping_add(1),
            gid: current.gid,
        },
        Duration::from_secs(1),
    )
    .expect("shape");
    assert!(wrong.validate_socket().is_err());
}

struct TestServer {
    address: std::net::SocketAddr,
    shutdown: oneshot::Sender<()>,
    task: tokio::task::JoinHandle<()>,
}

impl TestServer {
    async fn start(publisher: Arc<dyn LocatorPublisher>, limits: LocatorHttpLimits) -> Self {
        let (pairing, _, _) = pairing_fixture(InMemoryPairingStore::default());
        Self::start_with_pairing(publisher, pairing, limits).await
    }

    async fn start_with_pairing(
        publisher: Arc<dyn LocatorPublisher>,
        pairing: Arc<dyn PairingEndpoints>,
        limits: LocatorHttpLimits,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let address = listener.local_addr().expect("address");
        let config = LocatorServerConfig::new(address, limits).expect("config");
        let (shutdown, receiver) = oneshot::channel();
        let task = tokio::spawn(async move {
            serve_listener_until(listener, config, publisher, pairing, async {
                let _ = receiver.await;
            })
            .await
            .expect("server");
        });
        Self {
            address,
            shutdown,
            task,
        }
    }

    async fn request(&self, request: Vec<u8>) -> String {
        let mut stream = TcpStream::connect(self.address).await.expect("connect");
        stream.write_all(&request).await.expect("write");
        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.expect("read");
        String::from_utf8(response).expect("HTTP response")
    }

    async fn stop(self) {
        let _ = self.shutdown.send(());
        self.task.await.expect("join");
    }
}

fn publish_request() -> [u8; 191] {
    OwnerPublishDraft::generate(
        RelayRegion::Cn,
        OwnerNoiseStatic::new(OWNER_KEY).expect("owner key"),
        ExpectedDiscoveryId::new("stable-relay-prelude").expect("discovery"),
        RelayPublishLifetime::new(600).expect("lifetime"),
    )
    .expect("draft")
    .request()
    .encode_binary()
}

fn exact_publish_request(body: &[u8]) -> Vec<u8> {
    publish_wire("/v1/locator", body, "Bearer header.payload.signature", true)
}

fn publish_wire(path: &str, body: &[u8], authorization: &str, include_accept: bool) -> Vec<u8> {
    let accept = if include_accept {
        "Accept: application/vnd.openpencil.relay-locator-v1\r\n"
    } else {
        ""
    };
    let headers = format!(
        "POST {path} HTTP/1.1\r\nHost: test\r\n\
         Content-Type: application/vnd.openpencil.relay-owner-publish-v1\r\n\
         {accept}Authorization: {authorization}\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let mut request = headers.into_bytes();
    request.extend_from_slice(body);
    request
}

fn pairing_fixture(
    store: InMemoryPairingStore,
) -> (
    Arc<dyn PairingEndpoints>,
    OpaqueCollabTicket,
    OpaqueCollabTicket,
) {
    let now = unix_now();
    let issuer = TestCollabIssuer::initial();
    let owner_ticket = issuer
        .issue(&TestCollabTicketSpec::valid_at(now, PAIRING_OWNER_KEY))
        .expect("owner ticket");
    let guest_ticket = issuer
        .issue(&TestCollabTicketSpec::valid_at(now, PAIRING_GUEST_KEY))
        .expect("guest ticket");
    let verifier = CollabTicketVerifier::new(
        TestCollabIssuer::verifier_config().expect("verifier config"),
        StaticTestJwksFetcher::new(issuer.jwks_json().expect("JWKS"), 300),
        CollabJwksCacheLimits::default(),
    )
    .expect("ticket verifier");
    (
        Arc::new(RelayPairingService::new(verifier, store)),
        owner_ticket,
        guest_ticket,
    )
}

fn pairing_publish_wire(body: &[u8], ticket: Option<&[u8]>) -> Vec<u8> {
    pairing_wire(
        PAIRING_PUBLISH_PATH,
        body,
        PAIRING_PUBLISH_CONTENT_TYPE,
        None,
        ticket,
    )
}

fn pairing_claim_wire(body: &[u8], ticket: Option<&[u8]>) -> Vec<u8> {
    pairing_wire(
        PAIRING_CLAIM_PATH,
        body,
        PAIRING_CLAIM_CONTENT_TYPE,
        Some(SEALED_INVITE_CONTENT_TYPE),
        ticket,
    )
}

fn pairing_wire(
    path: &str,
    body: &[u8],
    content_type: &str,
    accept: Option<&str>,
    ticket: Option<&[u8]>,
) -> Vec<u8> {
    let accept = accept.map_or_else(String::new, |value| format!("Accept: {value}\r\n"));
    let authorization = ticket.map_or_else(String::new, |value| {
        format!(
            "Authorization: Bearer {}\r\n",
            std::str::from_utf8(value).expect("ASCII ticket")
        )
    });
    let headers = format!(
        "POST {path} HTTP/1.1\r\nHost: test\r\nContent-Type: {content_type}\r\n\
         {accept}{authorization}Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let mut request = headers.into_bytes();
    request.extend_from_slice(body);
    request
}

fn http_body(response: &str) -> &str {
    response.split_once("\r\n\r\n").map_or("", |(_, body)| body)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs()
}

#[cfg(unix)]
fn current_peer() -> ExpectedUnixPeer {
    ExpectedUnixPeer {
        uid: unsafe { libc::geteuid() },
        gid: unsafe { libc::getegid() },
    }
}

#[cfg(unix)]
fn workspace_tempdir() -> tempfile::TempDir {
    let system_temp = std::env::temp_dir()
        .canonicalize()
        .expect("canonical system temp directory");
    tempfile::tempdir_in(system_temp).expect("system temp directory")
}
