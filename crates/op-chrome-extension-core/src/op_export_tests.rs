//! Tests for [`crate::op_export`] — turning an extractor snapshot into a
//! ready-to-open `.op` (`PenDocument`) document. Exercised natively; the wasm
//! boundary is a thin type conversion over this.

use crate::op_export::{snapshot_to_op, OpExport};
use jian_ops_schema::document::PenDocument;

/// A minimal but real v1 snapshot: a body root, one child box, and text.
const SAMPLE: &str = r#"{
  "version": 1,
  "source": "https://example.com/page",
  "title": "My Test Page",
  "viewport": { "width": 1440, "height": 900 },
  "root": {
    "kind": "element",
    "tag": "body",
    "rect": { "x": 0, "y": 0, "w": 1440, "h": 600 },
    "styles": { "background-color": "rgb(255, 255, 255)" },
    "children": [
      {
        "kind": "element",
        "tag": "div",
        "rect": { "x": 24, "y": 24, "w": 300, "h": 80 },
        "styles": {
          "background-color": "rgba(16, 32, 48, 1)",
          "border-radius": "8px"
        },
        "children": [
          {
            "kind": "text",
            "rect": { "x": 40, "y": 48, "w": 120, "h": 24 },
            "text": "Hello world",
            "styles": {
              "color": "rgb(255, 255, 255)",
              "font-family": "Inter, sans-serif",
              "font-size": "16px",
              "font-weight": "700",
              "line-height": "24px"
            }
          }
        ]
      }
    ]
  }
}"#;

#[test]
fn converts_a_snapshot_into_a_valid_op_document() {
    let OpExport::Ready {
        op,
        node_count,
        warnings,
    } = snapshot_to_op(SAMPLE, Some("Ignored By Snapshot Path"))
    else {
        panic!("a non-empty snapshot must convert");
    };

    // The `op` string is the exact `.op` file contents: it must round-trip
    // back into the canonical schema, which is what makes it double-click open.
    let document: PenDocument = serde_json::from_str(&op).expect(".op must be valid PenDocument");

    // The snapshot path names the document from the snapshot's own `title`
    // field (the extractor fills it with the page title).
    assert_eq!(document.name.as_deref(), Some("My Test Page"));
    assert!(!document.children.is_empty(), "document must carry nodes");

    // body frame + child box + its text = 3 nodes.
    assert_eq!(node_count, 3);
    assert!(warnings.is_empty(), "a clean capture warns about nothing");
}

#[test]
fn passes_a_non_blank_title_through_as_document_name() {
    // A snapshot with no `title` of its own falls back to the importer's
    // "Web Snapshot" default; the title argument is threaded through
    // `HtmlImportOptions::document_name` regardless (forward-compatible).
    let no_title = SAMPLE.replace("\"title\": \"My Test Page\",", "");
    let OpExport::Ready { op, .. } = snapshot_to_op(&no_title, Some("  My Tab Title  ")) else {
        panic!("snapshot without a title must still convert");
    };
    let document: PenDocument = serde_json::from_str(&op).expect(".op must be valid");
    assert_eq!(document.name.as_deref(), Some("Web Snapshot"));
}

#[test]
fn refuses_malformed_json_with_an_actionable_error() {
    let OpExport::Failed { error } = snapshot_to_op("not json at all", Some("x")) else {
        panic!("malformed JSON must be refused, not silently downloaded");
    };
    assert!(
        error.starts_with("no importable content:"),
        "unexpected error: {error}"
    );
}

#[test]
fn refuses_an_unsupported_snapshot_version() {
    let bumped = SAMPLE.replace("\"version\": 1", "\"version\": 99");
    let OpExport::Failed { error } = snapshot_to_op(&bumped, None) else {
        panic!("an unsupported version must be refused");
    };
    assert!(error.starts_with("no importable content:"));
}
