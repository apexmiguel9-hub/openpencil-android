//! Tests for [`crate::hub_reply`] — classifying what the Hub answered.
//!
//! Every body here is the exact wire shape op-hub emits: the created envelope
//! from `snapshot_routes.go::snapshotCreatedBody`, and the error envelope from
//! `router.go::writeError` (`{"error":{"code","message"}}`), including the two
//! quota messages that share one code.

use crate::hub_reply::{classify_create_reply, CreateFailure, CreateReply};

/// The `201` body, as `writeJSON` renders it (trailing newline included).
const CREATED: &str = concat!(
    r#"{"id":"0123456789abcdef0123456789abcdef","name":"Example page — 2026-08-04 17:20","#,
    r#""created_at":"2026-08-04T09:20:31Z","bytes":184320,"#,
    r#""expires_at":"2026-09-03T09:20:31Z"}"#,
    "\n"
);

fn error_body(code: &str, message: &str) -> String {
    format!(r#"{{"error":{{"code":"{code}","message":"{message}"}}}}"#)
}

fn failure(status: u16, text: &str, retry_after: &str) -> (CreateFailure, String, Option<u32>) {
    match classify_create_reply(status, text, retry_after) {
        CreateReply::Failed {
            code,
            detail,
            retry_after_seconds,
        } => (code, detail, retry_after_seconds),
        other => panic!("expected a failure for {status}, got {other:?}"),
    }
}

#[test]
fn a_created_reply_carries_the_name_the_hub_filed_it_under() {
    let reply = classify_create_reply(201, CREATED, "");
    assert_eq!(
        reply,
        CreateReply::Created {
            id: "0123456789abcdef0123456789abcdef".to_owned(),
            name: "Example page — 2026-08-04 17:20".to_owned(),
            bytes: 184_320.0,
            expires_at: "2026-09-03T09:20:31Z".to_owned(),
        }
    );
}

#[test]
fn a_two_hundred_is_accepted_as_readily_as_a_two_oh_one() {
    // The route answers 201; a proxy that rewrites the status while keeping
    // the body has still filed the snapshot.
    assert!(matches!(
        classify_create_reply(200, CREATED, ""),
        CreateReply::Created { .. }
    ));
}

#[test]
fn a_success_without_an_id_is_not_a_success() {
    // A captive portal or a misrouted proxy answering 200 with a page must
    // not be reported as a filed snapshot: the user would believe a capture
    // is safe that is nowhere.
    for body in [
        "<!doctype html><title>Sign in to the wifi</title>",
        "",
        "{}",
        r#"{"id":"","name":"x"}"#,
        r#"{"id":42}"#,
    ] {
        let (code, _, _) = failure(200, body, "");
        assert_eq!(code, CreateFailure::Unavailable, "for {body:?}");
    }
}

#[test]
fn the_session_expiring_is_its_own_outcome() {
    let (code, detail, _) = failure(
        401,
        &error_body("authentication_required", "Authentication required"),
        "",
    );
    assert_eq!(code, CreateFailure::SignedOut);
    assert_eq!(detail, "Authentication required");
}

#[test]
fn an_origin_or_csrf_refusal_is_distinct_from_a_signed_out_session() {
    let (code, detail, _) = failure(403, &error_body("forbidden", "Operation forbidden"), "");
    assert_eq!(code, CreateFailure::Forbidden);
    assert_eq!(detail, "Operation forbidden");
}

#[test]
fn both_quota_ceilings_arrive_as_one_outcome_with_the_servers_own_sentence() {
    // op-hub answers ErrItemQuota and ErrByteQuota with the SAME
    // `quota_exceeded` code and differs only in prose, so the code cannot
    // tell them apart and this client does not pretend otherwise.
    for message in [
        "The inbox already holds the maximum number of snapshots",
        "The inbox has no remaining storage for this snapshot",
    ] {
        let (code, detail, _) = failure(409, &error_body("quota_exceeded", message), "");
        assert_eq!(code, CreateFailure::Quota);
        assert_eq!(detail, message);
    }
}

#[test]
fn a_conflict_that_is_not_a_quota_is_a_refusal_not_a_full_inbox() {
    let (code, _, _) = failure(409, &error_body("something_else", "Nope"), "");
    assert_eq!(code, CreateFailure::Rejected);
}

#[test]
fn a_rejected_envelope_reports_what_the_hub_objected_to() {
    let (code, detail, _) = failure(
        400,
        &error_body("invalid_request", "Snapshot request is invalid"),
        "",
    );
    assert_eq!(code, CreateFailure::Rejected);
    assert_eq!(detail, "Snapshot request is invalid");
}

#[test]
fn an_over_cap_body_is_its_own_outcome() {
    let (code, _, _) = failure(
        413,
        &error_body("request_too_large", "Request body is too large"),
        "",
    );
    assert_eq!(code, CreateFailure::TooLarge);
}

#[test]
fn the_rate_limit_carries_the_wait_the_hub_asked_for() {
    let (code, detail, wait) = failure(
        429,
        &error_body("rate_limited", "Too many snapshot uploads"),
        "180",
    );
    assert_eq!(code, CreateFailure::RateLimited);
    assert_eq!(detail, "Too many snapshot uploads");
    assert_eq!(wait, Some(180));
}

#[test]
fn a_retry_after_that_is_not_delta_seconds_yields_no_wait() {
    // The HTTP-date form needs the current time to turn into a duration, and
    // this crate has no clock. A missing wait is better than a wrong one.
    for header in [
        "",
        "   ",
        "Wed, 21 Oct 2026 07:28:00 GMT",
        "-5",
        "1.5",
        "60s",
        "99999999999999999999",
    ] {
        let (_, _, wait) = failure(429, &error_body("rate_limited", "x"), header);
        assert_eq!(wait, None, "for {header:?}");
    }
    // Clamped at both ends rather than believed.
    let (_, _, floor) = failure(429, "", "0");
    assert_eq!(floor, Some(1));
    let (_, _, ceiling) = failure(429, "", "999999");
    assert_eq!(ceiling, Some(86_400));
}

#[test]
fn a_hub_without_an_inbox_reads_as_unavailable_rather_than_as_the_users_fault() {
    // `routes.snapshots == nil` leaves the route unregistered, so the request
    // falls through to the static site: 404 on the path, 405 on the method.
    for (status, body) in [
        (404, error_body("not_found", "Resource not found")),
        (405, error_body("method_not_allowed", "Method not allowed")),
        (500, String::new()),
        (
            503,
            error_body("service_unavailable", "Service unavailable"),
        ),
        (502, "<html>bad gateway</html>".to_owned()),
    ] {
        let (code, _, _) = failure(status, &body, "");
        assert_eq!(code, CreateFailure::Unavailable, "for {status}");
    }
    // The busy-slot refusal asks for a one-second wait; it is carried too.
    let (_, _, wait) = failure(503, &error_body("service_unavailable", "x"), "1");
    assert_eq!(wait, Some(1));
}

#[test]
fn a_body_that_is_not_json_still_produces_a_readable_line() {
    let (_, detail, _) = failure(500, "  upstream connect error  ", "");
    assert_eq!(detail, "upstream connect error");
    let (_, empty, _) = failure(500, "", "");
    assert_eq!(empty, "HTTP 500");
}

#[test]
fn server_chosen_text_is_capped_before_it_reaches_the_popup() {
    let long = "x".repeat(1000);
    let (_, detail, _) = failure(400, &error_body("invalid_request", &long), "");
    assert_eq!(detail.chars().count(), 200);
    let (_, raw, _) = failure(500, &long, "");
    assert_eq!(raw.chars().count(), 200);
}

#[test]
fn a_name_the_hub_echoes_back_is_re_checked_before_it_is_rendered() {
    // The reply is not a place to start trusting: a name with a control
    // character in it would corrupt the popup's single-line status.
    let reply = classify_create_reply(
        201,
        &format!(
            r#"{{"id":"{}","name":"line\nbreak","bytes":1,"expires_at":"z"}}"#,
            "a".repeat(32)
        ),
        "",
    );
    match reply {
        CreateReply::Created { name, .. } => assert_eq!(name, ""),
        other => panic!("expected a created reply, got {other:?}"),
    }
    let over_long = classify_create_reply(
        201,
        &format!(
            r#"{{"id":"{}","name":"{}","bytes":1,"expires_at":"z"}}"#,
            "a".repeat(32),
            "n".repeat(500)
        ),
        "",
    );
    match over_long {
        CreateReply::Created { name, .. } => assert_eq!(name.chars().count(), 200),
        other => panic!("expected a created reply, got {other:?}"),
    }
}

#[test]
fn failure_codes_keep_the_spellings_the_popup_switches_on() {
    for (code, wire) in [
        (CreateFailure::SignedOut, "signedOut"),
        (CreateFailure::Forbidden, "forbidden"),
        (CreateFailure::Quota, "quota"),
        (CreateFailure::TooLarge, "tooLarge"),
        (CreateFailure::RateLimited, "rateLimited"),
        (CreateFailure::Rejected, "rejected"),
        (CreateFailure::Unavailable, "unavailable"),
    ] {
        assert_eq!(code.as_str(), wire);
    }
}
