//! Tests for [`crate::hub`] — the inbox create request.
//!
//! The envelope fixtures are the shapes op-hub's own tests build
//! (`backend/internal/httpapi/snapshot_routes_test.go::createBody` and
//! `validSnapshotEnvelope`), so a drift on either side shows up here rather
//! than as a `400` a user has to report.

use serde_json::Value;

use crate::hub::{
    captured_at, create_envelope_template, snapshot_name, snapshot_too_large, snapshots_url,
    source_url, QUOTA_ITEMS, QUOTA_TOTAL_MB, SNAPSHOT_PLACEHOLDER, UPLOAD_TIMEOUT_MS,
};
use crate::hub_time::{format_local_stamp, format_rfc3339_utc, unix_seconds_from_ms};

/// 2026-08-04T09:20:31Z — the instant op-hub's proposal used as its example.
const CAPTURED_MS: f64 = 1_785_835_231_000.0;

/// Splice a document into a template the way `client.js` does.
fn splice(template: &str, document: &str) -> String {
    let parts: Vec<&str> = template.split(SNAPSHOT_PLACEHOLDER).collect();
    assert_eq!(parts.len(), 2, "template must hold exactly one placeholder");
    format!("{}{}{}", parts[0], document, parts[1])
}

#[test]
fn the_url_is_the_bare_route_with_no_query() {
    assert_eq!(
        snapshots_url("https://op.zseven.cn"),
        "https://op.zseven.cn/api/v1/snapshots"
    );
    assert_eq!(
        snapshots_url("http://127.0.0.1:18081"),
        "http://127.0.0.1:18081/api/v1/snapshots"
    );
}

