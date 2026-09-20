use std::num::NonZeroU64;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use op_collab_relay_protocol::{
    relay_challenge_proof_binding_digest, CallerDeviceDhPublic, ExpectedDiscoveryId, LocatorKeyId,
    LocatorSignature, OwnerNoiseStatic, RelayAuthExtensionV1, RelayChallengeKeyId,
    RelayChallengeProofV2, RelayClientHello, RelayHelloAuthMode, RelayLocatorVerifier,
    RelayProtocolError, RelayRegion, RelayRole, RelayServerChallengeV1, RouteCapability, RouteId,
    UnsignedRelayLocatorV1, VerifiedRelayRoute, MAX_RELAY_CHALLENGE_HEADER_BYTES,
    MAX_RELAY_CHALLENGE_KEY_ID_BYTES, RELAY_CHALLENGE_HEADER_NAME, RELAY_CHALLENGE_PREFIX,
    RELAY_CHALLENGE_PROOF_V2_BYTES,
};

const NOW: u64 = 1_000_100;
const BEARER: &[u8] = b"exact.ticket_bytes-_/+=";
const SHARED_SECRET: [u8; 32] = [0x71; 32];

struct AcceptVerifier;

impl RelayLocatorVerifier for AcceptVerifier {
    fn verify(
        &self,
        key_id: &LocatorKeyId,
        _canonical_signing_bytes: &[u8],
        signature: &[u8; 64],
    ) -> bool {
        key_id.as_str() == "locator-key" && signature == &[0x55; 64]
    }
}

fn route(capability: u8, route_id: u8) -> VerifiedRelayRoute {
    let locator = UnsignedRelayLocatorV1::new(
        RelayRegion::Cn,
        RouteId::new([route_id; 16]).unwrap(),
        NonZeroU64::new(7).unwrap(),
        OwnerNoiseStatic::new([0x22; 32]).unwrap(),
        ExpectedDiscoveryId::new("owner-discovery").unwrap(),
        1_000_000,
        1_000_600,
        LocatorKeyId::new("locator-key").unwrap(),
    )
    .unwrap()
    .attach_signature(LocatorSignature::new([0x55; 64]).unwrap())
    .verify(&AcceptVerifier, NOW)
    .unwrap();
    VerifiedRelayRoute::new(locator, RouteCapability::new([capability; 32]).unwrap())
}

fn challenge(key_id: &str, nonce: u8) -> RelayServerChallengeV1 {
    RelayServerChallengeV1::new(RelayChallengeKeyId::new(key_id).unwrap(), [nonce; 32]).unwrap()
}

fn template_hello(role: RelayRole, caller: u8, capability: u8, route_id: u8) -> RelayClientHello {
    RelayClientHello::new_challenge_bound_v2(
        role,
        &route(capability, route_id),
        RelayAuthExtensionV1::without_possession_proof(
            CallerDeviceDhPublic::new([caller; 32]).unwrap(),
        ),
    )
    .unwrap()
}

fn encode_raw_challenge(raw: &[u8]) -> String {
    format!("{RELAY_CHALLENGE_PREFIX}{}", URL_SAFE_NO_PAD.encode(raw))
}

#[test]
fn challenge_header_is_canonical_bounded_and_round_trips() {
    assert_eq!(RELAY_CHALLENGE_HEADER_NAME, "openpencil-relay-challenge");
    for key_id in ["k", &"x".repeat(MAX_RELAY_CHALLENGE_KEY_ID_BYTES)] {
        let challenge = challenge(key_id, 0x31);
        let header = challenge.encode_header();
        assert!(header.starts_with(RELAY_CHALLENGE_PREFIX));
        assert!(!header.contains('='));
        assert!(header.len() <= MAX_RELAY_CHALLENGE_HEADER_BYTES);
        assert_eq!(
            RelayServerChallengeV1::decode_header(&header).unwrap(),
            challenge
        );
    }
    assert_eq!(
        challenge(&"x".repeat(MAX_RELAY_CHALLENGE_KEY_ID_BYTES), 1)
            .encode_header()
            .len(),
        MAX_RELAY_CHALLENGE_HEADER_BYTES
    );
}

