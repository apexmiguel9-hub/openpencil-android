use std::time::Instant;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use ed25519_dalek::{Signer as _, SigningKey};
use op_auth_bridge::{
    CollabJwksCacheLimits, CollabJwksError, CollabJwksFetchError, CollabJwksFetchRequest,
    CollabJwksFetchResponse, CollabJwksFetcher, CollabTicketVerifier, CollabUnionPolicyError,
    CollabVerifierConfig, CollabVerifyError,
};
use serde::Deserialize;
use serde_json::json;

const ISSUER: &str = "https://collab.example.com";
const ENDPOINT: &str = "https://cn.example/api/v1/collab/policy";
const NOW: u64 = 1_800_000_000;
const BINDING: [u8; 32] = [0x42; 32];
const GO_V2_FIXTURE: &str = include_str!("fixtures/zseven-sso-go-union-policy-v2.json");

#[derive(Deserialize)]
struct GoUnionPolicyFixture {
    policy_json: String,
}

fn sequence_key(first_byte: u8) -> SigningKey {
    let mut seed = [0_u8; 32];
    for (index, byte) in seed.iter_mut().enumerate() {
        *byte = first_byte + u8::try_from(index).unwrap();
    }
    SigningKey::from_bytes(&seed)
}

fn policy_key(region: &str, kid: &str, first_byte: u8, activated: i64) -> serde_json::Value {
    json!({
        "region": region,
        "kid": kid,
        "x": URL_SAFE_NO_PAD.encode(sequence_key(first_byte).verifying_key().to_bytes()),
        "published_at_unix": 1_700_000_000,
        "activated_at_unix": activated,
        "retired_at_unix": 0,
        "not_after_unix": 0,
    })
}

fn legacy_policy_body() -> Vec<u8> {
    serde_json::to_vec(&json!({
        "version": 1,
        "generation": 7,
        "issuer": ISSUER,
        "not_before_unix": 1_799_900_000,
        "not_after_unix": 1_800_500_000,
        "required_regions": ["cn", "global"],
        "keys": [
            policy_key("cn", "active_key", 1, 1_700_000_300),
            policy_key("cn", "next_key", 41, 0),
            policy_key("global", "remote_active_key", 81, 1_700_000_300),
            policy_key("global", "remote_next_key", 121, 0),
        ],
        "signature": "",
    }))
    .unwrap()
}

fn policy_body() -> Vec<u8> {
    serde_json::from_str::<GoUnionPolicyFixture>(GO_V2_FIXTURE)
        .unwrap()
        .policy_json
        .into_bytes()
}

struct FrozenPolicy(Vec<u8>);

impl CollabJwksFetcher for FrozenPolicy {
    fn fetch(
        &self,
        request: CollabJwksFetchRequest<'_>,
    ) -> Result<CollabJwksFetchResponse, CollabJwksFetchError> {
        assert_eq!(request.endpoint, ENDPOINT);
        let body = self.0.clone();
        assert!(body.len() <= request.maximum_body_bytes);
        Ok(CollabJwksFetchResponse::Modified {
            body,
            etag: Some("\"signed-union-v7\"".to_owned()),
            max_age_seconds: 300,
        })
    }
}

fn ticket(kid: &str, signing_key: &SigningKey) -> Vec<u8> {
    let header = URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&json!({
            "alg": "Ed25519",
            "typ": "openpencil-collab+jwt",
            "kid": kid,
        }))
        .unwrap(),
    );
    let claims = URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&json!({
            "iss": ISSUER,
            "aud": "openpencil-collab",
            "ver": 1,
            "sub": "123e4567-e89b-12d3-a456-426614174000",
            "device_id": "123e4567-e89b-12d3-a456-426614174001",
            "dh_pub_x25519": URL_SAFE_NO_PAD.encode(BINDING),
            "scope": "collab:connect",
            "iat": NOW,
            "nbf": NOW,
            "exp": NOW + 900,
            "jti": "dGVzdC10aWNrZXQtaWQtMDAwMQ",
        }))
        .unwrap(),
    );
    let signing_input = format!("{header}.{claims}");
    let signature = URL_SAFE_NO_PAD.encode(signing_key.sign(signing_input.as_bytes()).to_bytes());
    format!("{signing_input}.{signature}").into_bytes()
}

#[test]
fn production_root_signed_v2_policy_verifies_active_ticket_and_blocks_next_key() {
    let verifier = CollabTicketVerifier::new(
        CollabVerifierConfig::new_signed_policy(ISSUER, ENDPOINT).unwrap(),
        FrozenPolicy(policy_body()),
        CollabJwksCacheLimits::default(),
    )
    .unwrap();
    let now = Instant::now();

    let verified = verifier
        .verify_at(&ticket("cn-active", &sequence_key(1)), &BINDING, NOW, now)
        .unwrap();
    assert_eq!(verified.issuer(), ISSUER);
    assert_eq!(
        verifier.verify_at(&ticket("cn-next", &sequence_key(2)), &BINDING, NOW, now),
        Err(CollabVerifyError::Jwks(CollabJwksError::UnknownKey))
    );
}

#[test]
fn legacy_v1_signed_union_policy_fails_closed() {
    let verifier = CollabTicketVerifier::new(
        CollabVerifierConfig::new_signed_policy(ISSUER, ENDPOINT).unwrap(),
        FrozenPolicy(legacy_policy_body()),
        CollabJwksCacheLimits::default(),
    )
    .unwrap();
    assert_eq!(
        verifier.verify_at(
            &ticket("active_key", &sequence_key(1)),
            &BINDING,
            NOW,
            Instant::now(),
        ),
        Err(CollabVerifyError::Jwks(CollabJwksError::Policy(
            CollabUnionPolicyError::MalformedJson
        )))
    );
}
