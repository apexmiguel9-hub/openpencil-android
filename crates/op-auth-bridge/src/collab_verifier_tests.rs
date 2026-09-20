//! Verifier unit tests split out of `collab_verifier.rs` for the file cap.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use ed25519_dalek::{Signer as _, SigningKey};
use serde_json::{json, Value};

use super::*;
use crate::{
    StaticTestJwksFetcher, TestCollabIssuer, TestCollabTicketSpec, COLLAB_TICKET_AUDIENCE,
    COLLAB_TICKET_SCOPE, COLLAB_TICKET_VERSION, MAX_COLLAB_PROFILE_AVATAR_URL_BYTES,
    MAX_COLLAB_PROFILE_DISPLAY_NAME_CHARS, TEST_AVATAR_URL, TEST_COLLAB_ISSUER, TEST_DEVICE_ID,
    TEST_DISPLAY_NAME, TEST_SUBJECT, TEST_TICKET_ID,
};

const NOW: u64 = 2_000_000_000;
const CHANNEL_BINDING: [u8; 32] = [0x42; 32];
const OTHER_CHANNEL_BINDING: [u8; 32] = [0x24; 32];
const TEST_KEY_A_SEED: [u8; 32] = [0x11; 32];

fn test_verifier() -> CollabTicketVerifier<StaticTestJwksFetcher> {
    let issuer = TestCollabIssuer::initial();
    CollabTicketVerifier::new(
        TestCollabIssuer::verifier_config().unwrap(),
        StaticTestJwksFetcher::new(issuer.jwks_json().unwrap(), 300),
        CollabJwksCacheLimits::default(),
    )
    .unwrap()
}

fn claims() -> Value {
    json!({
        "iss": TEST_COLLAB_ISSUER,
        "aud": COLLAB_TICKET_AUDIENCE,
        "ver": COLLAB_TICKET_VERSION,
        "sub": TEST_SUBJECT,
        "device_id": TEST_DEVICE_ID,
        "dh_pub_x25519": URL_SAFE_NO_PAD.encode(CHANNEL_BINDING),
        "scope": COLLAB_TICKET_SCOPE,
        "iat": NOW,
        "nbf": NOW,
        "exp": NOW + 900,
        "jti": TEST_TICKET_ID,
        "display_name": TEST_DISPLAY_NAME,
        "avatar_url": TEST_AVATAR_URL,
    })
}

fn header() -> Value {
    json!({
        "alg": COLLAB_JWS_ALGORITHM,
        "typ": COLLAB_JWS_TYPE,
        "kid": "test_key_A",
    })
}

fn sign(header: &Value, claims: &Value) -> Vec<u8> {
    let header = URL_SAFE_NO_PAD.encode(serde_json::to_vec(header).unwrap());
    let claims = URL_SAFE_NO_PAD.encode(serde_json::to_vec(claims).unwrap());
    sign_encoded_segments(&header, &claims)
}

fn sign_encoded_segments(header_segment: &str, claims_segment: &str) -> Vec<u8> {
    let signing_input = format!("{header_segment}.{claims_segment}");
    let signature = SigningKey::from_bytes(&TEST_KEY_A_SEED)
        .sign(signing_input.as_bytes())
        .to_bytes();
    format!("{signing_input}.{}", URL_SAFE_NO_PAD.encode(signature)).into_bytes()
}

fn verify_claims(
    verifier: &CollabTicketVerifier<StaticTestJwksFetcher>,
    claims: &Value,
    now_unix_seconds: u64,
) -> Result<VerifiedCollabClaims, CollabVerifyError> {
    verifier.verify_at(
        &sign(&header(), claims),
        &CHANNEL_BINDING,
        now_unix_seconds,
        Instant::now(),
    )
}

#[test]
fn cancellable_verification_has_closed_cancelled_error() {
    assert_eq!(
        test_verifier().verify_at_cancellable(
            b"not-a-ticket",
            &CHANNEL_BINDING,
            NOW,
            Instant::now(),
            &|| true,
        ),
        Err(CollabVerifyError::Cancelled)
    );
}

