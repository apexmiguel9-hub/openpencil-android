#![cfg(test)]

use std::{
    io::{Read as _, Write as _},
    net::TcpListener,
    num::NonZeroU64,
    thread,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use ed25519_dalek::{Signature, Signer as _, SigningKey, Verifier as _, VerifyingKey};
use op_auth_bridge::{
    CollabJwksCacheLimits, CollabTicketVerifier, OpaqueCollabTicket, StaticTestJwksFetcher,
    TestCollabIssuer, TestCollabTicketSpec, VerifiedCollabClaims,
};
use op_collab_relay_protocol::{
    ExpectedDiscoveryId, LocatorKeyId, LocatorSignature, OwnerNoiseStatic, RelayInviteV1,
    RelayLocatorVerifier, RelayProtocolError, RelayRegion, UnsignedRelayLocatorV1,
};

use crate::{
    OwnerPublishDraft, OwnerPublishRequest, RegionBoundOwnerPublishPolicy, RelayLocatorHttpClient,
    RelayLocatorIssueError, RelayLocatorIssuer, RelayLocatorPublishService,
    RelayLocatorPublishServiceError, RelayLocatorSigner, RelayLocatorSignerError,
    RelayPublishLifetime, SignedLocatorResponse, TicketVerifiedOwnerBinding,
    MAX_ISSUER_CLOCK_SKEW_SECS, OWNER_PUBLISH_REQUEST_BYTES, RELAY_LOCATOR_PUBLISH_PATH,
    SIGNED_LOCATOR_CONTENT_TYPE,
};

const NOW: u64 = 2_000_000_000;
const OWNER_KEY: [u8; 32] = [0x42; 32];
const LOCATOR_KEY_ID: &str = "test-locator-2026-07";

struct TestLocatorSigner {
    signing_key: SigningKey,
    key_id: LocatorKeyId,
}

impl TestLocatorSigner {
    fn new() -> Self {
        Self {
            signing_key: SigningKey::from_bytes(&[0x71; 32]),
            key_id: LocatorKeyId::new(LOCATOR_KEY_ID).expect("test key id"),
        }
    }

    fn verifier(&self) -> TestLocatorVerifier {
        TestLocatorVerifier {
            verifying_key: self.signing_key.verifying_key(),
            key_id: self.key_id.clone(),
        }
    }
}

impl RelayLocatorSigner for TestLocatorSigner {
    fn active_key_id(&self) -> Result<LocatorKeyId, RelayLocatorSignerError> {
        Ok(self.key_id.clone())
    }

    fn sign(
        &self,
        key_id: &LocatorKeyId,
        canonical_signing_bytes: &[u8; 268],
    ) -> Result<LocatorSignature, RelayLocatorSignerError> {
        if key_id != &self.key_id {
            return Err(RelayLocatorSignerError::Rejected);
        }
        LocatorSignature::new(self.signing_key.sign(canonical_signing_bytes).to_bytes())
            .map_err(|_| RelayLocatorSignerError::Internal)
    }
}

struct TestLocatorVerifier {
    verifying_key: VerifyingKey,
    key_id: LocatorKeyId,
}

impl RelayLocatorVerifier for TestLocatorVerifier {
    fn verify(
        &self,
        key_id: &LocatorKeyId,
        canonical_signing_bytes: &[u8],
        signature: &[u8; 64],
    ) -> bool {
        key_id == &self.key_id
            && self
                .verifying_key
                .verify(canonical_signing_bytes, &Signature::from_bytes(signature))
                .is_ok()
    }
}

struct RejectSigner;

impl RelayLocatorSigner for RejectSigner {
    fn active_key_id(&self) -> Result<LocatorKeyId, RelayLocatorSignerError> {
        Ok(LocatorKeyId::new(LOCATOR_KEY_ID).expect("test key id"))
    }

    fn sign(
        &self,
        _key_id: &LocatorKeyId,
        _canonical_signing_bytes: &[u8; 268],
    ) -> Result<LocatorSignature, RelayLocatorSignerError> {
        Err(RelayLocatorSignerError::Unavailable)
    }
}

struct RejectVerifier;

impl RelayLocatorVerifier for RejectVerifier {
    fn verify(
        &self,
        _key_id: &LocatorKeyId,
        _canonical_signing_bytes: &[u8],
        _signature: &[u8; 64],
    ) -> bool {
        false
    }
}

#[test]
fn owner_local_draft_to_ticket_bound_signed_invite_round_trips() {
    let draft = new_draft(RelayRegion::Cn, 600);
    let encoded_request = draft.request().encode_binary();
    let decoded_request = OwnerPublishRequest::decode_binary(&encoded_request).expect("request");
    let claims = verified_claims(OWNER_KEY, NOW + 15 * 60);
    let binding = TicketVerifiedOwnerBinding::from_verified_ticket(&claims);
    let signer = TestLocatorSigner::new();
    let verifier = signer.verifier();
    let response = RelayLocatorIssuer::new(signer)
        .issue_at(&decoded_request, &binding, NOW + 1)
        .expect("locator");
    let response_wire = response.encode();
    let decoded_response = SignedLocatorResponse::decode(&response_wire).expect("response");
    let published = draft
        .complete(decoded_response, &verifier, NOW + 1)
        .expect("invite");

    assert_eq!(published.home_region(), RelayRegion::Cn);
    assert_eq!(published.expires_at_unix(), NOW + 601);
    let invite_code = published.invite_code();
    assert!(invite_code.expose_secret().starts_with("opc1_"));
    let decoded_invite = RelayInviteV1::from_fragment(invite_code.expose_secret()).expect("invite");
    let route = decoded_invite.verify(&verifier, NOW + 1).expect("route");
    assert_eq!(route.locator().claims().home_region(), RelayRegion::Cn);
    assert_eq!(
        route.locator().claims().owner_noise_static().as_bytes(),
        &OWNER_KEY
    );
}

#[test]
fn request_codec_is_fixed_exact_canonical_and_capability_free() {
    let draft = new_draft(RelayRegion::Global, 3_600);
    let encoded = draft.request().encode_binary();
    assert_eq!(encoded.len(), OWNER_PUBLISH_REQUEST_BYTES);
    assert_eq!(
        OwnerPublishRequest::decode_binary(&encoded).expect("canonical"),
        *draft.request()
    );

    for length in [0, 1, OWNER_PUBLISH_REQUEST_BYTES - 1] {
        assert!(matches!(
            OwnerPublishRequest::decode_binary(&encoded[..length]),
            Err(RelayLocatorIssueError::InvalidRequestLength { .. })
        ));
    }
    let mut trailing = encoded.to_vec();
    trailing.push(0);
    assert!(matches!(
        OwnerPublishRequest::decode_binary(&trailing),
        Err(RelayLocatorIssueError::InvalidRequestLength { .. })
    ));

    let mut noncanonical = encoded;
    noncanonical[100] = 1;
    assert_eq!(
        OwnerPublishRequest::decode_binary(&noncanonical),
        Err(RelayLocatorIssueError::NonZeroRequestPadding)
    );
}

#[test]
fn request_codec_rejects_bad_version_generation_and_lifetime() {
    let encoded = new_draft(RelayRegion::Cn, 600).request().encode_binary();
    let mut bad_version = encoded;
    bad_version[0] = 2;
    assert!(matches!(
        OwnerPublishRequest::decode_binary(&bad_version),
        Err(RelayLocatorIssueError::UnsupportedRequestVersion { .. })
    ));

    let mut zero_generation = encoded;
    zero_generation[18..26].fill(0);
    assert_eq!(
        OwnerPublishRequest::decode_binary(&zero_generation),
        Err(RelayLocatorIssueError::Protocol(
            RelayProtocolError::ZeroGeneration
        ))
    );

    let lifetime_offset = OWNER_PUBLISH_REQUEST_BYTES - 4;
    let mut zero_lifetime = encoded;
    zero_lifetime[lifetime_offset..].fill(0);
    assert_eq!(
        OwnerPublishRequest::decode_binary(&zero_lifetime),
        Err(RelayLocatorIssueError::ZeroLifetime)
    );
    let mut long_lifetime = encoded;
    long_lifetime[lifetime_offset..].copy_from_slice(&3_601_u32.to_be_bytes());
    assert!(matches!(
        OwnerPublishRequest::decode_binary(&long_lifetime),
        Err(RelayLocatorIssueError::LifetimeTooLong { .. })
    ));
}

#[test]
fn requested_lifetime_is_nonzero_and_at_most_one_hour() {
    assert_eq!(
        RelayPublishLifetime::new(0),
        Err(RelayLocatorIssueError::ZeroLifetime)
    );
    assert_eq!(
        RelayPublishLifetime::new(3_600)
            .expect("one hour")
            .seconds(),
        3_600
    );
    assert!(matches!(
        RelayPublishLifetime::new(3_601),
        Err(RelayLocatorIssueError::LifetimeTooLong { .. })
    ));
    assert!(matches!(
        RelayPublishLifetime::new(u64::MAX),
        Err(RelayLocatorIssueError::LifetimeTooLong { .. })
    ));
}

#[test]
fn owner_ticket_dh_binding_and_time_are_fail_closed() {
    let draft = new_draft(RelayRegion::Cn, 600);
    let signer = RelayLocatorIssuer::new(TestLocatorSigner::new());

    let wrong_key_claims = verified_claims([0x24; 32], NOW + 900);
    assert!(matches!(
        signer.issue_at(
            draft.request(),
            &TicketVerifiedOwnerBinding::from_verified_ticket(&wrong_key_claims),
            NOW,
        ),
        Err(RelayLocatorIssueError::OwnerBindingRejected)
    ));

    let claims = verified_claims(OWNER_KEY, NOW + 10);
    let binding = TicketVerifiedOwnerBinding::from_verified_ticket(&claims);
    assert!(matches!(
        signer.issue_at(draft.request(), &binding, NOW - 1),
        Err(RelayLocatorIssueError::OwnerBindingRejected)
    ));
    assert!(matches!(
        signer.issue_at(draft.request(), &binding, NOW + 10),
        Err(RelayLocatorIssueError::OwnerBindingRejected)
    ));
}

#[test]
fn current_bound_ticket_can_issue_a_longer_bounded_locator() {
    let draft = new_draft(RelayRegion::Cn, 3_600);
    let claims = verified_claims(OWNER_KEY, NOW + 90);
    let signer = TestLocatorSigner::new();
    let verifier = signer.verifier();
    let response = RelayLocatorIssuer::new(signer)
        .issue_at(
            draft.request(),
            &TicketVerifiedOwnerBinding::from_verified_ticket(&claims),
            NOW,
        )
        .expect("locator");
    assert_eq!(response.locator().claims().expires_at_unix(), NOW + 3_600);
    assert_eq!(
        draft
            .complete(response, &verifier, NOW)
            .expect("invite")
            .expires_at_unix(),
        NOW + 3_600
    );
}

#[test]
fn external_signer_failure_is_typed_and_fail_closed() {
    let draft = new_draft(RelayRegion::Global, 600);
    let claims = verified_claims(OWNER_KEY, NOW + 900);
    assert!(matches!(
        RelayLocatorIssuer::new(RejectSigner).issue_at(
            draft.request(),
            &TicketVerifiedOwnerBinding::from_verified_ticket(&claims),
            NOW,
        ),
        Err(RelayLocatorIssueError::Signer(
            RelayLocatorSignerError::Unavailable
        ))
    ));
}

#[test]
fn owner_rejects_unpinned_or_claim_mismatched_signed_response() {
    let draft = new_draft(RelayRegion::Cn, 600);
    let claims = verified_claims(OWNER_KEY, NOW + 900);
    let signer = TestLocatorSigner::new();
    let response = RelayLocatorIssuer::new(signer)
        .issue_at(
            draft.request(),
            &TicketVerifiedOwnerBinding::from_verified_ticket(&claims),
            NOW,
        )
        .expect("locator");
    assert!(matches!(
        draft.complete(response, &RejectVerifier, NOW),
        Err(RelayLocatorIssueError::Protocol(
            RelayProtocolError::SignatureVerificationFailed
        ))
    ));

    let draft = new_draft(RelayRegion::Cn, 600);
    let signer = TestLocatorSigner::new();
    let verifier = signer.verifier();
    let mismatched = signed_locator_for(
        draft.request(),
        &signer,
        RelayRegion::Global,
        draft.request().expected_discovery_id().clone(),
        NOW,
    );
    assert!(matches!(
        draft.complete(mismatched, &verifier, NOW),
        Err(RelayLocatorIssueError::ResponseBindingMismatch {
            field: "home_region"
        })
    ));
}

#[test]
fn owner_rejects_stale_or_longer_than_requested_response() {
    let draft = new_draft(RelayRegion::Cn, 600);
    let signer = TestLocatorSigner::new();
    let verifier = signer.verifier();
    let stale_not_before = NOW - MAX_ISSUER_CLOCK_SKEW_SECS - 1;
    let stale = signed_locator_at(draft.request(), &signer, stale_not_before, NOW + 100);
    assert!(matches!(
        draft.complete(stale, &verifier, NOW),
        Err(RelayLocatorIssueError::ResponseBindingMismatch {
            field: "not_before_unix"
        })
    ));

    let draft = new_draft(RelayRegion::Cn, 60);
    let signer = TestLocatorSigner::new();
    let verifier = signer.verifier();
    let too_long = signed_locator_at(draft.request(), &signer, NOW, NOW + 61);
    assert!(matches!(
        draft.complete(too_long, &verifier, NOW),
        Err(RelayLocatorIssueError::ResponseBindingMismatch {
            field: "expires_at_unix"
        })
    ));
}

#[test]
fn independent_drafts_use_fresh_route_ids_and_generations() {
    let first = new_draft(RelayRegion::Cn, 600);
    let second = new_draft(RelayRegion::Cn, 600);
    assert_ne!(first.request().route_id(), second.request().route_id());
    assert_ne!(first.request().generation(), second.request().generation());
}

#[test]
fn publish_service_single_sources_ticket_verification_and_issuance() {
    let draft = new_draft(RelayRegion::Cn, 600);
    let (ticket_verifier, ticket) = ticket_material(OWNER_KEY, NOW + 900);
    let signer = TestLocatorSigner::new();
    let locator_verifier = signer.verifier();
    let service = RelayLocatorPublishService::new(
        ticket_verifier,
        signer,
        RegionBoundOwnerPublishPolicy::new(RelayRegion::Cn),
    );
    let response = service
        .publish_at(
            &draft.request().encode_binary(),
            ticket.expose(),
            NOW,
            Instant::now(),
        )
        .expect("published locator");
    let invite = draft
        .complete(response, &locator_verifier, NOW)
        .expect("invite");
    assert_eq!(invite.home_region(), RelayRegion::Cn);

    let draft = new_draft(RelayRegion::Cn, 600);
    assert!(matches!(
        service.publish_at(
            &draft.request().encode_binary(),
            b"not-a-signed-ticket",
            NOW,
            Instant::now(),
        ),
        Err(RelayLocatorPublishServiceError::AuthenticationFailed)
    ));

    let draft = new_draft(RelayRegion::Global, 600);
    let (ticket_verifier, ticket) = ticket_material(OWNER_KEY, NOW + 900);
    let service = RelayLocatorPublishService::new(
        ticket_verifier,
        TestLocatorSigner::new(),
        RegionBoundOwnerPublishPolicy::new(RelayRegion::Cn),
    );
    assert!(matches!(
        service.publish_at(
            &draft.request().encode_binary(),
            ticket.expose(),
            NOW,
            Instant::now(),
        ),
        Err(RelayLocatorPublishServiceError::AuthenticationFailed)
    ));
}

#[test]
fn production_http_endpoint_policy_is_https_exact_and_redirect_free() {
    let valid = RelayLocatorHttpClient::new("https://relay.example/v1/locator", RejectVerifier)
        .expect("HTTPS endpoint");
    assert!(!format!("{valid:?}").contains("relay.example"));

    for invalid in [
        "http://relay.example/v1/locator",
        "https://relay.example/v1/locator/",
        "https://relay.example/v1/locator?ticket=secret",
        "https://relay.example/v1/locator#secret",
        "https://user:pass@relay.example/v1/locator",
        "https://relay.example/other",
    ] {
        assert!(RelayLocatorHttpClient::new(invalid, RejectVerifier).is_err());
    }

    assert!(RelayLocatorHttpClient::new_loopback_http_for_development(
        "http://127.0.0.1:8091/v1/locator",
        RejectVerifier,
        true,
    )
    .is_ok());
    assert!(RelayLocatorHttpClient::new_loopback_http_for_development(
        "http://localhost:8091/v1/locator",
        RejectVerifier,
        true,
    )
    .is_err());
    assert!(RelayLocatorHttpClient::new_loopback_http_for_development(
        "http://127.0.0.1:8091/v1/locator",
        RejectVerifier,
        false,
    )
    .is_err());
}

#[test]
fn concrete_http_client_sends_bounded_binary_and_accepts_signed_locator() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let signer = TestLocatorSigner::new();
    let verifier = signer.verifier();
    let server = thread::spawn(move || serve_one_locator(listener, signer));
    let endpoint = format!("http://{address}{RELAY_LOCATOR_PUBLISH_PATH}");
    let client =
        RelayLocatorHttpClient::new_loopback_http_for_development(&endpoint, verifier, true)
            .expect("debug client");
    let draft = OwnerPublishDraft::generate(
        RelayRegion::Cn,
        OwnerNoiseStatic::new(OWNER_KEY).expect("owner key"),
        ExpectedDiscoveryId::new("stable-relay-prelude").expect("discovery"),
        RelayPublishLifetime::new(60).expect("lifetime"),
    )
    .expect("draft");
    let ticket = OpaqueCollabTicket::new(b"header.payload.signature".to_vec()).expect("ticket");
    let invite = client.publish(draft, &ticket).expect("publish");
    assert!(invite.invite_code().expose_secret().starts_with("opc1_"));
    server.join().expect("server");
}

