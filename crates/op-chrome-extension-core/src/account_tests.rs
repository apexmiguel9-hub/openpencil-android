//! Tests for [`crate::account`] — regions, hub URLs, session parsing.

use crate::account::{
    account_url, host_permission, hub_origin, login_url, logout_url, parse_session,
    session_to_json, session_url, Region, SessionView, HUB_ORIGIN_CN, HUB_ORIGIN_GLOBAL,
};

/// A session envelope in exactly the shape `op-hub`'s session handler writes.
fn envelope(display_name: &str, avatar: &str) -> String {
    format!(
        r#"{{"user":{{"id":"123e4567-e89b-12d3-a456-426614174000",
        "username":"person_name","display_name":{display_name},
        "avatar_url":{avatar},"primary_email":"person@example.com",
        "roles":["user"]}},
        "csrf_token":"UJ2gk1rMxq7Yy4Yy0i9Qh2vJ8Kx3Lp5Rn7Tt1Ww9Zz0",
        "capabilities":{{"admin":false}}}}"#
    )
}

fn signed_in(view: SessionView) -> crate::account::Account {
    match view {
        SessionView::SignedIn(account) => account,
        other => panic!("expected a signed-in session, got {other:?}"),
    }
}

/* ------------------------------------------------------------- the regions */

#[test]
fn regions_round_trip_their_persisted_spelling() {
    assert_eq!(Region::Cn.as_str(), "cn");
    assert_eq!(Region::Global.as_str(), "global");
    assert_eq!(Region::from_stored("cn"), Region::Cn);
    assert_eq!(Region::from_stored("global"), Region::Global);
    assert_eq!(Region::from_stored(" global "), Region::Global);
}

#[test]
fn an_unknown_or_absent_region_is_china_the_product_default() {
    // Matches `CollabRelayRegion::default()`, so the two surfaces agree.
    assert_eq!(Region::default(), Region::Cn);
    for raw in ["", "  ", "CN", "Global", "eu", "null", "undefined"] {
        assert_eq!(Region::from_stored(raw), Region::Cn, "for {raw:?}");
    }
}

#[test]
fn each_region_maps_onto_its_published_hub() {
    assert_eq!(Region::Cn.hub_origin(), "https://op.zseven.cn");
    assert_eq!(Region::Global.hub_origin(), "https://op.zseven.tech");
}

/* --------------------------------------------------------- the hub origin */

#[test]
fn without_an_override_the_region_decides() {
    assert_eq!(hub_origin(Region::Cn, ""), HUB_ORIGIN_CN);
    assert_eq!(hub_origin(Region::Global, "   "), HUB_ORIGIN_GLOBAL);
}

#[test]
fn a_loopback_override_wins_because_the_manifest_already_grants_it() {
    // The Hub's own documented development invocation.
    assert_eq!(
        hub_origin(Region::Cn, "http://127.0.0.1:18081"),
        "http://127.0.0.1:18081"
    );
    // Same normalization the editor endpoint gets: scheme optional, trailing
    // slashes dropped, `localhost` rewritten to the numeric literal.
    assert_eq!(
        hub_origin(Region::Global, "localhost:18081/"),
        "http://127.0.0.1:18081"
    );
}

#[test]
fn a_non_loopback_override_is_ignored_not_obeyed() {
    // This is the whole point of routing the override through the endpoint
    // whitelist: no stored value can aim the session probe — which carries
    // the user's Hub cookie — at a host of someone else's choosing.
    for raw in [
        "https://evil.test",
        "http://evil.test:80",
        "evil.test:18081",
        "https://op.zseven.cn.evil.test",
        "http://192.168.1.5:18081",
        "https://127.0.0.1:8443",
        "//127.0.0.1:18081",
        "127.0.0.1",
        "http://127.0.0.1:18081/api/v1/session",
    ] {
        assert_eq!(hub_origin(Region::Cn, raw), HUB_ORIGIN_CN, "for {raw:?}");
    }
}

/* ----------------------------------------------------------- the hub URLs */

#[test]
fn the_hub_urls_match_the_routes_op_hub_registers() {
    let origin = HUB_ORIGIN_CN;
    assert_eq!(
        login_url(origin),
        "https://op.zseven.cn/api/v1/auth/login?return_to=/account"
    );
    assert_eq!(session_url(origin), "https://op.zseven.cn/api/v1/session");
    assert_eq!(
        logout_url(origin),
        "https://op.zseven.cn/api/v1/auth/logout"
    );
    assert_eq!(account_url(origin), "https://op.zseven.cn/account");
}