#[test]
fn verifies_the_frozen_profile_and_redacts_identity_debug_output() {
    let issuer = TestCollabIssuer::initial();
    let ticket = issuer
        .issue(&TestCollabTicketSpec::valid_at(NOW, CHANNEL_BINDING))
        .unwrap();
    let verifier = test_verifier();
    let verified = verifier
        .verify_at(ticket.expose(), &CHANNEL_BINDING, NOW, Instant::now())
        .unwrap();

    assert_eq!(verified.issuer(), TEST_COLLAB_ISSUER);
    assert_eq!(verified.subject(), TEST_SUBJECT);
    assert_eq!(verified.device_id(), TEST_DEVICE_ID);
    assert_eq!(verified.dh_pub_x25519(), &CHANNEL_BINDING);
    assert_eq!(verified.issued_at_unix_seconds(), NOW);
    assert_eq!(verified.not_before_unix_seconds(), NOW);
    assert_eq!(verified.expires_at_unix_seconds(), NOW + 900);
    assert_eq!(verified.expires_at_unix_ms(), (NOW + 900) * 1_000);
    assert_eq!(verified.ticket_id(), TEST_TICKET_ID);
    assert_eq!(verified.display_name(), Some(TEST_DISPLAY_NAME));
    assert_eq!(verified.avatar_url(), Some(TEST_AVATAR_URL));
    assert_eq!(verifier.cached_key_count().unwrap(), 1);

    let debug = format!("{verified:?}");
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains(TEST_SUBJECT));
    assert!(!debug.contains(TEST_DEVICE_ID));
    assert!(!debug.contains(TEST_TICKET_ID));
    assert!(!debug.contains(TEST_DISPLAY_NAME));
    assert!(!debug.contains(TEST_AVATAR_URL));

    let spec_debug = format!("{:?}", TestCollabTicketSpec::valid_at(NOW, CHANNEL_BINDING));
    assert!(!spec_debug.contains(TEST_SUBJECT));
    assert!(!spec_debug.contains(TEST_DEVICE_ID));
    assert!(!spec_debug.contains(TEST_DISPLAY_NAME));
    assert!(!spec_debug.contains(TEST_AVATAR_URL));
}

#[test]
fn accepts_legacy_ticket_without_optional_signed_profile() {
    let verifier = test_verifier();
    let mut legacy = claims();
    legacy.as_object_mut().unwrap().remove("display_name");
    legacy.as_object_mut().unwrap().remove("avatar_url");
    let verified = verify_claims(&verifier, &legacy, NOW).unwrap();
    assert_eq!(verified.display_name(), None);
    assert_eq!(verified.avatar_url(), None);
}

#[test]
fn production_signed_policy_path_never_falls_back_to_raw_jwks() {
    let issuer = TestCollabIssuer::initial();
    let ticket = issuer
        .issue(&TestCollabTicketSpec::valid_at(NOW, CHANNEL_BINDING))
        .unwrap();
    let verifier = CollabTicketVerifier::production(StaticTestJwksFetcher::new(
        issuer.jwks_json().unwrap(),
        300,
    ))
    .unwrap();

    assert!(verifier.config().uses_signed_policy());
    assert_eq!(
        verifier.verify_at(ticket.expose(), &CHANNEL_BINDING, NOW, Instant::now()),
        Err(CollabVerifyError::Jwks(CollabJwksError::Policy(
            crate::CollabUnionPolicyError::MalformedJson
        )))
    );
}

#[test]
fn enforces_signature_profile_and_channel_binding() {
    let verifier = test_verifier();

    let mut wrong_algorithm = header();
    wrong_algorithm["alg"] = json!("EdDSA");
    assert_eq!(
        verifier.verify_at(
            &sign(&wrong_algorithm, &claims()),
            &CHANNEL_BINDING,
            NOW,
            Instant::now()
        ),
        Err(CollabVerifyError::WrongAlgorithm)
    );

    let mut wrong_type = header();
    wrong_type["typ"] = json!("JWT");
    assert_eq!(
        verifier.verify_at(
            &sign(&wrong_type, &claims()),
            &CHANNEL_BINDING,
            NOW,
            Instant::now()
        ),
        Err(CollabVerifyError::WrongType)
    );

    let mut extra_header = header();
    extra_header["jku"] = json!("https://attacker.invalid/jwks");
    assert_eq!(
        verifier.verify_at(
            &sign(&extra_header, &claims()),
            &CHANNEL_BINDING,
            NOW,
            Instant::now()
        ),
        Err(CollabVerifyError::MalformedJson { part: "header" })
    );

    assert_eq!(
        verifier.verify_at(
            &sign(&header(), &claims()),
            &OTHER_CHANNEL_BINDING,
            NOW,
            Instant::now()
        ),
        Err(CollabVerifyError::ChannelBindingMismatch)
    );
    assert_eq!(
        verifier.verify_at(&sign(&header(), &claims()), &[0; 32], NOW, Instant::now()),
        Err(CollabVerifyError::InvalidChannelBinding)
    );
}