#[test]
fn all_sensitive_debug_surfaces_are_redacted() {
    let draft = new_draft(RelayRegion::Cn, 600);
    let claims = verified_claims(OWNER_KEY, NOW + 900);
    let binding = TicketVerifiedOwnerBinding::from_verified_ticket(&claims);
    let signer = TestLocatorSigner::new();
    let verifier = signer.verifier();
    let response = RelayLocatorIssuer::new(signer)
        .issue_at(draft.request(), &binding, NOW)
        .expect("locator");
    let response_wire = response.encode();

    assert!(!format!("{:?}", draft.request()).contains("relay-owner-secret"));
    assert!(!format!("{draft:?}").contains("relay-owner-secret"));
    assert!(!format!("{binding:?}").contains(claims.subject()));
    assert!(!format!("{response:?}").contains(&response_wire));
    let published = draft.complete(response, &verifier, NOW).expect("invite");
    let invite_code = published.invite_code();
    assert!(!format!("{published:?}").contains(invite_code.expose_secret()));
    assert!(!format!("{invite_code:?}").contains(invite_code.expose_secret()));
    assert_eq!(
        format!("{:?}", RelayLocatorIssuer::new(RejectSigner)),
        "RelayLocatorIssuer { signer: \"[EXTERNAL]\" }"
    );
}

