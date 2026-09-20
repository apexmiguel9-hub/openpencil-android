#![cfg(test)]

//! Relay-policy tests for the claim-minimized relay token and the bounded
//! dual-accept migration window.

use op_auth_bridge::{OpaqueCollabRelayToken, TestRelayTokenSpec};

use super::*;
use crate::ProductionRelayAuthConfig;

fn issue_relay_token(
    issuer: &TestCollabIssuer,
    caller_dh: [u8; 32],
    now: u64,
) -> OpaqueCollabRelayToken {
    issuer
        .issue_relay_token(&TestRelayTokenSpec::valid_at(now, caller_dh))
        .expect("signed minimized relay token")
}

fn relay_token_credential(token: &OpaqueCollabRelayToken) -> RelayBearerCredential {
    RelayBearerCredential::new(token.expose().to_vec())
}

#[test]
fn minimized_relay_token_authenticates_and_clamps_the_session_deadline() {
    let now = unix_now();
    let issuer = TestCollabIssuer::initial();
    let authenticator = full_authenticator(&issuer);
    let caller = client_public(CLIENT_A_SECRET);
    let token = issue_relay_token(&issuer, caller, now);
    let credential = relay_token_credential(&token);
    let route = route(RelayRegion::Cn, 0x55, 0x43, now);
    let (challenge_state, public_challenge) = challenge(&authenticator);
    let hello = v2_hello(
        RelayRole::Owner,
        CLIENT_A_SECRET,
        token.expose(),
        &public_challenge,
        &route,
    );

    let authenticated = authenticator
        .authenticate(&hello, Some(&credential), Some(challenge_state))
        .expect("the minimized relay token authenticates through relay policy");

    assert_eq!(authenticated.role(), RelayRole::Owner);
    // The relay's session deadline is the signed relay-token expiry clamped by
    // the hello, which is the only fact the bearer supplies.
    assert!(authenticated.expires_at_unix().get() <= now + 15 * 60);
    assert!(authenticated.expires_at_unix().get() > now);
}

#[test]
fn minimized_relay_token_is_far_smaller_than_the_identity_bearing_ticket() {
    let now = unix_now();
    let issuer = TestCollabIssuer::initial();
    let caller = client_public(CLIENT_A_SECRET);
    let token = issue_relay_token(&issuer, caller, now);
    let ticket = issue_ticket(
        &issuer,
        caller,
        op_auth_bridge::TEST_SUBJECT,
        TICKET_ID_A,
        now,
    );

    assert!(token.expose().len() < ticket.expose().len());
    assert!(token.expose().len() <= op_auth_bridge::MAX_COLLAB_RELAY_TOKEN_BYTES);
    // Dual-accept means the wire ceiling must still admit a full ticket.
    assert!(ticket.expose().len() <= op_collab_relay_protocol::MAX_RELAY_BEARER_BYTES);
}

#[test]
fn dual_accept_keeps_pre_migration_ticket_bearers_working() {
    let now = unix_now();
    let issuer = TestCollabIssuer::initial();
    let authenticator = full_authenticator(&issuer);
    let caller = client_public(CLIENT_A_SECRET);
    let ticket = issue_ticket(
        &issuer,
        caller,
        op_auth_bridge::TEST_SUBJECT,
        TICKET_ID_A,
        now,
    );
    let credential = credential(&ticket);
    let route = route(RelayRegion::Cn, 0x55, 0x43, now);
    let (challenge_state, public_challenge) = challenge(&authenticator);

    authenticator
        .authenticate(
            &v2_hello(
                RelayRole::Owner,
                CLIENT_A_SECRET,
                ticket.expose(),
                &public_challenge,
                &route,
            ),
            Some(&credential),
            Some(challenge_state),
        )
        .expect("dual-accept is the migration default");
}

#[test]
fn narrowed_relay_refuses_the_legacy_ticket_but_keeps_the_relay_token() {
    let now = unix_now();
    let issuer = TestCollabIssuer::initial();
    let authenticator = full_authenticator(&issuer).with_full_collab_ticket_accepted(false);
    let caller = client_public(CLIENT_A_SECRET);
    let route = route(RelayRegion::Cn, 0x55, 0x43, now);

    let ticket = issue_ticket(
        &issuer,
        caller,
        op_auth_bridge::TEST_SUBJECT,
        TICKET_ID_A,
        now,
    );
    let (ticket_state, ticket_challenge) = challenge(&authenticator);
    assert!(matches!(
        authenticator.authenticate(
            &v2_hello(
                RelayRole::Owner,
                CLIENT_A_SECRET,
                ticket.expose(),
                &ticket_challenge,
                &route,
            ),
            Some(&credential(&ticket)),
            Some(ticket_state),
        ),
        Err(RelayRejectCode::AuthenticationFailed)
    ));

    let token = issue_relay_token(&issuer, caller, now);
    let (token_state, token_challenge) = challenge(&authenticator);
    authenticator
        .authenticate(
            &v2_hello(
                RelayRole::Owner,
                CLIENT_A_SECRET,
                token.expose(),
                &token_challenge,
                &route,
            ),
            Some(&relay_token_credential(&token)),
            Some(token_state),
        )
        .expect("the minimized relay token is accepted on a narrowed relay");
}

#[test]
fn relay_token_bound_to_another_device_key_is_refused() {
    let now = unix_now();
    let issuer = TestCollabIssuer::initial();
    let authenticator = full_authenticator(&issuer);
    // Token bound to client B, presented on a hello from client A.
    let token = issue_relay_token(&issuer, client_public(CLIENT_B_SECRET), now);
    let route = route(RelayRegion::Cn, 0x55, 0x43, now);
    let (challenge_state, public_challenge) = challenge(&authenticator);

    assert!(matches!(
        authenticator.authenticate(
            &v2_hello(
                RelayRole::Owner,
                CLIENT_A_SECRET,
                token.expose(),
                &public_challenge,
                &route,
            ),
            Some(&relay_token_credential(&token)),
            Some(challenge_state),
        ),
        Err(RelayRejectCode::AuthenticationFailed)
    ));
}

/// An absolute path on every host: the config constructor rejects relative
/// paths, and a bare `/name` is not absolute on Windows (no drive prefix).
fn absolute_test_path(name: &str) -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(format!("C:\\{name}"))
    } else {
        PathBuf::from(format!("/{name}"))
    }
}

#[test]
fn legacy_ticket_bearer_policy_is_parsed_strictly_from_configuration() {
    let base = ProductionRelayAuthConfig::new(
        RelayRegion::Cn,
        absolute_test_path("policy.json"),
        absolute_test_path("locator.json"),
        Some(absolute_test_path("x25519.json")),
        NonZeroU64::new(60).expect("policy max age"),
        false,
    )
    .expect("production auth config");

    assert!(
        base.accepts_legacy_ticket_bearer(),
        "migration default must keep old clients working"
    );
    assert!(!base
        .with_legacy_ticket_bearer(false)
        .accepts_legacy_ticket_bearer());
}
