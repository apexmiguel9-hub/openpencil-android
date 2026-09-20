//! Tests for the hub identity client, against a mock hub on a real socket.
//!
//! A real `TcpListener` rather than a trait double on purpose: the things
//! worth testing here are the HTTP-level decisions — which status means
//! "definitively no", what header the introspection call must carry, and
//! whether a second lookup hits the wire at all — and a double would let
//! those drift without a test noticing.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;

use super::*;

/// One canned reply.
struct Reply {
    status: &'static str,
    body: &'static str,
}

impl Reply {
    const fn ok(body: &'static str) -> Self {
        Self {
            status: "200 OK",
            body,
        }
    }

    const fn status(status: &'static str) -> Self {
        Self { status, body: "{}" }
    }
}

/// A mock hub that serves `replies` in order, then stops.
///
/// Returns its base URL and a receiver of the raw request heads it saw, so a
/// test can assert on the headers the client sent.
fn mock_hub(replies: Vec<Reply>) -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind mock hub");
    let port = listener.local_addr().expect("addr").port();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for reply in replies {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut seen = Vec::new();
            let mut byte = [0u8; 1];
            // Read the head, then whatever body the declared length promises.
            while stream.read(&mut byte).map(|n| n == 1).unwrap_or(false) {
                seen.push(byte[0]);
                if seen.ends_with(b"\r\n\r\n") {
                    break;
                }
            }
            let head = String::from_utf8_lossy(&seen).into_owned();
            let length = head
                .lines()
                .find_map(|line| {
                    let (name, value) = line.trim().split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            let mut body = vec![0u8; length];
            if length > 0 {
                let _ = stream.read_exact(&mut body);
            }
            let _ = tx.send(format!("{head}{}", String::from_utf8_lossy(&body)));
            let response = format!(
                "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n{}",
                reply.status,
                reply.body.len(),
                reply.body
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });
    (format!("http://127.0.0.1:{port}"), rx)
}

const SESSION_BODY: &str = r#"{
  "user": {
    "id": "user-uuid-1",
    "username": "person_name",
    "display_name": "Person",
    "avatar_url": "https://cdn.example.com/a.png",
    "primary_email": "person@example.com",
    "roles": ["user"]
  },
  "csrf_token": "csrf",
  "capabilities": { "admin": false }
}"#;

const TOKEN_BODY: &str = r#"{
  "active": true,
  "user_id": "user-uuid-1",
  "username": "person_name",
  "scopes": ["mcp:read", "mcp:write"],
  "expires_at_unix": null
}"#;

fn client(base_url: &str) -> HubAuthClient {
    HubAuthClient::new(base_url, Some("shared-secret".into())).expect("client builds")
}

// ---------------------------------------------------------------------------
// verify_session
// ---------------------------------------------------------------------------

#[test]
fn a_session_cookie_resolves_to_its_account() {
    let (base, requests) = mock_hub(vec![Reply::ok(SESSION_BODY)]);
    let user = client(&base)
        .verify_session("sess-value")
        .expect("resolves");
    assert_eq!(user.id, "user-uuid-1");
    assert_eq!(user.username, "person_name");
    assert_eq!(user.display_name.as_deref(), Some("Person"));
    assert_eq!(user.roles, vec!["user".to_string()]);

    let seen = requests.recv().expect("the hub saw a request");
    assert!(seen.starts_with("GET /api/v1/session "), "{seen}");
    assert!(
        seen.contains("cookie: op_hub_session=sess-value")
            || seen.contains("Cookie: op_hub_session=sess-value"),
        "the session cookie must be forwarded verbatim: {seen}"
    );
}

#[test]
fn a_session_response_with_null_optionals_still_resolves() {
    let body = r#"{"user":{"id":"u2","username":"u2name","display_name":null,
        "avatar_url":null,"primary_email":null,"roles":[]},"csrf_token":"c",
        "capabilities":{"admin":false}}"#;
    let (base, _requests) = mock_hub(vec![Reply {
        status: "200 OK",
        body: Box::leak(body.to_string().into_boxed_str()),
    }]);
    let user = client(&base).verify_session("sess").expect("resolves");
    assert_eq!(user.display_name, None);
    assert_eq!(user.primary_email, None);
}

#[test]
fn a_signed_out_browser_is_a_definitive_negative() {
    for status in ["401 Unauthorized", "403 Forbidden", "404 Not Found"] {
        let (base, _requests) = mock_hub(vec![Reply::status(status)]);
        assert_eq!(
            client(&base).verify_session("sess").unwrap_err(),
            HubAuthError::Unauthenticated,
            "{status}"
        );
    }
}