fn new_draft(region: RelayRegion, lifetime: u64) -> OwnerPublishDraft {
    OwnerPublishDraft::generate_at(
        region,
        OwnerNoiseStatic::new(OWNER_KEY).expect("owner key"),
        ExpectedDiscoveryId::new("relay-owner-secret").expect("discovery"),
        RelayPublishLifetime::new(lifetime).expect("lifetime"),
        NOW,
    )
    .expect("draft")
}

fn verified_claims(dh_public: [u8; 32], expires_at_unix: u64) -> VerifiedCollabClaims {
    let (verifier, ticket) = ticket_material(dh_public, expires_at_unix);
    verifier
        .verify_at(ticket.expose(), &dh_public, NOW, Instant::now())
        .expect("verified ticket")
}

fn ticket_material(
    dh_public: [u8; 32],
    expires_at_unix: u64,
) -> (
    CollabTicketVerifier<StaticTestJwksFetcher>,
    OpaqueCollabTicket,
) {
    let issuer = TestCollabIssuer::initial();
    let mut spec = TestCollabTicketSpec::valid_at(NOW, dh_public);
    spec.expires_at_unix_seconds = expires_at_unix;
    let ticket = issuer.issue(&spec).expect("ticket");
    let verifier = CollabTicketVerifier::new(
        TestCollabIssuer::verifier_config().expect("config"),
        StaticTestJwksFetcher::new(issuer.jwks_json().expect("jwks"), 300),
        CollabJwksCacheLimits::default(),
    )
    .expect("verifier");
    (verifier, ticket)
}

