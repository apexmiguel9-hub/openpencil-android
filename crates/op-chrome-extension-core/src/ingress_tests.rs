//! Tests for [`crate::ingress`] — envelope construction and reply
//! classification for the two local ingresses.

use op_util::json_escape::escape_json_quoted;
use serde_json::Value;

use crate::ingress::{
    classify_ingest_reply, classify_mcp_reply, mcp_envelope_template, FailureCode, Reply,
    SNAPSHOT_PLACEHOLDER,
};

/// What the popup's glue produces: the template split on the placeholder,
/// rejoined around the snapshot's JSON string literal.
///
/// `escape_json_quoted` stands in for the `JSON.stringify` the glue calls;
/// the two agree on every input a Rust `&str` can hold (they differ only on
/// lone surrogates, which is exactly why the real splice happens in JS).
fn render_envelope(snapshot: &str) -> String {
    let template = mcp_envelope_template();
    let (prefix, suffix) = template
        .split_once(SNAPSHOT_PLACEHOLDER)
        .expect("the template must carry exactly one placeholder");
    format!("{prefix}{}{suffix}", escape_json_quoted(snapshot))
}

fn failure(reply: &Reply) -> (FailureCode, &str) {
    match reply {
        Reply::Failed { code, detail } => (*code, detail.as_str()),
        other => panic!("expected a failure, got {other:?}"),
    }
}

fn imported(reply: &Reply) -> (f64, &[String]) {
    match reply {
        Reply::Imported {
            node_count,
            warnings,
        } => (*node_count, warnings.as_slice()),
        other => panic!("expected an import, got {other:?}"),
    }
}

/* ---------------------------------------------------------------- envelope */

#[test]
fn the_template_pins_the_exact_wire_shape() {
    // The popup's glue only splits this on the placeholder, so the byte
    // sequence — method, tool name, and the key ORDER the server contract
    // names (method → params.name → params.arguments.snapshot) — is fixed
    // here and nowhere else.
    assert_eq!(
        mcp_envelope_template(),
        concat!(
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","#,
            r#""params":{"name":"import_web_snapshot","#,
            r#""arguments":{"snapshot":__OPENPENCIL_SNAPSHOT__}}}"#,
        )
    );
}

#[test]
fn the_template_carries_exactly_one_placeholder() {
    let template = mcp_envelope_template();
    assert_eq!(template.matches(SNAPSHOT_PLACEHOLDER).count(), 1);
    // A `String.prototype.split` on the placeholder must yield two halves,
    // which is what the glue asserts before joining.
    assert_eq!(template.split(SNAPSHOT_PLACEHOLDER).count(), 2);
}