#[test]
fn rejects_tampering_and_unknown_claims() {
    let verifier = test_verifier();
    let mut signed = sign(&header(), &claims());
    let first_separator = signed.iter().position(|byte| *byte == b'.').unwrap();
    let claims_byte = first_separator + 2;
    signed[claims_byte] = if signed[claims_byte] == b'A' {
        b'B'
    } else {
        b'A'
    };
    assert_eq!(
        verifier.verify_at(&signed, &CHANNEL_BINDING, NOW, Instant::now()),
        Err(CollabVerifyError::InvalidSignature)
    );

    let mut extra_claim = claims();
    extra_claim["role"] = json!("owner");
    assert_eq!(
        verify_claims(&verifier, &extra_claim, NOW),
        Err(CollabVerifyError::MalformedJson { part: "claims" })
    );
}

#[test]
fn rejects_invalid_identity_authorization_and_time_claims() {
    let verifier = test_verifier();

    let mut invalid = claims();
    invalid["aud"] = json!("another-service");
    assert_eq!(
        verify_claims(&verifier, &invalid, NOW),
        Err(CollabVerifyError::InvalidAudience)
    );
    invalid = claims();
    invalid["ver"] = json!(2);
    assert_eq!(
        verify_claims(&verifier, &invalid, NOW),
        Err(CollabVerifyError::InvalidVersion)
    );
    invalid = claims();
    invalid["scope"] = json!("collab:admin");
    assert_eq!(
        verify_claims(&verifier, &invalid, NOW),
        Err(CollabVerifyError::InvalidScope)
    );
    invalid = claims();
    invalid["sub"] = json!("123E4567-e89b-12d3-a456-426614174000");
    assert_eq!(
        verify_claims(&verifier, &invalid, NOW),
        Err(CollabVerifyError::InvalidSubject)
    );
    invalid = claims();
    invalid["device_id"] = json!("00000000-0000-0000-0000-000000000000");
    assert_eq!(
        verify_claims(&verifier, &invalid, NOW),
        Err(CollabVerifyError::InvalidDeviceId)
    );
    invalid = claims();
    invalid["jti"] = json!("not.a.canonical.id");
    assert_eq!(
        verify_claims(&verifier, &invalid, NOW),
        Err(CollabVerifyError::InvalidTicketId)
    );
    invalid = claims();
    invalid["dh_pub_x25519"] = json!("AA");
    assert_eq!(
        verify_claims(&verifier, &invalid, NOW),
        Err(CollabVerifyError::InvalidChannelBinding)
    );
    invalid = claims();
    invalid["display_name"] = json!(" leading-space");
    assert_eq!(
        verify_claims(&verifier, &invalid, NOW),
        Err(CollabVerifyError::InvalidDisplayName)
    );
    invalid = claims();
    invalid["display_name"] = json!("x".repeat(MAX_COLLAB_PROFILE_DISPLAY_NAME_CHARS + 1));
    assert_eq!(
        verify_claims(&verifier, &invalid, NOW),
        Err(CollabVerifyError::InvalidDisplayName)
    );
    invalid = claims();
    invalid["avatar_url"] = json!("http://cdn.test.invalid/avatar.png");
    assert_eq!(
        verify_claims(&verifier, &invalid, NOW),
        Err(CollabVerifyError::InvalidAvatarUrl)
    );
    invalid = claims();
    invalid["avatar_url"] = json!(format!(
        "https://cdn.test.invalid/{}",
        "a".repeat(MAX_COLLAB_PROFILE_AVATAR_URL_BYTES)
    ));
    assert_eq!(
        verify_claims(&verifier, &invalid, NOW),
        Err(CollabVerifyError::InvalidAvatarUrl)
    );

    invalid = claims();
    invalid["iat"] = json!(NOW + 1);
    assert_eq!(
        verify_claims(&verifier, &invalid, NOW),
        Err(CollabVerifyError::InvalidTimestamps)
    );
    invalid = claims();
    invalid["nbf"] = json!(NOW + 61);
    assert_eq!(
        verify_claims(&verifier, &invalid, NOW),
        Err(CollabVerifyError::NotYetValid)
    );
    invalid = claims();
    invalid["iat"] = json!(NOW - 900);
    invalid["nbf"] = json!(NOW - 900);
    invalid["exp"] = json!(NOW - 61);
    assert_eq!(
        verify_claims(&verifier, &invalid, NOW),
        Err(CollabVerifyError::Expired)
    );
    invalid = claims();
    invalid["exp"] = json!(NOW + 901);
    assert_eq!(
        verify_claims(&verifier, &invalid, NOW),
        Err(CollabVerifyError::LifetimeTooLong)
    );
    invalid = claims();
    invalid["iat"] = json!(u64::MAX - 1);
    invalid["nbf"] = json!(u64::MAX - 1);
    invalid["exp"] = json!(u64::MAX);
    assert_eq!(
        verify_claims(&verifier, &invalid, u64::MAX - 1),
        Err(CollabVerifyError::ExpiryOverflow)
    );
}