fn signed_locator_for(
    request: &OwnerPublishRequest,
    signer: &TestLocatorSigner,
    region: RelayRegion,
    discovery: ExpectedDiscoveryId,
    now_unix: u64,
) -> SignedLocatorResponse {
    let key_id = signer.active_key_id().expect("key id");
    let claims = UnsignedRelayLocatorV1::new(
        region,
        *request.route_id(),
        request.generation(),
        *request.owner_noise_static(),
        discovery,
        now_unix,
        now_unix + request.desired_lifetime().seconds(),
        key_id.clone(),
    )
    .expect("claims");
    let signature = signer
        .sign(&key_id, &claims.canonical_signing_bytes())
        .expect("signature");
    SignedLocatorResponse::decode(&claims.attach_signature(signature).encode()).expect("response")
}

fn serve_one_locator(listener: TcpListener, signer: TestLocatorSigner) {
    let (mut stream, _) = listener.accept().expect("accept");
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .expect("read timeout");
    let mut request_bytes = Vec::with_capacity(4_096);
    let mut scratch = [0_u8; 1_024];
    let header_end = loop {
        let read = stream.read(&mut scratch).expect("request read");
        assert_ne!(read, 0, "request ended before headers");
        request_bytes.extend_from_slice(&scratch[..read]);
        assert!(request_bytes.len() <= 8 * 1_024);
        if let Some(offset) = request_bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
        {
            break offset + 4;
        }
    };
    let headers = std::str::from_utf8(&request_bytes[..header_end]).expect("headers");
    assert!(headers.starts_with("POST /v1/locator HTTP/1.1\r\n"));
    assert!(
        headers.contains("\r\ncontent-type: application/vnd.openpencil.relay-owner-publish-v1\r\n")
    );
    assert!(headers.contains("\r\naccept: application/vnd.openpencil.relay-locator-v1\r\n"));
    assert!(headers.contains("\r\nauthorization: Bearer header.payload.signature\r\n"));
    let content_length = headers
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length: ")
                .and_then(|value| value.parse::<usize>().ok())
        })
        .expect("content length");
    assert_eq!(content_length, OWNER_PUBLISH_REQUEST_BYTES);
    while request_bytes.len() - header_end < content_length {
        let read = stream.read(&mut scratch).expect("body read");
        assert_ne!(read, 0, "request ended before body");
        request_bytes.extend_from_slice(&scratch[..read]);
    }
    let request =
        OwnerPublishRequest::decode_binary(&request_bytes[header_end..header_end + content_length])
            .expect("publish request");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    let key_id = signer.active_key_id().expect("key id");
    let claims = UnsignedRelayLocatorV1::new(
        request.home_region(),
        *request.route_id(),
        request.generation(),
        *request.owner_noise_static(),
        request.expected_discovery_id().clone(),
        now,
        now + request.desired_lifetime().seconds(),
        key_id.clone(),
    )
    .expect("claims");
    let signature = signer
        .sign(&key_id, &claims.canonical_signing_bytes())
        .expect("signature");
    let body = claims.attach_signature(signature).encode();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {SIGNED_LOCATOR_CONTENT_TYPE}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).expect("response");
}

fn signed_locator_at(
    request: &OwnerPublishRequest,
    signer: &TestLocatorSigner,
    not_before_unix: u64,
    expires_at_unix: u64,
) -> SignedLocatorResponse {
    let key_id = signer.active_key_id().expect("key id");
    let claims = UnsignedRelayLocatorV1::new(
        request.home_region(),
        *request.route_id(),
        NonZeroU64::new(request.generation().get()).expect("generation"),
        *request.owner_noise_static(),
        request.expected_discovery_id().clone(),
        not_before_unix,
        expires_at_unix,
        key_id.clone(),
    )
    .expect("claims");
    let signature = signer
        .sign(&key_id, &claims.canonical_signing_bytes())
        .expect("signature");
    SignedLocatorResponse::decode(&claims.attach_signature(signature).encode()).expect("response")
}

#[path = "pairing_wire_tests.rs"]
mod pairing_wire_tests;