#[test]
fn the_login_url_carries_exactly_one_allowlisted_return_to() {
    // `op-hub`'s `exactQuery` rejects a login request whose query is not
    // exactly `return_to=<allowlisted path>`: no extra parameters, no
    // duplicates, no encoding, no fragment.
    let url = login_url(HUB_ORIGIN_GLOBAL);
    let (_, query) = url.split_once('?').expect("the login URL carries a query");
    assert_eq!(query, "return_to=/account");
    assert!(!url.contains('#'));
    assert_eq!(url.matches('?').count(), 1);
    assert!(!query.contains('%'));
}

#[test]
fn host_permission_patterns_cover_the_whole_origin() {
    assert_eq!(host_permission(HUB_ORIGIN_CN), "https://op.zseven.cn/*");
    assert_eq!(
        host_permission(HUB_ORIGIN_GLOBAL),
        "https://op.zseven.tech/*"
    );
}

/// The reachable host set is a manifest fact, and the manifest is a separate
/// file that no compiler checks. Chrome resolves `host_permissions` at
/// install time, so an origin added here and forgotten there produces a
/// popup that fails every probe with an opaque network error.
#[test]
fn the_manifest_grants_exactly_the_two_hub_origins() {
    const MANIFEST: &str = include_str!("../../../packages/op-chrome-extension/manifest.json");
    let parsed: serde_json::Value =
        serde_json::from_str(MANIFEST).expect("manifest.json is valid JSON");
    let granted: Vec<&str> = parsed["host_permissions"]
        .as_array()
        .expect("manifest.json declares host_permissions")
        .iter()
        .filter_map(|value| value.as_str())
        .collect();

    for origin in [HUB_ORIGIN_CN, HUB_ORIGIN_GLOBAL] {
        assert!(
            granted.contains(&host_permission(origin).as_str()),
            "manifest.json does not grant {origin}; it has {granted:?}"
        );
    }
    // The loopback grant the local import path depends on, and nothing wider.
    assert!(granted.contains(&"http://127.0.0.1/*"));
    assert_eq!(
        granted.len(),
        3,
        "unexpected host permission in {granted:?}"
    );
}

/* ------------------------------------------------------ the session reply */

#[test]
fn a_401_is_signed_out_and_not_an_error() {
    assert_eq!(
        parse_session(401, r#"{"error":{"code":"authentication_required"}}"#),
        SessionView::SignedOut
    );
    // Even an empty body: the status is the signal.
    assert_eq!(parse_session(401, ""), SessionView::SignedOut);
}

#[test]
fn a_full_envelope_yields_the_renderable_fields() {
    let account = signed_in(parse_session(
        200,
        &envelope("\"Person\"", "\"https://cdn.example.com/a.png\""),
    ));
    assert_eq!(account.user_id, "123e4567-e89b-12d3-a456-426614174000");
    assert_eq!(account.display_name, "Person");
    assert_eq!(
        account.avatar_url.as_deref(),
        Some("https://cdn.example.com/a.png")
    );
    assert_eq!(
        account.csrf_token,
        "UJ2gk1rMxq7Yy4Yy0i9Qh2vJ8Kx3Lp5Rn7Tt1Ww9Zz0"
    );
}

#[test]
fn a_null_or_blank_display_name_falls_back_to_the_username() {
    for name in ["null", "\"\"", "\"   \""] {
        let account = signed_in(parse_session(200, &envelope(name, "null")));
        assert_eq!(account.display_name, "person_name", "for {name}");
    }
}

#[test]
fn a_long_display_name_cannot_push_the_sign_out_control_off_screen() {
    let long = format!("\"{}\"", "ま".repeat(200));
    let account = signed_in(parse_session(200, &envelope(&long, "null")));
    assert_eq!(account.display_name.chars().count(), 48);
}

#[test]
fn only_an_https_avatar_reaches_the_img_tag() {
    for avatar in [
        "null",
        "\"\"",
        "\"javascript:alert(1)\"",
        "\"data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=\"",
        "\"http://cdn.example.com/a.png\"",
        "\"file:///etc/passwd\"",
        "\"/avatars/me.png\"",
        "\"//cdn.example.com/a.png\"",
        "\"https://cdn.example.com/a.png\\\" onerror=\\\"alert(1)\"",
        "\"https://cdn.example.com/a.png onerror=alert(1)\"",
        "\"https://cdn.example.com/<script>\"",
    ] {
        let account = signed_in(parse_session(200, &envelope("\"Person\"", avatar)));
        assert_eq!(account.avatar_url, None, "for {avatar}");
    }
}

#[test]
fn a_csrf_token_that_could_forge_a_header_is_refused_outright() {
    // A token the popup would otherwise splice into `X-CSRF-Token`. The
    // whole session is rejected rather than the field dropped: without a
    // usable token there is no sign-out, and pretending otherwise would put
    // a dead control on screen.
    for token in [
        "\"a\\r\\nX-Injected: 1\"",
        "\"a b\"",
        "\"a\\tb\"",
        "\"\"",
        "null",
        "\"токен\"",
        "123",
    ] {
        let body = format!(
            r#"{{"user":{{"id":"u","username":"n","display_name":null,
            "avatar_url":null,"primary_email":null,"roles":["user"]}},
            "csrf_token":{token},"capabilities":{{"admin":false}}}}"#
        );
        assert!(
            matches!(parse_session(200, &body), SessionView::Unavailable { .. }),
            "accepted {token}"
        );
    }
}

