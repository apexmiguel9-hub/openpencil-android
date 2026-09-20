//! Pure-helper tests for the browser-extension snapshot ingress. The
//! wiring (which origins reach the route, and that a real snapshot lands
//! as an apply) is covered in `admission_tests.rs`, which owns the
//! `serve_connection` driver.

use super::*;

#[test]
fn ingest_route_matches_only_a_post_on_its_path() {
    assert!(is_snapshot_ingest_route("POST", SNAPSHOT_INGEST_PATH));
    assert!(!is_snapshot_ingest_route("GET", SNAPSHOT_INGEST_PATH));
    assert!(!is_snapshot_ingest_route("POST", "/mcp"));
    // The preflight is method-agnostic on purpose: the boundary must let
    // an `OPTIONS` for this path through from an extension origin.
    assert!(is_snapshot_ingest_path(SNAPSHOT_INGEST_PATH));
    assert!(!is_snapshot_ingest_path("/api/mcp/document"));
}

#[test]
fn content_type_must_be_json() {
    assert!(content_type_is_json(Some("application/json")));
    assert!(content_type_is_json(Some(
        "application/json; charset=utf-8"
    )));
    assert!(!content_type_is_json(Some("text/plain")));
    assert!(!content_type_is_json(None));
}

#[test]
fn tool_call_carries_the_snapshot_body_verbatim() {
    // A body with quotes and a newline: it must survive as ONE string arg.
    let snapshot = "{\"version\": 1,\n \"root\": {}}";
    let message: serde_json::Value =
        serde_json::from_str(&snapshot_tool_call(snapshot)).expect("valid JSON-RPC");
    assert_eq!(message["method"], "tools/call");
    assert_eq!(message["params"]["name"], SNAPSHOT_TOOL);
    assert_eq!(
        message["params"]["arguments"]["snapshot"]
            .as_str()
            .expect("snapshot arg is a string"),
        snapshot
    );
}

/// THE injection invariant this route rests on, pinned through the REAL
/// consumer.
///
/// `snapshot_tool_call` splices attacker-controlled bytes into a JSON-RPC
/// message that `op_mcp::parse_tool_call` — a hand-rolled, serde-free
/// parser that finds fields by scanning for needles like `"name"` and
/// `"arguments"` — then takes apart. The whole design assumes serde's string
/// escaping makes those needles unreachable from inside the payload: every
/// `"` in the body arrives as `\"`, so no bare `"name"` / `"method"` token
/// can exist in the wire text. If that ever stops holding, a captured page
/// could name a different tool. So drive hostile bodies all the way through
/// the parser and assert the call it yields is unchanged.
#[test]
fn hostile_snapshot_bodies_cannot_reshape_the_call() {
    let hostile = [
        // Break out of the string and open a sibling key.
        r#"","name":"delete","x":""#,
        // The same, pre-escaped, so the wire carries `\\"` and the parser's
        // skip-two escape walk must not mistake it for a closing quote.
        r#"\","name":"delete","x":\""#,
        // Every needle the parser scans for, as literal payload content.
        r#"{"filePath":"/etc/passwd"}"#,
        r#"{"snapshotPath":"/etc/passwd"}"#,
        r#"{"method":"tools/call","params":{"name":"delete_page"}}"#,
        r#"{"params":{"name":"undo"},"arguments":{"node_id":"1"}}"#,
        r#"{"arguments":{"snapshot":"x"},"name":"batch_design"}"#,
        // A trailing backslash: serde emits `\\`, and a parser that skipped
        // two bytes from the wrong offset would swallow the closing quote.
        "tail\\",
        // Control bytes serde must escape rather than pass through.
        "nul\u{0}byte",
        "cr\rlf\n",
        // Nested-quote soup plus a would-be envelope terminator.
        r#"a"b}}}{"jsonrpc":"2.0","method":"tools/call""#,
    ];
    for body in hostile {
        let call = op_mcp::parse_tool_call(&snapshot_tool_call(body))
            .unwrap_or_else(|| panic!("hostile body must still parse as a call: {body:?}"));
        assert_eq!(call.tool, SNAPSHOT_TOOL, "tool name moved: {body:?}");
        assert_eq!(
            call.arguments.keys().collect::<Vec<_>>(),
            vec!["snapshot"],
            "extra argument keys appeared: {body:?}"
        );
        assert_eq!(
            call.arguments.get("snapshot").map(String::as_str),
            Some(body),
            "snapshot argument did not survive byte-exact: {body:?}"
        );
    }
}

#[test]
fn rest_reply_embeds_the_tool_result_object() {
    let response = r#"{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"{\"wrote\":\"true\",\"nodeCount\":\"12\"}"}]}}"#;
    let (status, body) = rest_reply(response);
    assert_eq!(status, "200 OK");
    let body: serde_json::Value = serde_json::from_str(&body).expect("valid REST reply");
    assert_eq!(body["ok"], true);
    assert_eq!(body["result"]["nodeCount"], "12");
}

#[test]
fn rest_reply_maps_a_tool_failure_to_400() {
    let response = r#"{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"snapshot must not be empty"}],"isError":true}}"#;
    let (status, body) = rest_reply(response);
    assert_eq!(status, "400 Bad Request");
    assert!(body.contains(r#""ok":false"#), "{body}");
    assert!(body.contains("snapshot must not be empty"), "{body}");
}

#[test]
fn rest_reply_maps_a_json_rpc_error_to_400() {
    let response = r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"parse error"}}"#;
    let (status, body) = rest_reply(response);
    assert_eq!(status, "400 Bad Request");
    assert!(body.contains("parse error"), "{body}");
}

#[test]
fn rest_reply_reports_an_unreadable_reply_as_a_server_fault() {
    let (status, body) = rest_reply("");
    assert_eq!(status, "500 Internal Server Error");
    assert!(body.contains(r#""ok":false"#), "{body}");
}