#[test]
fn the_envelope_is_a_wellformed_tools_call() {
    let body = render_envelope(r#"{"nodes":[]}"#);
    let parsed: Value = serde_json::from_str(&body).expect("envelope must be valid JSON");
    assert_eq!(parsed["jsonrpc"], "2.0");
    assert_eq!(parsed["id"], 1);
    assert_eq!(parsed["method"], "tools/call");
    assert_eq!(parsed["params"]["name"], "import_web_snapshot");
    // The snapshot travels as a JSON *string*; the tool parses it itself.
    assert_eq!(parsed["params"]["arguments"]["snapshot"], r#"{"nodes":[]}"#);
}

#[test]
fn the_envelope_escapes_everything_that_could_break_out_of_the_string() {
    // A snapshot is attacker-influenced text (page titles, alt text, URLs).
    let hostile = "\"}}},\"method\":\"tools/call\",\"x\":\"\\ \n \t \r \u{0} \u{1f}";
    let body = render_envelope(hostile);
    let parsed: Value = serde_json::from_str(&body).expect("envelope must stay valid JSON");
    assert_eq!(parsed["method"], "tools/call");
    assert_eq!(parsed["params"]["arguments"]["snapshot"], hostile);
    // The raw control characters must not appear literally in the wire text.
    assert!(!body.contains('\n'));
    assert!(!body.contains('\u{0}'));
}

#[test]
fn a_snapshot_that_names_the_placeholder_cannot_reach_the_template_seam() {
    // The splice is prefix + literal + suffix, so a placeholder-shaped run
    // inside the snapshot is just more escaped text; it never gets a second
    // substitution pass.
    let snapshot = format!(r#"{{"title":"{SNAPSHOT_PLACEHOLDER}"}}"#);
    let parsed: Value = serde_json::from_str(&render_envelope(&snapshot)).unwrap();
    assert_eq!(parsed["params"]["arguments"]["snapshot"], snapshot);
}

#[test]
fn the_envelope_survives_non_ascii_content() {
    let snapshot = r#"{"text":"周报 😀 «quote»"}"#;
    let parsed: Value = serde_json::from_str(&render_envelope(snapshot)).unwrap();
    assert_eq!(parsed["params"]["arguments"]["snapshot"], snapshot);
}

/* ------------------------------------------------------- ingest-route reply */

#[test]
fn a_404_asks_for_the_fallback() {
    assert_eq!(classify_ingest_reply(404, "Not Found"), Reply::Fallback);
}

#[test]
fn a_successful_ingest_yields_the_node_count_and_warnings() {
    let body = r#"{"ok":true,"result":{"nodeCount":42,"warnings":"a\nb"}}"#;
    let reply = classify_ingest_reply(200, body);
    let (count, warnings) = imported(&reply);
    assert_eq!(count, 42.0);
    assert_eq!(warnings, ["a".to_owned(), "b".to_owned()]);
}

#[test]
fn an_empty_warning_string_yields_no_warnings() {
    let reply = classify_ingest_reply(200, r#"{"ok":true,"result":{"nodeCount":1,"warnings":""}}"#);
    let (count, warnings) = imported(&reply);
    assert_eq!(count, 1.0);
    assert!(warnings.is_empty());
}

#[test]
fn a_missing_node_count_reads_as_zero() {
    let reply = classify_ingest_reply(200, r#"{"ok":true,"result":{}}"#);
    assert_eq!(imported(&reply).0, 0.0);
    let reply = classify_ingest_reply(200, r#"{"ok":true,"result":{"nodeCount":"7"}}"#);
    assert_eq!(imported(&reply).0, 7.0);
    let reply = classify_ingest_reply(200, r#"{"ok":true,"result":{"nodeCount":"nope"}}"#);
    assert_eq!(imported(&reply).0, 0.0);
}

#[test]
fn a_403_is_the_no_ingress_case() {
    let reply = classify_ingest_reply(403, r#"{"error":"extension origin refused"}"#);
    assert_eq!(
        failure(&reply),
        (FailureCode::Forbidden, "extension origin refused")
    );
}

#[test]
fn a_2xx_without_the_ok_flag_is_an_import_failure() {
    let reply = classify_ingest_reply(200, r#"{"ok":false,"error":{"message":"bad snapshot"}}"#);
    assert_eq!(failure(&reply), (FailureCode::Import, "bad snapshot"));
}

#[test]
fn a_non_json_body_is_reported_verbatim_and_capped() {
    let reply = classify_ingest_reply(500, "boom");
    assert_eq!(failure(&reply), (FailureCode::Import, "boom"));

    let long = "x".repeat(1000);
    let capped = classify_ingest_reply(500, &long);
    assert_eq!(failure(&capped).1.len(), 400);
}

#[test]
fn every_detail_branch_is_capped_not_just_the_raw_body() {
    let long = "x".repeat(1000);
    // 1. `error` as a string.
    let body = format!(r#"{{"ok":false,"error":"{long}"}}"#);
    assert_eq!(failure(&classify_ingest_reply(200, &body)).1.len(), 400);
    // 2. `error.message`.
    let body = format!(r#"{{"ok":false,"error":{{"message":"{long}"}}}}"#);
    assert_eq!(failure(&classify_ingest_reply(200, &body)).1.len(), 400);
    // 3. the MCP content envelope's text, reached through both classifiers.
    let body = format!(r#"{{"result":{{"isError":true,"content":[{{"text":"{long}"}}]}}}}"#);
    assert_eq!(failure(&classify_mcp_reply(200, &body)).1.len(), 400);
    assert_eq!(failure(&classify_ingest_reply(500, &body)).1.len(), 400);
    // 4. a forbidden reply, which takes the same path.
    let body = format!(r#"{{"error":{{"message":"{long}"}}}}"#);
    assert_eq!(failure(&classify_mcp_reply(403, &body)).1.len(), 400);
}

#[test]
fn the_cap_never_splits_a_surrogate_pair() {
    // 399 ASCII units then an emoji: the 400th unit is a high surrogate, so
    // the whole emoji is dropped rather than a half of it kept.
    let message = format!("{}😀tail", "x".repeat(399));
    let body = format!(r#"{{"ok":false,"error":"{message}"}}"#);
    let detail = failure(&classify_ingest_reply(200, &body)).1.to_owned();
    assert_eq!(detail, "x".repeat(399));
}

#[test]
fn a_short_structured_message_is_left_alone() {
    let reply = classify_ingest_reply(200, r#"{"ok":false,"error":"too many nodes"}"#);
    assert_eq!(failure(&reply), (FailureCode::Import, "too many nodes"));
}

#[test]
fn an_empty_body_falls_back_to_the_status_line() {
    assert_eq!(
        failure(&classify_ingest_reply(502, "")),
        (FailureCode::Import, "HTTP 502")
    );
}

/* ---------------------------------------------------------------- mcp reply */

#[test]
fn a_successful_tool_call_yields_the_parsed_payload() {
    let body = r#"{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text",
        "text":"{\"nodeCount\":9,\"warnings\":\"w\"}"}]}}"#;
    let reply = classify_mcp_reply(200, body);
    let (count, warnings) = imported(&reply);
    assert_eq!(count, 9.0);
    assert_eq!(warnings, ["w".to_owned()]);
}

#[test]
fn an_unparseable_tool_payload_degrades_to_the_zero_outcome() {
    let body = r#"{"result":{"content":[{"type":"text","text":"not json"}]}}"#;
    let reply = classify_mcp_reply(200, body);
    assert_eq!(imported(&reply), (0.0, &[] as &[String]));
}

#[test]
fn an_is_error_result_is_an_import_failure() {
    let body = r#"{"result":{"isError":true,"content":[{"type":"text","text":"refused"}]}}"#;
    assert_eq!(
        failure(&classify_mcp_reply(200, body)),
        (FailureCode::Import, "refused")
    );
}

#[test]
fn a_missing_result_is_an_import_failure() {
    let body = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"no such tool"}}"#;
    assert_eq!(
        failure(&classify_mcp_reply(200, body)),
        (FailureCode::Import, "no such tool")
    );
}

#[test]
fn the_mcp_fallback_never_falls_back_again_on_a_404() {
    // Only the ingest route may answer `Fallback`; a 404 here is terminal.
    assert_eq!(failure(&classify_mcp_reply(404, "")).0, FailureCode::Import);
}

#[test]
fn a_401_or_403_on_the_fallback_is_the_no_ingress_case() {
    for status in [401u16, 403] {
        let reply = classify_mcp_reply(status, r#"{"error":{"message":"token required"}}"#);
        assert_eq!(failure(&reply), (FailureCode::Forbidden, "token required"));
    }
}
