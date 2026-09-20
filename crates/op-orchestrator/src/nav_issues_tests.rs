//! Tests for the Track B `navIssues` echo — see `nav_issues.rs` module doc.

use super::*;
use jian_ops_schema::PenDocument;
use op_editor_core::EditorState;

fn state_from_json(json: &str) -> EditorState {
    let doc: PenDocument = serde_json::from_str(json).expect("valid PenDocument");
    EditorState::from_document(doc)
}

/// Two screen-marked frames, each with a bottom-tab-bar. The "Home" tab is
/// already bound to itself (as `wire_screen_navigation` would leave it once
/// wired); the "Profile" tab has no `events` yet and its label matches the
/// OTHER screen's name, so it alone should be echoed with the node id and
/// the exact patch to bind.
const TWO_SCREENS_UNBOUND_PROFILE_TAB: &str = r##"{ "version": "1.0", "children": [
    { "type": "frame", "id": "home", "name": "Home", "screen": "/",
      "width": 390, "height": 844, "layout": "vertical",
      "children": [
        { "type": "frame", "id": "nav", "name": "Bottom Nav", "role": "bottom-tab-bar",
          "layout": "horizontal", "width": "fill_container",
          "children": [
            { "type": "frame", "id": "tab-home", "layout": "vertical",
              "events": { "onTap": [ { "replace": "\"/\"" } ] },
              "children": [ { "type": "text", "id": "t1", "content": "Home" } ] },
            { "type": "frame", "id": "tab-profile", "layout": "vertical",
              "children": [ { "type": "text", "id": "t2", "content": "Profile" } ] }
          ] }
      ] },
    { "type": "frame", "id": "profile", "name": "Profile", "screen": "/profile",
      "width": 390, "height": 844 }
] }"##;

#[test]
fn unbound_matching_tab_is_echoed_with_id_and_suggested_patch() {
    let state = state_from_json(TWO_SCREENS_UNBOUND_PROFILE_TAB);
    let issues = scan_nav_issues(&state);
    assert_eq!(issues.len(), 1, "{issues:?}");
    assert!(issues[0].contains("tab-profile"), "{issues:?}");
    assert!(
        issues[0].contains("\"/\""),
        "names the screen it sits on: {issues:?}"
    );
    assert!(
        issues[0].contains(r#"{"replace":"\"/profile\""}"#),
        "carries the exact bindable patch: {issues:?}"
    );
}

#[test]
fn already_bound_tab_is_not_echoed() {
    let mut state = state_from_json(TWO_SCREENS_UNBOUND_PROFILE_TAB);
    // Bind the profile tab directly on the document before scanning.
    let home = &mut state.active_children_mut()[0];
    let nav = &mut home.children_mut().unwrap()[0];
    let tab_profile = &mut nav.children_mut().unwrap()[1];
    let PenNode::Frame(tab_profile) = tab_profile else {
        panic!("tab-profile frame");
    };
    tab_profile.events = Some(
        serde_json::from_value(serde_json::json!({"onTap": [{"replace": "\"/profile\""}]}))
            .unwrap(),
    );

    let issues = scan_nav_issues(&state);
    assert!(
        issues.is_empty(),
        "already-bound tab must not be echoed: {issues:?}"
    );
}

#[test]
fn single_marked_screen_has_no_navigation_to_check_yet() {
    let state = state_from_json(
        r##"{ "version": "1.0", "children": [
            { "type": "frame", "id": "home", "name": "Home", "screen": "/",
              "width": 390, "height": 844, "layout": "vertical",
              "children": [
                { "type": "frame", "id": "nav", "name": "Bottom Nav", "role": "bottom-tab-bar",
                  "layout": "horizontal", "width": "fill_container",
                  "children": [
                    { "type": "frame", "id": "tab-profile", "layout": "vertical",
                      "children": [ { "type": "text", "id": "t2", "content": "Profile" } ] }
                  ] }
              ] },
            { "type": "frame", "id": "profile", "name": "Profile", "width": 390, "height": 844 }
        ] }"##,
    );
    // Only "Home" is screen-marked — "Profile" is a plain frame, not yet
    // committed to routing, so the gate must stay closed.
    assert!(scan_nav_issues(&state).is_empty());
}