#[test]
fn challenge_header_rejects_noncanonical_and_malformed_values() {
    let header = challenge("edge-cn-1", 0x42).encode_header();
    assert_eq!(
        RelayServerChallengeV1::decode_header("OPRC1_A").unwrap_err(),
        RelayProtocolError::InvalidChallengePrefix
    );
    assert_eq!(
        RelayServerChallengeV1::decode_header(&format!("{header}=")).unwrap_err(),
        RelayProtocolError::InvalidChallengeEncoding
    );
    assert_eq!(
        RelayServerChallengeV1::decode_header(&format!(
            "{RELAY_CHALLENGE_PREFIX}{}",
            "A".repeat(MAX_RELAY_CHALLENGE_HEADER_BYTES)
        ))
        .unwrap_err(),
        RelayProtocolError::ChallengeHeaderTooLong {
            actual: RELAY_CHALLENGE_PREFIX.len() + MAX_RELAY_CHALLENGE_HEADER_BYTES,
            maximum: MAX_RELAY_CHALLENGE_HEADER_BYTES,
        }
    );

    assert_eq!(
        RelayServerChallengeV1::decode_header(RELAY_CHALLENGE_PREFIX).unwrap_err(),
        RelayProtocolError::InvalidChallengeEncoding
    );
    assert!(matches!(
        RelayServerChallengeV1::decode_header(&encode_raw_challenge(&[1])),
        Err(RelayProtocolError::Truncated { .. })
    ));

    let mut wrong_version = vec![2, 1, b'k'];
    wrong_version.extend_from_slice(&[1; 32]);
    assert_eq!(
        RelayServerChallengeV1::decode_header(&encode_raw_challenge(&wrong_version)).unwrap_err(),
        RelayProtocolError::UnsupportedChallengeVersion {
            actual: 2,
            expected: 1,
        }
    );

    let mut empty_key_id = vec![1, 0];
    empty_key_id.extend_from_slice(&[1; 32]);
    assert!(matches!(
        RelayServerChallengeV1::decode_header(&encode_raw_challenge(&empty_key_id)),
        Err(RelayProtocolError::InvalidAsciiField {
            field: "challenge_key_id"
        })
    ));

    let mut oversized_key_id = vec![1, 31];
    oversized_key_id.extend_from_slice(&[b'x'; 31]);
    oversized_key_id.extend_from_slice(&[1; 31]);
    assert!(matches!(
        RelayServerChallengeV1::decode_header(&encode_raw_challenge(&oversized_key_id)),
        Err(RelayProtocolError::AsciiFieldTooLong {
            field: "challenge_key_id",
            ..
        })
    ));

    let mut trailing = vec![1, 1, b'k'];
    trailing.extend_from_slice(&[1; 32]);
    trailing.push(0);
    assert!(matches!(
        RelayServerChallengeV1::decode_header(&encode_raw_challenge(&trailing)),
        Err(RelayProtocolError::TrailingBytes { .. })
    ));
}

#[test]
fn challenge_key_and_nonce_constraints_are_fail_closed() {
    for invalid in ["", "has space", "\u{7f}", "é"] {
        assert!(matches!(
            RelayChallengeKeyId::new(invalid),
            Err(RelayProtocolError::InvalidAsciiField {
                field: "challenge_key_id"
            })
        ));
    }
    assert!(matches!(
        RelayChallengeKeyId::new("x".repeat(MAX_RELAY_CHALLENGE_KEY_ID_BYTES + 1)),
        Err(RelayProtocolError::AsciiFieldTooLong { .. })
    ));
    assert_eq!(
        RelayServerChallengeV1::new(RelayChallengeKeyId::new("k").unwrap(), [0; 32]).unwrap_err(),
        RelayProtocolError::ZeroChallengeNonce
    );

    let mut zero_nonce = vec![1, 1, b'k'];
    zero_nonce.extend_from_slice(&[0; 32]);
    assert_eq!(
        RelayServerChallengeV1::decode_header(&encode_raw_challenge(&zero_nonce)).unwrap_err(),
        RelayProtocolError::ZeroChallengeNonce
    );
}

#[test]
fn challenge_bound_mode_preserves_v1_and_validates_present_proof_wire() {
    let route = route(0x33, 0x11);
    let caller = CallerDeviceDhPublic::new([0x44; 32]).unwrap();
    let v1 = RelayClientHello::new(
        RelayRole::Guest,
        &route,
        RelayAuthExtensionV1::without_possession_proof(caller),
    );
    assert_eq!(
        v1.auth_mode(),
        RelayHelloAuthMode::SignedLocatorAndBearerTicketV1
    );

    let v2 = RelayClientHello::new_challenge_bound_v2(
        RelayRole::Guest,
        &route,
        RelayAuthExtensionV1::without_possession_proof(caller),
    )
    .unwrap();
    assert_eq!(v2.auth_mode(), RelayHelloAuthMode::ChallengeBoundX25519V2);
    assert_eq!(RelayClientHello::decode(&v2.encode()).unwrap(), v2);

    let invalid = RelayAuthExtensionV1::new(caller, Some(vec![2; 32])).unwrap();
    assert!(matches!(
        RelayClientHello::new_challenge_bound_v2(RelayRole::Guest, &route, invalid),
        Err(RelayProtocolError::Truncated { .. })
    ));
}