#[test]
fn a_session_body_missing_its_identity_is_not_trusted() {
    let (base, _requests) = mock_hub(vec![Reply::ok(r#"{"user":{"id":"","username":""}}"#)]);
    assert_eq!(
        client(&base).verify_session("sess").unwrap_err(),
        HubAuthError::MalformedResponse
    );
}

#[test]
fn a_malformed_credential_never_reaches_the_hub() {
    // No mock hub at all: if this called out, it would fail with Upstream.
    let client = HubAuthClient::new("http://127.0.0.1:1", Some("s".into())).expect("builds");
    for bad in ["", "   ", "has space", "has;semicolon", "line\nbreak"] {
        assert_eq!(
            client.verify_session(bad).unwrap_err(),
            HubAuthError::InvalidCredential,
            "{bad:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// introspect_token
// ---------------------------------------------------------------------------

#[test]
fn a_bearer_token_introspects_to_its_account() {
    let (base, requests) = mock_hub(vec![Reply::ok(TOKEN_BODY)]);
    let token = client(&base).introspect_token("tok-1").expect("resolves");
    assert!(token.active);
    assert_eq!(token.user_id, "user-uuid-1");
    assert_eq!(
        token.scopes,
        vec!["mcp:read".to_string(), "mcp:write".into()]
    );

    let seen = requests.recv().expect("the hub saw a request");
    assert!(
        seen.starts_with("POST /api/v1/tokens/introspect "),
        "{seen}"
    );
    assert!(
        seen.to_lowercase()
            .contains("x-op-internal-auth: shared-secret"),
        "the internal shared secret must be sent: {seen}"
    );
    assert!(
        seen.contains(r#""token":"tok-1""#),
        "the token must be in the body, never the URL: {seen}"
    );
}

#[test]
fn introspection_without_a_configured_secret_fails_closed_without_calling() {
    let client = HubAuthClient::new("http://127.0.0.1:1", None).expect("builds");
    assert_eq!(
        client.introspect_token("tok-1").unwrap_err(),
        HubAuthError::MissingInternalAuth
    );
}

#[test]
fn an_inactive_token_is_a_definitive_negative() {
    let (base, _requests) = mock_hub(vec![Reply::ok(r#"{"active":false}"#)]);
    assert_eq!(
        client(&base).introspect_token("tok-1").unwrap_err(),
        HubAuthError::Unauthenticated
    );
}

#[test]
fn an_active_token_with_no_account_is_not_trusted() {
    let (base, _requests) = mock_hub(vec![Reply::ok(r#"{"active":true,"user_id":""}"#)]);
    assert_eq!(
        client(&base).introspect_token("tok-1").unwrap_err(),
        HubAuthError::MalformedResponse
    );
}

// ---------------------------------------------------------------------------
// Fail-closed
// ---------------------------------------------------------------------------

#[test]
fn a_hub_5xx_fails_closed_as_an_upstream_failure() {
    for status in [
        "500 Internal Server Error",
        "502 Bad Gateway",
        "503 Service Unavailable",
    ] {
        let (base, _requests) = mock_hub(vec![Reply::status(status)]);
        let error = client(&base).verify_session("sess").unwrap_err();
        assert_eq!(error, HubAuthError::Upstream, "{status}");
        assert!(error.is_upstream_failure(), "{status}");
    }
}

#[test]
fn an_unreachable_hub_fails_closed() {
    // Port 1 on loopback refuses immediately.
    let client = HubAuthClient::new("http://127.0.0.1:1", Some("s".into())).expect("builds");
    assert_eq!(
        client.verify_session("sess").unwrap_err(),
        HubAuthError::Upstream
    );
}

#[test]
fn an_unparseable_success_body_is_not_trusted() {
    let (base, _requests) = mock_hub(vec![Reply::ok("not json at all")]);
    assert_eq!(
        client(&base).verify_session("sess").unwrap_err(),
        HubAuthError::MalformedResponse
    );
}

#[test]
fn a_base_url_that_is_not_a_plain_origin_is_refused() {
    for bad in [
        "not a url",
        "ftp://hub:8080",
        "http://user:pw@hub:8080",
        "http://hub:8080/?q=1",
        "http://hub:8080/#frag",
    ] {
        assert!(
            HubAuthClient::new(bad, Some("s".into())).is_err(),
            "{bad} must not build a client"
        );
    }
}

// ---------------------------------------------------------------------------
// Caching
// ---------------------------------------------------------------------------

#[test]
fn a_repeated_session_lookup_is_served_from_cache() {
    // The mock serves exactly ONE reply; a second wire call would hang until
    // the client's own timeout and then fail, so a passing assertion here is
    // proof the second lookup never left the process.
    let (base, _requests) = mock_hub(vec![Reply::ok(SESSION_BODY)]);
    let client = client(&base);
    let first = client.verify_session("sess").expect("first resolves");
    let second = client.verify_session("sess").expect("second is cached");
    assert_eq!(first, second);
}

#[test]
fn a_repeated_token_lookup_is_served_from_cache() {
    let (base, _requests) = mock_hub(vec![Reply::ok(TOKEN_BODY)]);
    let client = client(&base);
    let first = client.introspect_token("tok-1").expect("first resolves");
    let second = client.introspect_token("tok-1").expect("second is cached");
    assert_eq!(first, second);
}

#[test]
fn a_definitive_negative_is_cached_so_a_retry_loop_cannot_hammer_the_hub() {
    let (base, _requests) = mock_hub(vec![Reply::status("401 Unauthorized")]);
    let client = client(&base);
    assert_eq!(
        client.verify_session("sess").unwrap_err(),
        HubAuthError::Unauthenticated
    );
    // Served from the negative cache — the mock has no second reply.
    assert_eq!(
        client.verify_session("sess").unwrap_err(),
        HubAuthError::Unauthenticated
    );
}

#[test]
fn an_upstream_failure_is_never_cached_so_a_blip_does_not_become_an_outage() {
    // First call gets a 503, second gets a good answer. If the 503 had been
    // cached, the recovery would be invisible for the whole negative TTL.
    let (base, _requests) = mock_hub(vec![
        Reply::status("503 Service Unavailable"),
        Reply::ok(SESSION_BODY),
    ]);
    let client = client(&base);
    assert_eq!(
        client.verify_session("sess").unwrap_err(),
        HubAuthError::Upstream
    );
    let user = client.verify_session("sess").expect("the hub recovered");
    assert_eq!(user.id, "user-uuid-1");
}

#[test]
fn only_a_definitive_verdict_is_cacheable() {
    assert!(HubAuthError::Unauthenticated.is_cacheable());
    for error in [
        HubAuthError::Upstream,
        HubAuthError::MalformedResponse,
        HubAuthError::NotConfigured,
        HubAuthError::MissingInternalAuth,
        HubAuthError::InvalidCredential,
    ] {
        assert!(!error.is_cacheable(), "{error:?}");
    }
}

#[test]
fn the_cookie_and_token_namespaces_do_not_share_cache_entries() {
    // The same string used as both credentials must ask the hub twice: one
    // reply per lookup, and the second is an introspection with its own body.
    let (base, requests) = mock_hub(vec![Reply::ok(SESSION_BODY), Reply::ok(TOKEN_BODY)]);
    let client = client(&base);
    client.verify_session("same-value").expect("session");
    client.introspect_token("same-value").expect("token");
    let first = requests.recv().expect("first request");
    let second = requests.recv().expect("second request");
    assert!(first.starts_with("GET /api/v1/session "), "{first}");
    assert!(
        second.starts_with("POST /api/v1/tokens/introspect "),
        "{second}"
    );
}

#[test]
fn a_token_expiring_sooner_than_the_ceiling_shortens_its_cache_entry() {
    let now = crate::web_canvas_server::tenant::now_unix();
    let soon = HubToken {
        active: true,
        user_id: "u".into(),
        username: "u".into(),
        scopes: Vec::new(),
        expires_at_unix: Some(now + 30),
    };
    assert!(token_positive_ttl(&soon) <= Duration::from_secs(30));

    let far = HubToken {
        expires_at_unix: Some(now + 100_000),
        ..soon.clone()
    };
    assert_eq!(token_positive_ttl(&far), TOKEN_POSITIVE_TTL);

    let already_expired = HubToken {
        expires_at_unix: Some(now.saturating_sub(10)),
        ..soon.clone()
    };
    assert!(
        token_positive_ttl(&already_expired).is_zero(),
        "an expired token must not earn a cache entry"
    );

    let no_expiry = HubToken {
        expires_at_unix: None,
        ..soon
    };
    assert_eq!(token_positive_ttl(&no_expiry), TOKEN_POSITIVE_TTL);
}

#[test]
fn a_cache_key_never_contains_the_credential() {
    let key = cache_key(b"session", "super-secret-cookie");
    assert_eq!(key.len(), 32);
    assert!(!key.starts_with(b"super"));
    // Domain separation: the same credential hashes differently per question.
    assert_ne!(cache_key(b"session", "x"), cache_key(b"token", "x"));
}

#[test]
fn the_cache_evicts_rather_than_growing_without_bound() {
    let mut cache = HubAuthCache::default();
    let now = Instant::now();
    for index in 0..(MAX_CACHE_ENTRIES + 64) {
        let key = cache_key(b"token", &format!("tok-{index}"));
        cache.insert(key, Verdict::Denied, NEGATIVE_TTL, now);
    }
    assert!(cache.entries.len() <= MAX_CACHE_ENTRIES);
}

#[test]
fn an_expired_entry_is_not_served() {
    let mut cache = HubAuthCache::default();
    let now = Instant::now();
    let key = cache_key(b"token", "tok");
    cache.insert(key, Verdict::Denied, Duration::from_millis(1), now);
    assert!(cache.get(&key, now).is_some());
    assert!(cache.get(&key, now + Duration::from_secs(1)).is_none());
}
