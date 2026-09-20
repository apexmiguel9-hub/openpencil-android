#![cfg(test)]

use op_auth_bridge::{
    MAX_COLLAB_PROFILE_AVATAR_URL_BYTES, MAX_COLLAB_PROFILE_DISPLAY_NAME_BYTES,
    MAX_COLLAB_TICKET_BYTES,
};
use op_collab_relay_protocol::MAX_RELAY_BEARER_BYTES;

use super::*;

#[test]
fn maximum_valid_profile_ticket_crosses_the_legacy_header_limit_and_authenticates() {
    const LEGACY_RELAY_BEARER_BYTES: usize = 4_089;

    assert_eq!(MAX_RELAY_BEARER_BYTES, MAX_COLLAB_TICKET_BYTES);
    let now = unix_now();
    let caller = client_public(CLIENT_A_SECRET);
    let key_id = "k".repeat(128);
    let signing_key = SigningKey::from_bytes(&[0x71; 32]);
    let display_name = "🦀".repeat(MAX_COLLAB_PROFILE_DISPLAY_NAME_BYTES / 4);
    let avatar_prefix = "https://cdn.example/";
    let avatar_url = format!(
        "{avatar_prefix}{}",
        "a".repeat(MAX_COLLAB_PROFILE_AVATAR_URL_BYTES - avatar_prefix.len())
    );
    let header = URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&serde_json::json!({
            "alg": "Ed25519",
            "typ": "openpencil-collab+jwt",
            "kid": key_id,
        }))
        .unwrap(),
    );
    let claims = URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&serde_json::json!({
            "iss": op_auth_bridge::TEST_COLLAB_ISSUER,
            "aud": "openpencil-collab",
            "ver": 1,
            "sub": op_auth_bridge::TEST_SUBJECT,
            "device_id": op_auth_bridge::TEST_DEVICE_ID,
            "dh_pub_x25519": URL_SAFE_NO_PAD.encode(caller),
            "scope": "collab:connect",
            "iat": now,
            "nbf": now,
            "exp": now + 15 * 60,
            "jti": URL_SAFE_NO_PAD.encode([0xa5; 96]),
            "display_name": display_name,
            "avatar_url": avatar_url,
        }))
        .unwrap(),
    );
    let signing_input = format!("{header}.{claims}");
    let signature = signing_key.sign(signing_input.as_bytes()).to_bytes();
    let ticket = OpaqueCollabTicket::new(
        format!("{signing_input}.{}", URL_SAFE_NO_PAD.encode(signature)).into_bytes(),
    )
    .expect("maximum valid profile ticket");
    assert!(
        ticket.expose().len() > LEGACY_RELAY_BEARER_BYTES,
        "regression fixture must cross the retired 4089-byte relay ceiling"
    );
    assert!(ticket.expose().len() <= MAX_RELAY_BEARER_BYTES);

    let jwks = serde_json::to_vec(&serde_json::json!({
        "keys": [{
            "kty": "OKP",
            "crv": "Ed25519",
            "alg": "Ed25519",
            "use": "sig",
            "key_ops": ["verify"],
            "kid": key_id,
            "x": URL_SAFE_NO_PAD.encode(signing_key.verifying_key().to_bytes()),
        }]
    }))
    .unwrap();
    let verifier = CollabTicketVerifier::new(
        TestCollabIssuer::verifier_config().unwrap(),
        StaticTestJwksFetcher::new(jwks, 300),
        CollabJwksCacheLimits::default(),
    )
    .unwrap();
    let authenticator = CollabTicketRelayAuthenticator::new(
        verifier,
        RelayRegion::Cn,
        StrictTestLocatorVerifier,
        TestX25519Boundary::new(),
    );
    let route = route(RelayRegion::Cn, 0x55, 0x43, now);
    let (challenge_state, public_challenge) = challenge(&authenticator);
    let hello = v2_hello(
        RelayRole::Guest,
        CLIENT_A_SECRET,
        ticket.expose(),
        &public_challenge,
        &route,
    );
    authenticator
        .authenticate(&hello, Some(&credential(&ticket)), Some(challenge_state))
        .expect("maximum valid profile ticket authenticates through relay policy");
}
