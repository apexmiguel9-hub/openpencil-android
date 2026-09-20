use std::num::NonZeroU64;

use op_collab_relay_protocol::{
    CallerDeviceDhPublic, ExpectedDiscoveryId, LocatorKeyId, LocatorSignature, OwnerNoiseStatic,
    RelayAuthExtensionV1, RelayChallengeKeyId, RelayClientHello, RelayLocatorVerifier,
    RelayProtocolError, RelayReauthChallengeV1, RelayReauthResponseV1, RelayRegion, RelayRole,
    RelayServerChallengeV1, RouteCapability, RouteId, UnsignedRelayLocatorV1, VerifiedRelayRoute,
    MAX_RELAY_BEARER_BYTES, MAX_RELAY_REAUTH_CHALLENGE_TEXT_BYTES,
    MAX_RELAY_REAUTH_RESPONSE_TEXT_BYTES, RELAY_CHALLENGE_PROOF_V2_BYTES,
};

const NOW: u64 = 1_000_100;

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

fn route() -> VerifiedRelayRoute {
    let locator = UnsignedRelayLocatorV1::new(
        RelayRegion::Cn,
        RouteId::new([0x11; 16]).unwrap(),
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
    VerifiedRelayRoute::new(locator, RouteCapability::new([0x33; 32]).unwrap())
}

fn challenge(nonce: u8) -> RelayServerChallengeV1 {
    RelayServerChallengeV1::new(
        RelayChallengeKeyId::new("edge-cn-key-1").unwrap(),
        [nonce; 32],
    )
    .unwrap()
}

fn hello(role: RelayRole, caller: u8) -> RelayClientHello {
    RelayClientHello::new_challenge_bound_v2(
        role,
        &route(),
        RelayAuthExtensionV1::new(
            CallerDeviceDhPublic::new([caller; 32]).unwrap(),
            Some(vec![2; RELAY_CHALLENGE_PROOF_V2_BYTES]),
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn challenge_control_is_canonical_bounded_and_redacted() {
    let control = RelayReauthChallengeV1::new(challenge(0x41));
    let text = control.encode_text();
    assert!(text.len() <= MAX_RELAY_REAUTH_CHALLENGE_TEXT_BYTES);
    assert_eq!(
        RelayReauthChallengeV1::decode_text(&text)
            .unwrap()
            .challenge(),
        control.challenge()
    );
    assert!(!format!("{control:?}").contains("edge-cn-key-1"));

    assert_eq!(
        RelayReauthChallengeV1::decode_text("oprrc1_bad").unwrap_err(),
        RelayProtocolError::InvalidChallengePrefix
    );
    assert_eq!(
        RelayReauthChallengeV1::decode_text(&"x".repeat(MAX_RELAY_REAUTH_CHALLENGE_TEXT_BYTES + 1))
            .unwrap_err(),
        RelayProtocolError::ReauthControlTooLong {
            actual: MAX_RELAY_REAUTH_CHALLENGE_TEXT_BYTES + 1,
            maximum: MAX_RELAY_REAUTH_CHALLENGE_TEXT_BYTES,
        }
    );
}

#[test]
fn response_round_trips_exact_bearer_challenge_and_hello() {
    let response = RelayReauthResponseV1::new(
        challenge(0x42),
        b"fresh.ticket_1==",
        hello(RelayRole::Guest, 4),
    )
    .unwrap();
    let text = response.encode_text();
    assert!(text.len() <= MAX_RELAY_REAUTH_RESPONSE_TEXT_BYTES);
    assert!(!text.contains("fresh.ticket"));
    let decoded = RelayReauthResponseV1::decode_text(&text).unwrap();
    assert_eq!(decoded.challenge(), response.challenge());
    assert_eq!(decoded.bearer(), b"fresh.ticket_1==");
    assert_eq!(decoded.hello(), response.hello());
    let debug = format!("{decoded:?}");
    assert_eq!(debug, "RelayReauthResponseV1([REDACTED])");
    assert!(!debug.contains("fresh.ticket"));
}

#[test]
fn response_accepts_exact_maximum_and_rejects_oversize_before_decode() {
    let bearer = vec![b'a'; MAX_RELAY_BEARER_BYTES];
    let maximum_challenge =
        RelayServerChallengeV1::new(RelayChallengeKeyId::new("x".repeat(30)).unwrap(), [1; 32])
            .unwrap();
    let response =
        RelayReauthResponseV1::new(maximum_challenge, &bearer, hello(RelayRole::Owner, 5)).unwrap();
    let text = response.encode_text();
    assert_eq!(text.len(), MAX_RELAY_REAUTH_RESPONSE_TEXT_BYTES);
    assert_eq!(
        RelayReauthResponseV1::decode_text(&text)
            .unwrap()
            .bearer()
            .len(),
        MAX_RELAY_BEARER_BYTES
    );

    let oversized = format!("{}A", text.as_str());
    assert_eq!(
        RelayReauthResponseV1::decode_text(&oversized).unwrap_err(),
        RelayProtocolError::ReauthControlTooLong {
            actual: MAX_RELAY_REAUTH_RESPONSE_TEXT_BYTES + 1,
            maximum: MAX_RELAY_REAUTH_RESPONSE_TEXT_BYTES,
        }
    );
}

#[test]
fn response_parser_rejects_noncanonical_malformed_and_reduced_mode() {
    let valid =
        RelayReauthResponseV1::new(challenge(2), b"ticket", hello(RelayRole::Guest, 6)).unwrap();
    let text = valid.encode_text();
    assert!(matches!(
        RelayReauthResponseV1::decode_text(&format!("{}=", text.as_str())),
        Err(RelayProtocolError::InvalidReauthControlEncoding)
    ));
    assert_eq!(
        RelayReauthResponseV1::decode_text("oprrr1_AA").unwrap_err(),
        RelayProtocolError::Truncated {
            context: "relay reauthentication response",
            expected: 4,
            actual: 1,
        }
    );

    let v1 = RelayClientHello::new(
        RelayRole::Guest,
        &route(),
        RelayAuthExtensionV1::without_possession_proof(CallerDeviceDhPublic::new([7; 32]).unwrap()),
    );
    assert_eq!(
        RelayReauthResponseV1::new(challenge(3), b"ticket", v1).unwrap_err(),
        RelayProtocolError::ReauthRequiresChallengeProof
    );
    assert_eq!(
        RelayReauthResponseV1::new(challenge(4), b"bad=middle", hello(RelayRole::Guest, 8))
            .unwrap_err(),
        RelayProtocolError::InvalidRelayBearer
    );
}

#[test]
fn response_binding_distinguishes_replay_identity_inputs() {
    let original =
        RelayReauthResponseV1::new(challenge(5), b"ticket", hello(RelayRole::Guest, 9)).unwrap();
    let stale =
        RelayReauthResponseV1::new(challenge(6), b"ticket", hello(RelayRole::Guest, 9)).unwrap();
    let wrong_role =
        RelayReauthResponseV1::new(challenge(5), b"ticket", hello(RelayRole::Owner, 9)).unwrap();
    let wrong_key =
        RelayReauthResponseV1::new(challenge(5), b"ticket", hello(RelayRole::Guest, 10)).unwrap();
    assert_ne!(original.encode_text(), stale.encode_text());
    assert_ne!(original.encode_text(), wrong_role.encode_text());
    assert_ne!(original.encode_text(), wrong_key.encode_text());
}