#[test]
fn unmatched_tab_label_is_not_echoed() {
    let state = state_from_json(
        r##"{ "version": "1.0", "children": [
            { "type": "frame", "id": "home", "name": "Home", "screen": "/",
              "width": 390, "height": 844, "layout": "vertical",
              "children": [
                { "type": "frame", "id": "nav", "name": "Bottom Nav", "role": "bottom-tab-bar",
                  "layout": "horizontal", "width": "fill_container",
                  "children": [
                    { "type": "frame", "id": "tab-settings", "layout": "vertical",
                      "children": [ { "type": "text", "id": "t3", "content": "Settings" } ] }
                  ] }
              ] },
            { "type": "frame", "id": "profile", "name": "Profile", "screen": "/profile",
              "width": 390, "height": 844 }
        ] }"##,
    );
    // "Settings" matches neither "Home" nor "Profile" — ambiguous, so the
    // echo stays silent rather than guessing a wrong destination.
    assert!(scan_nav_issues(&state).is_empty());
}

/// `0808-k3-2.op`'s shape: a four-tab bar over a two-screen document. 今夜 and
/// 星图 have screens; 地点 and 我的 do not.
const CJK_FOUR_TABS_TWO_SCREENS: &str = r##"{ "version": "1.0", "children": [
    { "type": "frame", "id": "s1", "name": "Nocturne 今夜", "screen": "/",
      "width": 375, "height": 812, "layout": "vertical",
      "children": [
        { "type": "frame", "id": "nav1", "name": "Bottom Nav", "role": "bottom-tab-bar",
          "layout": "horizontal", "width": "fill_container",
          "children": [
            { "type": "frame", "id": "a-tonight", "layout": "vertical",
              "children": [ { "type": "text", "id": "a1", "content": "今夜" } ] },
            { "type": "frame", "id": "a-starmap", "layout": "vertical",
              "children": [ { "type": "text", "id": "a2", "content": "星图" } ] },
            { "type": "frame", "id": "a-places", "layout": "vertical",
              "children": [ { "type": "text", "id": "a3", "content": "地点" } ] },
            { "type": "frame", "id": "a-mine", "layout": "vertical",
              "children": [ { "type": "text", "id": "a4", "content": "我的" } ] }
          ] }
      ] },
    { "type": "frame", "id": "s2", "name": "星图", "screen": "/screen-1",
      "width": 375, "height": 812, "layout": "vertical" }
] }"##;

#[test]
fn tabs_naming_no_screen_are_echoed_once_for_the_document() {
    // "wrong worse than dead": these two tabs must NOT be bound to an
    // existing route, but the model is the only one who can decide whether
    // the missing screens should exist — so it is told.
    let state = state_from_json(CJK_FOUR_TABS_TWO_SCREENS);
    let issues = scan_nav_issues(&state);

    let orphan: Vec<&String> = issues
        .iter()
        .filter(|line| line.contains("name no screen"))
        .collect();
    assert_eq!(
        orphan.len(),
        1,
        "one line for the whole document: {issues:?}"
    );
    assert!(orphan[0].contains("地点"), "{orphan:?}");
    assert!(orphan[0].contains("我的"), "{orphan:?}");
    assert!(
        !orphan[0].contains("星图") && !orphan[0].contains("今夜"),
        "tabs that DO have a screen are not orphans: {orphan:?}"
    );
    // …and the tabs that do have screens still get their bind suggestion.
    assert!(issues.iter().any(|l| l.contains("a-starmap")), "{issues:?}");
}

#[test]
fn a_fully_covered_tab_bar_reports_no_orphans() {
    let state = state_from_json(TWO_SCREENS_UNBOUND_PROFILE_TAB);
    let issues = scan_nav_issues(&state);
    assert!(
        issues.iter().all(|line| !line.contains("name no screen")),
        "{issues:?}"
    );
}
