//! Relay-bearer verification tests, including the bidirectional
//! non-confusability property that makes one shared signing key defensible.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use ed25519_dalek::{Signer as _, SigningKey};
use serde_json::{json, Value};

use super::*;
use crate::{
    CollabJwksCacheLimits, StaticTestJwksFetcher, TestCollabIssuer, TestCollabTicketSpec,
    TestRelayTokenSpec, COLLAB_JWS_ALGORITHM, COLLAB_TICKET_AUDIENCE, COLLAB_TICKET_SCOPE,
    COLLAB_TICKET_VERSION, RELAY_TOKEN_AUDIENCE, RELAY_TOKEN_SCOPE, RELAY_TOKEN_VERSION,
    TEST_COLLAB_ISSUER, TEST_DEVICE_ID, TEST_SUBJECT, TEST_TICKET_ID,
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

fn relay_token_claims() -> Value {
    json!({
        "iss": TEST_COLLAB_ISSUER,
        "aud": RELAY_TOKEN_AUDIENCE,
        "ver": RELAY_TOKEN_VERSION,
        "dh_pub_x25519": URL_SAFE_NO_PAD.encode(CHANNEL_BINDING),
        "scope": RELAY_TOKEN_SCOPE,
        "iat": NOW,
        "nbf": NOW,
        "exp": NOW + 900,
    })
}

fn relay_token_header() -> Value {
    json!({
        "alg": COLLAB_JWS_ALGORITHM,
        "typ": RELAY_TOKEN_JWS_TYPE,
        "kid": "test_key_A",
    })
}

fn sign(header: &Value, claims: &Value) -> Vec<u8> {
    let header = URL_SAFE_NO_PAD.encode(serde_json::to_vec(header).unwrap());
    let claims = URL_SAFE_NO_PAD.encode(serde_json::to_vec(claims).unwrap());
    let signing_input = format!("{header}.{claims}");
    let signature = SigningKey::from_bytes(&TEST_KEY_A_SEED)
        .sign(signing_input.as_bytes())
        .to_bytes();
    format!("{signing_input}.{}", URL_SAFE_NO_PAD.encode(signature)).into_bytes()
}

#[test]
fn verifies_a_minimized_relay_token_and_exposes_only_the_expiry() {
    let issuer = TestCollabIssuer::initial();
    let token = issuer
        .issue_relay_token(&TestRelayTokenSpec::valid_at(NOW, CHANNEL_BINDING))
        .unwrap();
    let verified = test_verifier()
        .verify_relay_token_at(token.expose(), &CHANNEL_BINDING, NOW, Instant::now())
        .unwrap();

    assert_eq!(verified.expires_at_unix_seconds(), NOW + 900);
    assert_eq!(verified.expires_at_unix_ms(), (NOW + 900) * 1_000);

    // The credential must carry no account subject, device, ticket id, or
    // profile at all — not merely redact them in Debug output.
    let payload = String::from_utf8(token.expose().to_vec()).unwrap();
    let claims_segment = payload.split('.').nth(1).unwrap();
    let claims = String::from_utf8(URL_SAFE_NO_PAD.decode(claims_segment).unwrap()).unwrap();
    for forbidden in ["sub", "device_id", "jti", "display_name", "avatar_url"] {
        assert!(
            !claims.contains(forbidden),
            "minimized relay token must not carry `{forbidden}`: {claims}"
        );
    }
    assert!(!format!("{verified:?}").contains(TEST_SUBJECT));
}

#[test]
fn relay_token_parser_rejects_a_full_collaboration_ticket() {
    // Direction 1 of the non-confusability property: the identity-bearing
    // ticket must be invalid against the minimized parser.
    let issuer = TestCollabIssuer::initial();
    let ticket = issuer
        .issue(&TestCollabTicketSpec::valid_at(NOW, CHANNEL_BINDING))
        .unwrap();
    let verifier = test_verifier();

    // The protected-header `typ` alone already refuses it.
    assert_eq!(
        verifier.verify_relay_token_at(ticket.expose(), &CHANNEL_BINDING, NOW, Instant::now()),
        Err(CollabVerifyError::WrongType)
    );

    // And so does the claim shape, independently: even relabelled with the
    // relay `typ` and re-signed, `deny_unknown_fields` rejects `sub`,
    // `device_id`, `jti`, and the profile claims.
    let mut relabelled = json!({
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
    });
    assert_eq!(
        verifier.verify_relay_token_at(
            &sign(&relay_token_header(), &relabelled),
            &CHANNEL_BINDING,
            NOW,
            Instant::now()
        ),
        Err(CollabVerifyError::MalformedJson { part: "claims" })
    );

    // Stripping the identity claims still fails on the strict audience and
    // scope comparisons, so no single mistake is enough to confuse them.
    for key in ["sub", "device_id", "jti"] {
        relabelled.as_object_mut().unwrap().remove(key);
    }
    assert_eq!(
        verifier.verify_relay_token_at(
            &sign(&relay_token_header(), &relabelled),
            &CHANNEL_BINDING,
            NOW,
            Instant::now()
        ),
        Err(CollabVerifyError::InvalidAudience)
    );
}

#[test]
fn collaboration_ticket_parser_rejects_a_minimized_relay_token() {
    // Direction 2: the minimized token must be invalid against the ticket
    // parser, so it can never be replayed as a peer-admission credential.
    let issuer = TestCollabIssuer::initial();
    let token = issuer
        .issue_relay_token(&TestRelayTokenSpec::valid_at(NOW, CHANNEL_BINDING))
        .unwrap();
    let verifier = test_verifier();

    assert_eq!(
        verifier.verify_at(token.expose(), &CHANNEL_BINDING, NOW, Instant::now()),
        Err(CollabVerifyError::WrongType)
    );

    // Relabelled with the collaboration `typ` and re-signed, the claim shape
    // still fails: `sub`, `device_id`, and `jti` are missing.
    let mut relabelled_header = relay_token_header();
    relabelled_header["typ"] = json!(COLLAB_JWS_TYPE);
    assert_eq!(
        verifier.verify_at(
            &sign(&relabelled_header, &relay_token_claims()),
            &CHANNEL_BINDING,
            NOW,
            Instant::now()
        ),
        Err(CollabVerifyError::MalformedJson { part: "claims" })
    );
}

#[test]
fn relay_token_enforces_channel_binding_scope_and_lifetime() {
    let verifier = test_verifier();

    assert_eq!(
        verifier.verify_relay_token_at(
            &sign(&relay_token_header(), &relay_token_claims()),
            &OTHER_CHANNEL_BINDING,
            NOW,
            Instant::now()
        ),
        Err(CollabVerifyError::ChannelBindingMismatch)
    );

    let mut invalid = relay_token_claims();
    invalid["scope"] = json!(COLLAB_TICKET_SCOPE);
    assert_eq!(
        verifier.verify_relay_token_at(
            &sign(&relay_token_header(), &invalid),
            &CHANNEL_BINDING,
            NOW,
            Instant::now()
        ),
        Err(CollabVerifyError::InvalidScope)
    );

    invalid = relay_token_claims();
    invalid["ver"] = json!(RELAY_TOKEN_VERSION + 1);
    assert_eq!(
        verifier.verify_relay_token_at(
            &sign(&relay_token_header(), &invalid),
            &CHANNEL_BINDING,
            NOW,
            Instant::now()
        ),
        Err(CollabVerifyError::InvalidVersion)
    );

    invalid = relay_token_claims();
    invalid["exp"] = json!(NOW + 901);
    assert_eq!(
        verifier.verify_relay_token_at(
            &sign(&relay_token_header(), &invalid),
            &CHANNEL_BINDING,
            NOW,
            Instant::now()
        ),
        Err(CollabVerifyError::LifetimeTooLong)
    );

    invalid = relay_token_claims();
    invalid["nbf"] = json!(NOW + 61);
    assert_eq!(
        verifier.verify_relay_token_at(
            &sign(&relay_token_header(), &invalid),
            &CHANNEL_BINDING,
            NOW,
            Instant::now()
        ),
        Err(CollabVerifyError::NotYetValid)
    );

    invalid = relay_token_claims();
    invalid["iat"] = json!(NOW - 900);
    invalid["nbf"] = json!(NOW - 900);
    invalid["exp"] = json!(NOW - 61);
    assert_eq!(
        verifier.verify_relay_token_at(
            &sign(&relay_token_header(), &invalid),
            &CHANNEL_BINDING,
            NOW,
            Instant::now()
        ),
        Err(CollabVerifyError::Expired)
    );

    assert_eq!(
        verifier.verify_relay_token_at(
            &vec![b'a'; MAX_COLLAB_RELAY_TOKEN_BYTES + 1],
            &CHANNEL_BINDING,
            NOW,
            Instant::now()
        ),
        Err(CollabVerifyError::InvalidTicketSize {
            maximum: MAX_COLLAB_RELAY_TOKEN_BYTES
        })
    );
}

#[test]
fn relay_bearer_dual_accept_discriminates_on_type_and_never_returns_identity() {
    let issuer = TestCollabIssuer::initial();
    let token = issuer
        .issue_relay_token(&TestRelayTokenSpec::valid_at(NOW, CHANNEL_BINDING))
        .unwrap();
    let ticket = issuer
        .issue(&TestCollabTicketSpec::valid_at(NOW, CHANNEL_BINDING))
        .unwrap();
    let verifier = test_verifier();

    for accept_full in [true, false] {
        let accepted = verifier
            .verify_relay_bearer_at(
                token.expose(),
                &CHANNEL_BINDING,
                NOW,
                Instant::now(),
                accept_full,
            )
            .expect("the minimized relay token is always accepted");
        assert_eq!(accepted.kind(), RelayBearerKind::MinimizedRelayToken);
        assert_eq!(accepted.expires_at_unix_seconds(), NOW + 900);
    }

    let legacy = verifier
        .verify_relay_bearer_at(ticket.expose(), &CHANNEL_BINDING, NOW, Instant::now(), true)
        .expect("dual-accept keeps pre-migration clients working");
    assert_eq!(legacy.kind(), RelayBearerKind::FullCollabTicket);
    assert_eq!(legacy.expires_at_unix_seconds(), NOW + 900);
    // Even the legacy branch hands back nothing but the expiry.
    assert!(!format!("{legacy:?}").contains(TEST_SUBJECT));

    assert_eq!(
        verifier.verify_relay_bearer_at(
            ticket.expose(),
            &CHANNEL_BINDING,
            NOW,
            Instant::now(),
            false
        ),
        Err(RelayBearerVerifyError::UnacceptedCredentialType)
    );
}

#[test]
fn relay_bearer_refuses_an_unknown_credential_type_before_claim_parsing() {
    let mut foreign = relay_token_header();
    foreign["typ"] = json!("JWT");
    assert_eq!(
        test_verifier().verify_relay_bearer_at(
            &sign(&foreign, &relay_token_claims()),
            &CHANNEL_BINDING,
            NOW,
            Instant::now(),
            true
        ),
        Err(RelayBearerVerifyError::UnacceptedCredentialType)
    );
}