#[test]
fn proof_wire_is_exact_and_versioned() {
    let hello = template_hello(RelayRole::Guest, 0x44, 0x33, 0x11);
    let challenge = challenge("edge-cn-1", 0x42);
    let proof = RelayChallengeProofV2::derive(&SHARED_SECRET, &challenge, BEARER, &hello).unwrap();
    assert_eq!(
        challenge.encode_header(),
        "oprc1_AQllZGdlLWNuLTFCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQg"
    );
    assert_eq!(
        relay_challenge_proof_binding_digest(&challenge, BEARER, &hello).unwrap(),
        [
            0x53, 0xe9, 0xdf, 0xbe, 0x35, 0x2b, 0xe0, 0xb9, 0x9a, 0xbd, 0xf5, 0x03, 0xa7, 0x10,
            0xf8, 0x7b, 0x5e, 0x23, 0x9b, 0x04, 0xb0, 0x13, 0x46, 0x12, 0x87, 0x40, 0xfd, 0x83,
            0x8f, 0x2f, 0xe9, 0xd1,
        ]
    );
    assert_eq!(
        proof.as_bytes(),
        &[
            0x02, 0x04, 0xa6, 0xae, 0x20, 0x0e, 0x53, 0x13, 0xc7, 0xa5, 0x09, 0x76, 0xd4, 0x0d,
            0x84, 0xdc, 0xfd, 0x02, 0xff, 0x0b, 0xd0, 0x28, 0xcb, 0x44, 0x07, 0x10, 0x85, 0x72,
            0x62, 0x2a, 0xa6, 0x6b, 0x71,
        ]
    );
    assert_eq!(proof.as_bytes().len(), RELAY_CHALLENGE_PROOF_V2_BYTES);
    assert_eq!(proof.as_bytes()[0], 2);
    assert_eq!(
        RelayChallengeProofV2::decode(proof.as_bytes()).unwrap(),
        proof
    );
    assert!(matches!(
        RelayChallengeProofV2::decode(&proof.as_bytes()[..32]),
        Err(RelayProtocolError::Truncated { .. })
    ));
    let mut trailing = proof.as_bytes().to_vec();
    trailing.push(0);
    assert!(matches!(
        RelayChallengeProofV2::decode(&trailing),
        Err(RelayProtocolError::TrailingBytes { .. })
    ));
    let mut wrong_version = *proof.as_bytes();
    wrong_version[0] = 1;
    assert_eq!(
        RelayChallengeProofV2::decode(&wrong_version).unwrap_err(),
        RelayProtocolError::UnsupportedChallengeProofVersion {
            actual: 1,
            expected: 2,
        }
    );
}

#[test]
fn proof_binds_challenge_bearer_and_every_hello_route_field() {
    let base_challenge = challenge("edge-cn-1", 0x42);
    let hello = template_hello(RelayRole::Guest, 0x44, 0x33, 0x11);
    let proof =
        RelayChallengeProofV2::derive(&SHARED_SECRET, &base_challenge, BEARER, &hello).unwrap();
    proof
        .verify(&SHARED_SECRET, &base_challenge, BEARER, &hello)
        .unwrap();

    let other_inputs = [
        template_hello(RelayRole::Owner, 0x44, 0x33, 0x11),
        template_hello(RelayRole::Guest, 0x45, 0x33, 0x11),
        template_hello(RelayRole::Guest, 0x44, 0x34, 0x11),
        template_hello(RelayRole::Guest, 0x44, 0x33, 0x12),
    ];
    for other_hello in &other_inputs {
        assert_eq!(
            proof
                .verify(&SHARED_SECRET, &base_challenge, BEARER, other_hello)
                .unwrap_err(),
            RelayProtocolError::ChallengeProofVerificationFailed
        );
    }
    for (other_challenge, other_bearer, other_secret) in [
        (challenge("edge-cn-1", 0x43), BEARER, SHARED_SECRET),
        (challenge("edge-cn-2", 0x42), BEARER, SHARED_SECRET),
        (
            base_challenge.clone(),
            &b"exact.ticket_bytes-_/+= "[..],
            SHARED_SECRET,
        ),
        (base_challenge.clone(), BEARER, [0x72; 32]),
    ] {
        assert_eq!(
            proof
                .verify(&other_secret, &other_challenge, other_bearer, &hello)
                .unwrap_err(),
            RelayProtocolError::ChallengeProofVerificationFailed
        );
    }
}

