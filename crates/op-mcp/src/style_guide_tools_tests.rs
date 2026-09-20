//! Style-guide MCP parity tests.

use std::collections::BTreeMap;

use super::{
    get_style_guide_snapshot, get_style_guide_tags_snapshot, list_style_guides_snapshot, McpTool,
    ToolErrorCode, ToolOutcome,
};

#[test]
fn get_style_guide_tags_returns_light_and_dark_vocab() {
    match get_style_guide_tags_snapshot().call(&BTreeMap::new()) {
        ToolOutcome::Ok(out) => {
            assert!(out
                .get("tags")
                .is_some_and(|tags| tags.contains("\"light-mode\"")));
            assert!(out
                .get("tags")
                .is_some_and(|tags| tags.contains("\"dark-mode\"")));
            let count: usize = out.get("count").expect("count").parse().expect("count");
            assert!(count > 50);
        }
        other => panic!("expected tags ok, got {other:?}"),
    }
}

#[test]
fn get_style_guide_finds_specific_guide_by_name() {
    let mut args = BTreeMap::new();
    args.insert("name".into(), "saas-clean-light".into());

    match get_style_guide_snapshot().call(&args) {
        ToolOutcome::Ok(out) => {
            assert_eq!(out.get("name"), Some(&"saas-clean-light".to_string()));
            assert_eq!(out.get("platform"), Some(&"webapp".to_string()));
            assert!(out
                .get("tags")
                .is_some_and(|tags| tags.contains("\"light-mode\"")));
            assert!(out
                .get("content")
                .is_some_and(|content| content.contains("saas-clean-light")));
        }
        other => panic!("expected guide ok, got {other:?}"),
    }
}

#[test]
fn listing_style_guides_covers_the_whole_shipped_corpus() {
    let out = match list_style_guides_snapshot().call(&BTreeMap::new()) {
        ToolOutcome::OkJson(json) => json,
        other => panic!("unexpected outcome: {other:?}"),
    };
    let value: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    let guides = value["guides"].as_array().expect("guides array");
    let corpus = op_ai_skills::style_guide::style_guide_registry().len();
    assert!(
        guides.len() >= corpus,
        "listing dropped corpus entries: {} < {corpus}",
        guides.len()
    );
    assert_eq!(value["count"].as_u64(), Some(guides.len() as u64));
    // Every corpus entry is addressable by the id the list hands back.
    for guide in op_ai_skills::style_guide::style_guide_registry() {
        assert!(
            guides
                .iter()
                .any(|entry| entry["id"].as_str() == Some(guide.name.as_str())),
            "{} missing from the listing",
            guide.name
        );
    }
}

#[test]
fn requesting_one_style_guide_by_id_carries_its_markdown() {
    let first = &op_ai_skills::style_guide::style_guide_registry()[0];
    let mut args = BTreeMap::new();
    args.insert("id".to_string(), first.name.clone());
    let out = match list_style_guides_snapshot().call(&args) {
        ToolOutcome::OkJson(json) => json,
        other => panic!("unexpected outcome: {other:?}"),
    };
    let value: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    let guides = value["guides"].as_array().expect("guides array");
    assert_eq!(guides.len(), 1);
    assert_eq!(guides[0]["content"].as_str(), Some(first.content.as_str()));
}

#[test]
fn an_unknown_style_guide_id_is_a_named_argument_error() {
    let mut args = BTreeMap::new();
    args.insert("id".to_string(), "no-such-guide".to_string());
    match list_style_guides_snapshot().call(&args) {
        ToolOutcome::Err(code, message) => {
            assert_eq!(code, ToolErrorCode::InvalidArgument);
            assert!(message.contains("no-such-guide"), "{message}");
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
}