#[test]
fn the_envelope_matches_the_hubs_own_test_fixture() {
    let template = create_envelope_template(
        "Example page",
        "https://example.com/pricing",
        CAPTURED_MS,
        0.0,
    );
    let body = splice(&template, r#"{"nodes":[]}"#);
    assert_eq!(
        body,
        concat!(
            r#"{"kind":"web-snapshot","name":"Example page — 2026-08-04 09:20","#,
            r#""source_url":"https://example.com/pricing","#,
            r#""captured_at":"2026-08-04T09:20:31Z","snapshot":{"nodes":[]}}"#
        )
    );
}

#[test]
fn the_envelope_is_always_exactly_five_fields() {
    // The field count is what makes splicing the document safe: op-hub's
    // decoder refuses a sixth field, so no snapshot text can smuggle one in.
    for url in [
        "https://example.com/",
        "",
        "javascript:alert(1)",
        "not a url",
    ] {
        let template = create_envelope_template("Title", url, CAPTURED_MS, -480.0);
        let body = splice(&template, "{}");
        let parsed: Value = serde_json::from_str(&body).expect("valid JSON");
        let object = parsed.as_object().expect("an object");
        assert_eq!(object.len(), 5, "for {url:?}");
        for field in ["kind", "name", "source_url", "captured_at", "snapshot"] {
            assert!(object.contains_key(field), "{field} missing for {url:?}");
        }
        assert_eq!(object["kind"], "web-snapshot");
    }
}

#[test]
fn a_withheld_source_url_is_null_rather_than_absent() {
    let template = create_envelope_template("Title", "file:///etc/passwd", CAPTURED_MS, 0.0);
    assert!(template.contains(r#""source_url":null,"#));
}

#[test]
fn the_name_and_timestamp_are_escaped_not_interpolated() {
    let template = create_envelope_template("a\"b\\c", "https://example.com/\"x", CAPTURED_MS, 0.0);
    let body = splice(&template, "{}");
    let parsed: Value = serde_json::from_str(&body).expect("valid JSON");
    assert!(parsed["name"]
        .as_str()
        .expect("a name")
        .starts_with("a\"b\\c"));
    // The quote also disqualifies the URL, which Go would have re-encoded.
    assert_eq!(parsed["source_url"], Value::Null);
}

#[test]
fn a_document_that_tries_to_add_a_field_stays_inside_the_snapshot_slot() {
    // Not a claim that op-hub accepts this — it does not; `decodeEnvelope`
    // refuses the sixth key and the trailing content. The claim is that the
    // splice cannot REPLACE a field we set: everything hostile lands after
    // the `"snapshot":` key and before our closing brace.
    let template = create_envelope_template("Real", "https://example.com/", CAPTURED_MS, 0.0);
    let body = splice(&template, r#"{}, "name":"evil""#);
    assert!(body.contains(r#""name":"Real — 2026-08-04 09:20""#));
    assert!(body.ends_with(r#""snapshot":{}, "name":"evil"}"#));
}

#[test]
fn the_captured_instant_is_rfc3339_utc_at_second_precision() {
    assert_eq!(captured_at(CAPTURED_MS), "2026-08-04T09:20:31Z");
    // Sub-second precision is dropped rather than rounded up.
    assert_eq!(captured_at(CAPTURED_MS + 999.0), "2026-08-04T09:20:31Z");
    assert_eq!(captured_at(0.0), "1970-01-01T00:00:00Z");
    // The Hub's parser bounds the field at 35 bytes and requires a `Z`.
    let value = captured_at(CAPTURED_MS);
    assert!((20..=35).contains(&value.len()));
    assert!(value.ends_with('Z'));
}

#[test]
fn leap_days_and_year_boundaries_come_out_right() {
    for (ms, want) in [
        (951_782_400_000.0_f64, "2000-02-29T00:00:00Z"),
        (1_709_164_800_000.0, "2024-02-29T00:00:00Z"),
        (1_767_225_599_000.0, "2025-12-31T23:59:59Z"),
        (1_767_225_600_000.0, "2026-01-01T00:00:00Z"),
        (-1000.0, "1969-12-31T23:59:59Z"),
    ] {
        assert_eq!(format_rfc3339_utc(unix_seconds_from_ms(ms)), want, "{ms}");
    }
}

#[test]
fn a_nonsense_clock_value_produces_the_epoch_rather_than_a_wrapped_year() {
    for ms in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(unix_seconds_from_ms(ms), 0);
    }
    // Beyond the JS `Date` range, clamped rather than wrapped.
    assert!(format_rfc3339_utc(unix_seconds_from_ms(1e300)).starts_with("275760-"));
}

#[test]
fn the_display_stamp_follows_the_callers_timezone() {
    let seconds = unix_seconds_from_ms(CAPTURED_MS);
    assert_eq!(format_local_stamp(seconds, 0), "2026-08-04 09:20");
    assert_eq!(format_local_stamp(seconds, 8 * 60), "2026-08-04 17:20");
    assert_eq!(format_local_stamp(seconds, -11 * 60), "2026-08-03 22:20");
    // An impossible offset is ignored, not applied.
    assert_eq!(format_local_stamp(seconds, 10_000), "2026-08-04 09:20");
}

#[test]
fn a_hostile_title_cannot_produce_a_name_the_hub_refuses() {
    // Control characters, C1 bytes, and the bidi / zero-width run op-hub's
    // `displayControl` rejects all collapse to a single space.
    let name = snapshot_name(
        "  Sale\u{202e}txt.exe\u{0}\n\t \u{200b}\u{feff}end  ",
        CAPTURED_MS,
        0.0,
    );
    assert_eq!(name, "Sale txt.exe end — 2026-08-04 09:20");
    assert!(!name.chars().any(char::is_control));
    assert_eq!(name.trim(), name);
}

#[test]
fn a_name_stays_inside_the_hubs_two_hundred_rune_ceiling() {
    // Counted in runes, as Go's `utf8.RuneCountInString` does — a CJK title
    // must not be measured in bytes and pass a check the server then fails.
    for title in [
        "x".repeat(1000),
        "字".repeat(1000),
        "😀".repeat(1000),
        String::new(),
        "   ".to_owned(),
    ] {
        let name = snapshot_name(&title, CAPTURED_MS, 0.0);
        assert!(
            name.chars().count() <= 200,
            "{} runes",
            name.chars().count()
        );
        assert!(!name.is_empty());
        assert_eq!(name.trim(), name);
    }
}

#[test]
fn an_empty_title_still_names_the_capture() {
    assert_eq!(
        snapshot_name("", CAPTURED_MS, 0.0),
        "Web capture — 2026-08-04 09:20"
    );
    assert_eq!(
        snapshot_name("\u{feff}\u{200b}", CAPTURED_MS, 0.0),
        "Web capture — 2026-08-04 09:20"
    );
}

#[test]
fn only_an_http_url_a_browser_could_have_produced_is_sent() {
    for url in [
        "https://example.com",
        "https://example.com/pricing?a=1#top",
        "http://127.0.0.1:3100/x",
        "https://example.com/%E4%B8%AD%E6%96%87",
    ] {
        assert_eq!(source_url(url).as_deref(), Some(url), "{url}");
    }
    for url in [
        "",
        "   ",
        "file:///etc/passwd",
        "javascript:alert(1)",
        "data:text/html,x",
        "HTTPS://example.com",
        "https://",
        "https:///path",
        "https://user:pw@example.com/",
        "https://example.com/a b",
        "https://example.com/a\nb",
        "https://example.com/\u{4e2d}",
        "https://example.com/<script>",
        "https://example.com/a\"b",
        "//example.com",
    ] {
        assert_eq!(source_url(url), None, "{url}");
    }
    // The 2048-byte ceiling, exactly.
    let long = format!("https://example.com/{}", "a".repeat(2048));
    assert_eq!(source_url(&long), None);
    let fits = format!("https://example.com/{}", "a".repeat(2048 - 20));
    assert_eq!(fits.len(), 2048);
    assert!(source_url(&fits).is_some());
}

#[test]
fn a_trailing_or_leading_space_is_trimmed_rather_than_disqualifying() {
    assert_eq!(
        source_url("  https://example.com/x  ").as_deref(),
        Some("https://example.com/x")
    );
}

#[test]
fn the_size_pre_check_leaves_room_for_the_envelope() {
    let cap = 32.0 * 1024.0 * 1024.0;
    assert!(!snapshot_too_large(cap - 4096.0));
    assert!(snapshot_too_large(cap - 4095.0));
    assert!(snapshot_too_large(cap));
    // The envelope really does fit in what is held back: the widest one this
    // module can build is a 200-rune name plus a 2048-byte URL.
    let widest = create_envelope_template(
        &"字".repeat(400),
        &format!("https://example.com/{}", "a".repeat(2028)),
        CAPTURED_MS,
        0.0,
    );
    assert!(
        widest.len() - SNAPSHOT_PLACEHOLDER.len() < 4096,
        "envelope overhead = {}",
        widest.len() - SNAPSHOT_PLACEHOLDER.len()
    );
}

#[test]
fn the_upload_budget_is_generous_enough_for_a_real_connection() {
    // 32 MiB at a mediocre 3 Mbit/s uplink is about 90 seconds; a timeout
    // below that would report "the hub is stuck" for an ordinary connection.
    const { assert!(UPLOAD_TIMEOUT_MS >= 90_000) };
    assert_eq!(QUOTA_ITEMS, 50);
    assert_eq!(QUOTA_TOTAL_MB, 200);
}