#[test]
fn proof_binding_normalizes_only_length_and_proof_slot() {
    let challenge = challenge("edge-cn-1", 0x42);
    let route = route(0x33, 0x11);
    let caller = CallerDeviceDhPublic::new([0x44; 32]).unwrap();
    let without_proof = RelayClientHello::new_challenge_bound_v2(
        RelayRole::Guest,
        &route,
        RelayAuthExtensionV1::without_possession_proof(caller),
    )
    .unwrap();
    let proof =
        RelayChallengeProofV2::derive(&SHARED_SECRET, &challenge, BEARER, &without_proof).unwrap();
    let with_proof = RelayClientHello::new_challenge_bound_v2(
        RelayRole::Guest,
        &route,
        RelayAuthExtensionV1::new(caller, Some(proof.as_bytes().to_vec())).unwrap(),
    )
    .unwrap();

    assert_eq!(
        relay_challenge_proof_binding_digest(&challenge, BEARER, &without_proof).unwrap(),
        relay_challenge_proof_binding_digest(&challenge, BEARER, &with_proof).unwrap(),
    );
    proof
        .verify(&SHARED_SECRET, &challenge, BEARER, &with_proof)
        .unwrap();
    assert_eq!(
        RelayClientHello::decode(&with_proof.encode())
            .unwrap()
            .auth_mode(),
        RelayHelloAuthMode::ChallengeBoundX25519V2
    );
}

#[test]
fn proof_rejects_v1_mode_noncontributory_secret_and_mutated_tag() {
    let challenge = challenge("edge-cn-1", 0x42);
    let route = route(0x33, 0x11);
    let caller = CallerDeviceDhPublic::new([0x44; 32]).unwrap();
    let v1 = RelayClientHello::new(
        RelayRole::Guest,
        &route,
        RelayAuthExtensionV1::without_possession_proof(caller),
    );
    assert_eq!(
        RelayChallengeProofV2::derive(&SHARED_SECRET, &challenge, BEARER, &v1).unwrap_err(),
        RelayProtocolError::ChallengeProofRequiresV2AuthMode
    );

    let hello = template_hello(RelayRole::Guest, 0x44, 0x33, 0x11);
    assert_eq!(
        RelayChallengeProofV2::derive(&[0; 32], &challenge, BEARER, &hello).unwrap_err(),
        RelayProtocolError::NonContributoryX25519SharedSecret
    );
    let proof = RelayChallengeProofV2::derive(&SHARED_SECRET, &challenge, BEARER, &hello).unwrap();
    let mut mutated = *proof.as_bytes();
    mutated[7] ^= 1;
    let mutated = RelayChallengeProofV2::decode(&mutated).unwrap();
    assert_eq!(
        mutated
            .verify(&SHARED_SECRET, &challenge, BEARER, &hello)
            .unwrap_err(),
        RelayProtocolError::ChallengeProofVerificationFailed
    );
}

#[test]
fn challenge_and_proof_debug_output_is_redacted() {
    let challenge = challenge("sensitive-hsm-key-id", 0x42);
    let hello = template_hello(RelayRole::Guest, 0x44, 0x33, 0x11);
    let proof = RelayChallengeProofV2::derive(&SHARED_SECRET, &challenge, BEARER, &hello).unwrap();
    let rendered = format!("{challenge:?} {:?} {proof:?}", challenge.key_id());
    assert!(rendered.contains("[REDACTED]"));
    assert!(!rendered.contains("sensitive-hsm-key-id"));
    assert!(!rendered.contains("66, 66"));
}

#[cfg(feature = "random")]
#[test]
fn generated_challenge_has_a_nonzero_nonce() {
    let challenge =
        RelayServerChallengeV1::generate(RelayChallengeKeyId::new("edge-cn-1").unwrap()).unwrap();
    assert!(challenge.nonce().iter().any(|byte| *byte != 0));
    assert_eq!(
        RelayServerChallengeV1::decode_header(&challenge.encode_header()).unwrap(),
        challenge
    );
}