#[test]
fn rejects_malformed_and_oversized_compact_inputs_before_fetch() {
    let verifier = test_verifier();
    assert_eq!(
        verifier.verify_at(b"", &CHANNEL_BINDING, NOW, Instant::now()),
        Err(CollabVerifyError::InvalidTicketSize {
            maximum: MAX_COLLAB_TICKET_BYTES
        })
    );
    assert_eq!(
        verifier.verify_at(b"only.two", &CHANNEL_BINDING, NOW, Instant::now()),
        Err(CollabVerifyError::MalformedCompactJws)
    );
    let padded_header = format!(
        "{}=.{}.{}",
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header()).unwrap()),
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims()).unwrap()),
        URL_SAFE_NO_PAD.encode([0; 64])
    );
    assert_eq!(
        verifier.verify_at(
            padded_header.as_bytes(),
            &CHANNEL_BINDING,
            NOW,
            Instant::now()
        ),
        Err(CollabVerifyError::InvalidBase64 { part: "header" })
    );
    let mut invalid_key_id = header();
    invalid_key_id["kid"] = json!("bad.key");
    assert_eq!(
        verifier.verify_at(
            &sign(&invalid_key_id, &claims()),
            &CHANNEL_BINDING,
            NOW,
            Instant::now()
        ),
        Err(CollabVerifyError::InvalidKeyId)
    );
    let short_signature = format!(
        "{}.{}.{}",
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header()).unwrap()),
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims()).unwrap()),
        URL_SAFE_NO_PAD.encode([0; 63])
    );
    assert_eq!(
        verifier.verify_at(
            short_signature.as_bytes(),
            &CHANNEL_BINDING,
            NOW,
            Instant::now()
        ),
        Err(CollabVerifyError::InvalidSignature)
    );
    let oversized_header = URL_SAFE_NO_PAD.encode(vec![b'a'; MAX_COLLAB_JWS_HEADER_BYTES + 1]);
    let valid_claims = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims()).unwrap());
    assert_eq!(
        verifier.verify_at(
            &sign_encoded_segments(&oversized_header, &valid_claims),
            &CHANNEL_BINDING,
            NOW,
            Instant::now()
        ),
        Err(CollabVerifyError::SegmentTooLarge {
            part: "header",
            maximum: MAX_COLLAB_JWS_HEADER_BYTES
        })
    );
    let valid_header = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header()).unwrap());
    let oversized_claims = URL_SAFE_NO_PAD.encode(vec![b'a'; MAX_COLLAB_JWS_CLAIMS_BYTES + 1]);
    assert_eq!(
        verifier.verify_at(
            &sign_encoded_segments(&valid_header, &oversized_claims),
            &CHANNEL_BINDING,
            NOW,
            Instant::now()
        ),
        Err(CollabVerifyError::SegmentTooLarge {
            part: "claims",
            maximum: MAX_COLLAB_JWS_CLAIMS_BYTES
        })
    );
    assert_eq!(
        verifier.verify_at(
            &vec![b'a'; MAX_COLLAB_TICKET_BYTES + 1],
            &CHANNEL_BINDING,
            NOW,
            Instant::now()
        ),
        Err(CollabVerifyError::InvalidTicketSize {
            maximum: MAX_COLLAB_TICKET_BYTES
        })
    );
}