#[test]
fn a_user_without_an_id_or_username_is_not_a_session() {
    for user in [
        r#"{"id":"","username":"n"}"#,
        r#"{"id":"u","username":""}"#,
        r#"{"id":"u"}"#,
        r#"{"username":"n"}"#,
        r#"{"id":123,"username":"n"}"#,
        "null",
    ] {
        let body = format!(
            r#"{{"user":{user},"csrf_token":"UJ2gk1rMxq7Yy4Yy0i9Qh2vJ8Kx3Lp5Rn7Tt1Ww9Zz0"}}"#
        );
        assert!(
            matches!(parse_session(200, &body), SessionView::Unavailable { .. }),
            "accepted {user}"
        );
    }
}

#[test]
fn a_non_401_failure_is_unavailable_never_signed_out() {
    // Reporting a 503 as "signed out" would show a sign-in button to a user
    // who is signed in, and one click later they would be looking at the
    // Hub's login page for no reason.
    let view = parse_session(
        503,
        r#"{"error":{"code":"service_unavailable","message":"Service unavailable"}}"#,
    );
    assert_eq!(
        view,
        SessionView::Unavailable {
            detail: "Service unavailable".to_owned()
        }
    );
}

#[test]
fn a_body_that_is_not_the_hubs_at_all_is_unavailable_with_the_status() {
    for (status, body) in [
        (200u16, "<html>captive portal</html>"),
        (200, ""),
        (502, "<html>bad gateway</html>"),
        (200, "{}"),
    ] {
        assert!(
            matches!(parse_session(status, body), SessionView::Unavailable { .. }),
            "accepted {status} {body:?}"
        );
    }
    assert_eq!(
        parse_session(502, ""),
        SessionView::Unavailable {
            detail: "HTTP 502".to_owned()
        }
    );
}

#[test]
fn a_server_chosen_error_message_is_capped_before_it_reaches_the_popup() {
    let body = format!(
        r#"{{"error":{{"code":"x","message":"{}"}}}}"#,
        "e".repeat(9999)
    );
    let SessionView::Unavailable { detail } = parse_session(500, &body) else {
        panic!("expected Unavailable");
    };
    assert_eq!(detail.chars().count(), 200);
}

/* ---------------------------------------------------------- the JSON shim */

#[test]
fn the_json_shim_carries_no_field_the_popup_does_not_render() {
    let json = session_to_json(&parse_session(
        200,
        &envelope("\"Person\"", "\"https://cdn.example.com/a.png\""),
    ));
    let value: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert_eq!(value["state"], "signedIn");
    assert_eq!(value["displayName"], "Person");
    assert_eq!(value["avatarUrl"], "https://cdn.example.com/a.png");
    // The envelope's email and roles must not survive the crossing: the
    // popup never shows them, and what is not handed over cannot leak into a
    // stored account row or a screenshot.
    assert!(!json.contains("person@example.com"));
    assert!(!json.contains("roles"));
    assert!(!json.contains("primary_email"));
    assert!(!json.contains("capabilities"));
}

#[test]
fn the_two_absent_states_serialize_as_the_popup_expects() {
    assert_eq!(
        session_to_json(&SessionView::SignedOut),
        r#"{"state":"signedOut"}"#
    );
    // Compare parsed fields rather than the raw string: serde_json's object
    // key order depends on whether `preserve_order` is unified into the build
    // graph (op-html pulls it in transitively via schemars), and the popup
    // `JSON.parse`s this either way, so ordering is not part of the contract.
    let json = session_to_json(&SessionView::Unavailable {
        detail: "HTTP 503".to_owned(),
    });
    let value: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert_eq!(value["state"], "error");
    assert_eq!(value["detail"], "HTTP 503");
}

#[test]
fn an_absent_avatar_crosses_as_null_rather_than_an_empty_string() {
    let json = session_to_json(&parse_session(200, &envelope("\"Person\"", "null")));
    let value: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert!(value["avatarUrl"].is_null());
}
