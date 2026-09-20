//! Tests for Track A's deterministic screen/nav wiring pass.

use super::*;
use jian_ops_schema::PenDocument;
use op_editor_core::EditorState;

fn state_from_json(json: &str) -> EditorState {
    let doc: PenDocument = serde_json::from_str(json).expect("valid PenDocument");
    EditorState::from_document(doc)
}

fn find_by_id<'a>(nodes: &'a [PenNode], id: &str) -> Option<&'a PenNode> {
    for node in nodes {
        if node.id_str() == id {
            return Some(node);
        }
        if let Some(children) = node.children() {
            if let Some(found) = find_by_id(children, id) {
                return Some(found);
            }
        }
    }
    None
}

fn collect_event_node_ids(nodes: &[PenNode], ids: &mut Vec<String>) {
    for node in nodes {
        if node_has_events(node) {
            ids.push(node.id_str().to_string());
        }
        if let Some(children) = node.children() {
            collect_event_node_ids(children, ids);
        }
    }
}

fn frame_screen(node: &PenNode) -> Option<&str> {
    match node {
        PenNode::Frame(f) => f.screen.as_deref(),
        _ => None,
    }
}

fn node_events_json(node: &PenNode) -> Option<serde_json::Value> {
    serde_json::to_value(node).ok()?.get("events").cloned()
}

fn run_pass(state: &mut EditorState) {
    let mut sink = crate::loop_finalize::StateDocSink { state };
    wire_screen_navigation(&mut sink);
}

// ── Pure-helper unit tests ─────────────────────────────────────────────

#[test]
fn normalize_slug_strips_non_ascii_and_hyphenates() {
    assert_eq!(normalize_slug("Profile Screen"), "profile-screen");
    assert_eq!(normalize_slug("  Settings!! "), "settings");
    assert_eq!(normalize_slug("首页"), ""); // all-CJK -> empty, caller falls back
}

#[test]
fn labels_match_exact_and_prefix() {
    assert!(labels_match("profile", "profile"));
    assert!(labels_match("profile", "profilescreen")); // "Profile" vs "Profile Screen"
    assert!(labels_match("profilescreen", "profile"));
    assert!(!labels_match("profile", "settings"));
    assert!(!labels_match("", "profile"));
}

/// Measured (`0718-1-glm-1.op`): screen names carry a brand prefix
/// ("Wander — Trips" / "Wander — Saved") — neither normalized form is a
/// prefix of the other ("trips" vs "wandertrips"), so every one of the
/// document's nav tabs (bare "Trips" / "Saved" labels) went unbound. The
/// token fallback must recover this WITHOUT opening the door to a bare
/// substring match.
#[test]
fn labels_match_handles_cjk_labels() {
    // `normalize_label` filtered on `is_ascii_alphanumeric`, so every CJK
    // label and screen name normalized to "" and NOTHING matched — the whole
    // tab-navigation layer silently no-opped on Chinese apps. Measured on
    // `0808-k3-2.op`: screen routes were written (that path never reads
    // labels) while not one of the eight tabs got an `onTap` action.
    assert!(labels_match("星图", "星图"));
    assert!(labels_match("今夜", "Nocturne 今夜"));
    assert!(labels_match("Nocturne 今夜", "今夜"));
    assert!(!labels_match("星图", "地点"));
    // Mixed scripts still tokenize on the usual separators.
    assert!(labels_match("设置", "Wander — 设置"));
    // …and a CJK label still never matches an unrelated ASCII one.
    assert!(!labels_match("星图", "Profile"));
}

#[test]
fn labels_match_token_fallback_for_brand_prefixed_names() {
    // The exact failure this was built from: brand-prefixed screen name,
    // bare tab label, either argument order.
    assert!(labels_match("Trips", "Wander — Trips"));
    assert!(labels_match("Wander — Trips", "Trips"));
    assert!(labels_match("Saved", "Wander — Saved"));

    // A bare SUBSTRING must never count as a token — "Roadtrips" tokenizes
    // to a single ["roadtrips"] token, which never equals "trips".
    assert!(!labels_match("Trips", "Roadtrips"));
    assert!(!labels_match("Trips", "Wander — Roadtrips"));

    // Multi-word tab label: "Your Library" should match a brand-prefixed
    // screen sharing either whole word as a token.
    assert!(labels_match("Your Library", "Wander — Library"));
    assert!(labels_match("Library", "Wander — Your Library"));

    // Still ambiguous when nothing at all is shared.
    assert!(!labels_match("Explore", "Wander — Trips"));
}

/// Contract point 1: the navigate body must be a Tier-1 string-LITERAL
/// expression source — a bare `/path` lexes as division and fails to
/// compile (`Expression::compile`). The correct wire form is exactly the
/// shape asserted by jian-core's own fixtures
/// (`jian-core/tests/action_navigation.rs::push_literal_path`,
/// `jian-ops-schema/src/events.rs::push_action_with_string_body`): the
/// JSON value under `"replace"`/`"push"` is the STRING `"\"/path\""` (i.e.
/// its decoded content is `"/path"`, quote characters included).
#[test]
fn navigate_patch_produces_expression_literal_body() {
    let patch = navigate_patch("replace", "/profile");
    assert_eq!(
        patch,
        r#"{"events":{"onTap":[{"replace":"\"/profile\""}]}}"#
    );
    // Round-trip through a real JSON parser to double-check the DECODED
    // action body equals the 10-char string `"/profile"` (quotes included) —
    // exactly what `jian_core::action::actions::navigation::expr_from_value`
    // feeds to `Expression::compile`.
    let v: serde_json::Value = serde_json::from_str(&patch).unwrap();
    let body = v["events"]["onTap"][0]["replace"].as_str().unwrap();
    assert_eq!(body, "\"/profile\"");
}

// ── Gate ────────────────────────────────────────────────────────────────

#[test]
fn single_screen_is_untouched_zero_new_keys() {
    let json = r#"{"version":"1.0","children":[
        {"type":"frame","id":"home","name":"Home","width":390,"height":844,
         "layout":"vertical","children":[]}
    ]}"#;
    let mut state = state_from_json(json);
    let before = serde_json::to_string(state.active_children()).unwrap();
    run_pass(&mut state);
    let after = serde_json::to_string(state.active_children()).unwrap();
    assert_eq!(
        after, before,
        "gate: <2 screen-shaped frames must be a no-op"
    );
    assert!(!after.contains("screen"), "no new `screen` key grown");
}

// ── Screen marking ──────────────────────────────────────────────────────

#[test]
fn two_screens_get_entry_and_slug_path() {
    let json = r#"{"version":"1.0","children":[
        {"type":"frame","id":"home","name":"Home","width":390,"height":844,
         "layout":"vertical","children":[]},
        {"type":"frame","id":"profile","name":"Profile","width":390,"height":844,
         "layout":"vertical","children":[]}
    ]}"#;
    let mut state = state_from_json(json);
    run_pass(&mut state);
    let home = find_by_id(state.active_children(), "home").unwrap();
    let profile = find_by_id(state.active_children(), "profile").unwrap();
    assert_eq!(frame_screen(home), Some("/"));
    assert_eq!(frame_screen(profile), Some("/profile"));
}

#[test]
fn entry_prefers_home_like_name_over_doc_order() {
    // "Profile" is first in document order, but "Dashboard" carries an
    // entry-name hint — Dashboard must win "/" regardless of position.
    let json = r#"{"version":"1.0","children":[
        {"type":"frame","id":"profile","name":"Profile","width":390,"height":844,
         "layout":"vertical","children":[]},
        {"type":"frame","id":"dash","name":"Dashboard","width":1200,"height":900,
         "layout":"vertical","children":[]}
    ]}"#;
    let mut state = state_from_json(json);
    run_pass(&mut state);
    let profile = find_by_id(state.active_children(), "profile").unwrap();
    let dash = find_by_id(state.active_children(), "dash").unwrap();
    assert_eq!(frame_screen(dash), Some("/"));
    assert_eq!(frame_screen(profile), Some("/profile"));
}

#[test]
fn duplicate_slugs_get_numeric_suffix() {
    let json = r#"{"version":"1.0","children":[
        {"type":"frame","id":"home","name":"Home","width":390,"height":844,
         "layout":"vertical","children":[]},
        {"type":"frame","id":"settings-a","name":"Settings","width":390,"height":844,
         "layout":"vertical","children":[]},
        {"type":"frame","id":"settings-b","name":"Settings","width":390,"height":844,
         "layout":"vertical","children":[]}
    ]}"#;
    let mut state = state_from_json(json);
    run_pass(&mut state);
    let a = find_by_id(state.active_children(), "settings-a").unwrap();
    let b = find_by_id(state.active_children(), "settings-b").unwrap();
    // Doc-order first gets the bare slug, the second gets the `-2` suffix.
    assert_eq!(frame_screen(a), Some("/settings"));
    assert_eq!(frame_screen(b), Some("/settings-2"));
}

#[test]
fn prompt_inventory_matches_the_routes_the_pass_persists() {
    let json = r#"{"version":"1.0","children":[
        {"type":"frame","id":"authored","name":"Checkout","width":390,"height":844,
         "layout":"vertical","children":[],"screen":"/buy-now"},
        {"type":"frame","id":"home","name":"Home","width":390,"height":844,
         "layout":"vertical","children":[]},
        {"type":"frame","id":"settings-a","name":"Settings","width":390,"height":844,
         "layout":"vertical","children":[]},
        {"type":"frame","id":"settings-b","name":"Settings","width":390,"height":844,
         "layout":"vertical","children":[]}
    ]}"#;
    let mut state = state_from_json(json);
    let inventory = screen_route_inventory::screen_route_inventory(&state);
    run_pass(&mut state);
    let persisted = state
        .active_children()
        .iter()
        .filter_map(|node| {
            let PenNode::Frame(frame) = node else {
                return None;
            };
            Some((
                frame.base.name.clone().unwrap_or_default(),
                frame.screen.clone()?,
            ))
        })
        .collect::<Vec<_>>();
    assert_eq!(inventory, persisted);
    assert_eq!(
        inventory,
        vec![
            ("Checkout".into(), "/buy-now".into()),
            ("Home".into(), "/".into()),
            ("Settings".into(), "/settings".into()),
            ("Settings".into(), "/settings-2".into()),
        ]
    );
}

#[test]
fn planned_inventory_merges_existing_entry_and_persists_midwidth_routes() {
    let json = r#"{"version":"1.0","children":[
        {"type":"frame","id":"home","name":"Home","screen":"/",
         "width":390,"height":844,"layout":"vertical","children":[]},
        {"type":"frame","id":"search","name":"Search",
         "width":768,"height":1024,"layout":"vertical","children":[]},
        {"type":"frame","id":"profile","name":"Profile",
         "width":768,"height":1024,"layout":"vertical","children":[]}
    ]}"#;
    let mut state = state_from_json(json);
    let plan: crate::plan::OrchestratorPlan = serde_json::from_value(serde_json::json!({
        "rootFrame": {
            "id": "root",
            "name": "App",
            "width": 768,
            "height": 1024
        },
        "subtasks": [
            {
                "id": "search-task",
                "label": "Search",
                "screen": "Search",
                "parentFrameId": "search",
                "region": {"width": 768, "height": 400}
            },
            {
                "id": "profile-task",
                "label": "Profile",
                "screen": "Profile",
                "parentFrameId": "profile",
                "region": {"width": 768, "height": 400}
            }
        ]
    }))
    .unwrap();

    let planned = prompt_screen_route_inventory(&plan, &state);
    assert_eq!(
        planned,
        vec![
            ("Home".into(), "/".into()),
            ("Search".into(), "/search".into()),
            ("Profile".into(), "/profile".into()),
        ]
    );

    {
        let mut sink = crate::loop_finalize::StateDocSink { state: &mut state };
        ensure_planned_screen_routes(&mut sink, &plan);
    }
    assert_eq!(prompt_screen_route_inventory(&plan, &state), planned);
    assert_eq!(
        ["home", "search", "profile"]
            .into_iter()
            .map(|id| frame_screen(find_by_id(state.active_children(), id).unwrap()).unwrap())
            .collect::<Vec<_>>(),
        vec!["/", "/search", "/profile"]
    );
}

#[test]
fn authored_midwidth_entry_reserves_root_for_new_shaped_screen() {
    let json = r#"{"version":"1.0","children":[
        {"type":"frame","id":"tablet-home","name":"Home","screen":"/",
         "width":768,"height":1024,"layout":"vertical","children":[]},
        {"type":"frame","id":"detail","name":"Detail",
         "width":390,"height":844,"layout":"vertical","children":[]}
    ]}"#;
    let mut state = state_from_json(json);
    assert_eq!(
        screen_route_inventory::screen_route_inventory(&state),
        vec![
            ("Home".into(), "/".into()),
            ("Detail".into(), "/detail".into())
        ]
    );

    run_pass(&mut state);

    assert_eq!(
        frame_screen(find_by_id(state.active_children(), "detail").unwrap()),
        Some("/detail")
    );
}

#[test]
fn collapsed_plan_groups_fall_back_to_the_live_route_inventory() {
    let state = state_from_json(
        r#"{"version":"1.0","children":[
            {"type":"frame","id":"home","name":"Home","screen":"/",
             "width":390,"height":844,"children":[]},
            {"type":"frame","id":"detail","name":"Detail","screen":"/detail",
             "width":390,"height":844,"children":[]}
        ]}"#,
    );
    let plan: crate::plan::OrchestratorPlan = serde_json::from_value(serde_json::json!({
        "rootFrame": {"id": "root", "name": "App", "width": 390, "height": 844},
        "subtasks": [
            {
                "id": "search-task",
                "label": "Search",
                "screen": "Search",
                "parentFrameId": "home",
                "region": {"width": 390, "height": 300}
            },
            {
                "id": "profile-task",
                "label": "Profile",
                "screen": "Profile",
                "parentFrameId": "home",
                "region": {"width": 390, "height": 300}
            }
        ]
    }))
    .unwrap();

    assert_eq!(
        prompt_screen_route_inventory(&plan, &state),
        vec![
            ("Home".into(), "/".into()),
            ("Detail".into(), "/detail".into())
        ]
    );
}

#[test]
fn authored_screen_marker_is_never_overwritten_and_not_reused_as_entry() {
    // "Checkout" already carries an authored (non-"/") marker. Since the
    // pass never touches authored markers even to satisfy the single-entry
    // rule, "/" is picked only among the UNMARKED candidates — here that's
    // "Extras", the only other one, despite its name matching no entry hint.
    let json = r#"{"version":"1.0","children":[
        {"type":"frame","id":"checkout","name":"Checkout","width":390,"height":844,
         "layout":"vertical","children":[],"screen":"/checkout"},
        {"type":"frame","id":"extras","name":"Extras","width":390,"height":844,
         "layout":"vertical","children":[]}
    ]}"#;
    let mut state = state_from_json(json);
    run_pass(&mut state);
    let checkout = find_by_id(state.active_children(), "checkout").unwrap();
    let extras = find_by_id(state.active_children(), "extras").unwrap();
    assert_eq!(
        frame_screen(checkout),
        Some("/checkout"),
        "authored marker untouched"
    );
    assert_eq!(
        frame_screen(extras),
        Some("/"),
        "sole unmarked candidate becomes entry"
    );
}

#[test]
fn second_run_is_idempotent() {
    let json = r#"{"version":"1.0","children":[
        {"type":"frame","id":"home","name":"Home","width":390,"height":844,
         "layout":"vertical","children":[
            {"type":"frame","id":"nav-home","name":"Bottom Nav","role":"bottom-tab-bar",
             "width":"fill_container","height":56,"layout":"horizontal","children":[
                {"type":"frame","id":"tab-home","name":"HomeTab","width":80,"height":40,
                 "children":[{"type":"text","id":"tab-home-lbl","content":"Home"}]},
                {"type":"frame","id":"tab-profile","name":"ProfileTab","width":80,"height":40,
                 "children":[{"type":"text","id":"tab-profile-lbl","content":"Profile"}]}
             ]}
         ]},
        {"type":"frame","id":"profile","name":"Profile","width":390,"height":844,
         "layout":"vertical","children":[]}
    ]}"#;
    let mut state = state_from_json(json);
    run_pass(&mut state);
    let once = serde_json::to_string(state.active_children()).unwrap();
    run_pass(&mut state);
    let twice = serde_json::to_string(state.active_children()).unwrap();
    assert_eq!(
        once, twice,
        "running the pass twice must be a no-op the second time"
    );
}

// ── Navbar wiring ───────────────────────────────────────────────────────

fn two_screen_bottom_nav_doc() -> &'static str {
    r#"{"version":"1.0","children":[
        {"type":"frame","id":"home","name":"Home","width":390,"height":844,
         "layout":"vertical","children":[
            {"type":"frame","id":"nav-home","name":"Bottom Nav","role":"bottom-tab-bar",
             "width":"fill_container","height":56,"layout":"horizontal","children":[
                {"type":"frame","id":"tab-home-in-home","name":"HomeTab","width":80,"height":40,
                 "children":[{"type":"text","id":"t1","content":"Home"}]},
                {"type":"frame","id":"tab-profile-in-home","name":"ProfileTab","width":80,"height":40,
                 "children":[{"type":"text","id":"t2","content":"Profile Screen"}]},
                {"type":"frame","id":"tab-settings-in-home","name":"SettingsTab","width":80,"height":40,
                 "children":[{"type":"text","id":"t3","content":"Settings"}]}
             ]}
         ]},
        {"type":"frame","id":"profile","name":"Profile","width":390,"height":844,
         "layout":"vertical","children":[
            {"type":"frame","id":"nav-profile","name":"Bottom Nav","role":"bottom-tab-bar",
             "width":"fill_container","height":56,"layout":"horizontal","children":[
                {"type":"frame","id":"tab-home-in-profile","name":"HomeTab","width":80,"height":40,
                 "children":[{"type":"text","id":"t4","content":"Home"}]},
                {"type":"frame","id":"tab-profile-in-profile","name":"ProfileTab","width":80,"height":40,
                 "children":[{"type":"text","id":"t5","content":"Profile Screen"}]}
             ]}
         ]}
    ]}"#
}

#[test]
fn bottom_nav_tabs_wire_to_matching_screens_including_self() {
    let mut state = state_from_json(two_screen_bottom_nav_doc());
    run_pass(&mut state);

    let home_path = frame_screen(find_by_id(state.active_children(), "home").unwrap())
        .unwrap()
        .to_string();
    let profile_path = frame_screen(find_by_id(state.active_children(), "profile").unwrap())
        .unwrap()
        .to_string();

    // Home screen's own navbar: Home tab points at itself, Profile tab at Profile.
    let home_tab_in_home = find_by_id(state.active_children(), "tab-home-in-home").unwrap();
    let profile_tab_in_home = find_by_id(state.active_children(), "tab-profile-in-home").unwrap();
    assert_eq!(
        node_events_json(home_tab_in_home).unwrap()["onTap"][0]["replace"],
        serde_json::json!(format!("\"{home_path}\""))
    );
    assert_eq!(
        node_events_json(profile_tab_in_home).unwrap()["onTap"][0]["replace"],
        serde_json::json!(format!("\"{profile_path}\""))
    );

    // Profile screen's own navbar wires the same way independently.
    let home_tab_in_profile = find_by_id(state.active_children(), "tab-home-in-profile").unwrap();
    let profile_tab_in_profile =
        find_by_id(state.active_children(), "tab-profile-in-profile").unwrap();
    assert_eq!(
        node_events_json(home_tab_in_profile).unwrap()["onTap"][0]["replace"],
        serde_json::json!(format!("\"{home_path}\""))
    );
    assert_eq!(
        node_events_json(profile_tab_in_profile).unwrap()["onTap"][0]["replace"],
        serde_json::json!(format!("\"{profile_path}\""))
    );
}

/// Real generated mobile navs commonly use a surface wrapper around the
/// actual four-item tab row. Both layers may carry a nav role, but only the
/// four tab-item frames are interactive targets. The inner row's first text
/// descendant (`Trips`) must not make the row itself look like a Trips tab.
fn nested_four_tab_bottom_nav_doc() -> &'static str {
    r#"{"version":"1.0","children":[
        {"type":"frame","id":"trips","name":"Trips","width":390,"height":844,
         "layout":"vertical","children":[
            {"type":"frame","id":"nav-shell","name":"Bottom Navigation Surface","role":"bottom-tab-bar",
             "width":"fill_container","height":84,"layout":"vertical","children":[
                {"type":"frame","id":"tabs-row","name":"Bottom Tab Row","role":"bottom-tab-bar",
                 "width":"fill_container","height":72,"layout":"horizontal","children":[
                    {"type":"frame","id":"tab-trips","name":"Trips Tab","width":80,"height":56,
                     "children":[{"type":"text","id":"label-trips","content":"Trips"}]},
                    {"type":"frame","id":"tab-explore","name":"Explore Tab","width":80,"height":56,
                     "children":[{"type":"text","id":"label-explore","content":"Explore"}]},
                    {"type":"frame","id":"tab-saved","name":"Saved Tab","width":80,"height":56,
                     "children":[{"type":"text","id":"label-saved","content":"Saved"}]},
                    {"type":"frame","id":"tab-profile","name":"Profile Tab","width":80,"height":56,
                     "children":[{"type":"text","id":"label-profile","content":"Profile"}]}
                 ]}
             ]}
         ]},
        {"type":"frame","id":"explore","name":"Explore","width":390,"height":844,
         "layout":"vertical","children":[]},
        {"type":"frame","id":"saved","name":"Saved","width":390,"height":844,
         "layout":"vertical","children":[]},
        {"type":"frame","id":"profile","name":"Profile","width":390,"height":844,
         "layout":"vertical","children":[]}
    ]}"#
}

#[test]
fn nested_bottom_nav_wires_only_real_tab_items() {
    let mut state = state_from_json(nested_four_tab_bottom_nav_doc());
    run_pass(&mut state);

    let inner_row = find_by_id(state.active_children(), "tabs-row").unwrap();
    assert!(
        node_events_json(inner_row).is_none(),
        "the inner tab-row wrapper must not inherit the first tab's Trips navigation"
    );

    let mut event_node_ids = Vec::new();
    collect_event_node_ids(state.active_children(), &mut event_node_ids);
    event_node_ids.sort();
    assert_eq!(
        event_node_ids,
        vec!["tab-explore", "tab-profile", "tab-saved", "tab-trips"],
        "only the four real tab-item frames should receive navigation events"
    );
}

#[test]
fn explicit_tab_row_beats_a_larger_labeled_content_group() {
    let mut state = state_from_json(
        r#"{"version":"1.0","children":[
          {"type":"frame","id":"trips","name":"Trips","width":390,"height":844,"children":[
            {"type":"frame","id":"nav-shell","name":"Bottom Navigation","role":"bottom-tab-bar","children":[
              {"type":"frame","id":"tabs-row","name":"Tab Bar","role":"tab-row","children":[
                {"type":"frame","id":"tab-trips","children":[{"type":"text","id":"t1","content":"Trips"}]},
                {"type":"frame","id":"tab-explore","children":[{"type":"text","id":"t2","content":"Explore"}]},
                {"type":"frame","id":"tab-saved","children":[{"type":"text","id":"t3","content":"Saved"}]},
                {"type":"frame","id":"tab-profile","children":[{"type":"text","id":"t4","content":"Profile"}]}
              ]},
              {"type":"frame","id":"larger-content-list","name":"Recommendations","children":[
                {"type":"frame","id":"card-trips","children":[{"type":"text","id":"c1","content":"Trips"}]},
                {"type":"frame","id":"card-explore","children":[{"type":"text","id":"c2","content":"Explore"}]},
                {"type":"frame","id":"card-saved","children":[{"type":"text","id":"c3","content":"Saved"}]},
                {"type":"frame","id":"card-profile","children":[{"type":"text","id":"c4","content":"Profile"}]},
                {"type":"frame","id":"card-settings","children":[{"type":"text","id":"c5","content":"Settings"}]}
              ]}
            ]}
          ]},
          {"type":"frame","id":"explore","name":"Explore","width":390,"height":844,"children":[]},
          {"type":"frame","id":"saved","name":"Saved","width":390,"height":844,"children":[]},
          {"type":"frame","id":"profile","name":"Profile","width":390,"height":844,"children":[]}
        ]}"#,
    );
    run_pass(&mut state);

    let mut event_node_ids = Vec::new();
    collect_event_node_ids(state.active_children(), &mut event_node_ids);
    event_node_ids.sort();
    assert_eq!(
        event_node_ids,
        vec!["tab-explore", "tab-profile", "tab-saved", "tab-trips"]
    );
}

#[test]
fn descendant_authored_tab_event_prevents_a_duplicate_root_binding() {
    let mut state = state_from_json(
        r#"{"version":"1.0","children":[
          {"type":"frame","id":"trips","name":"Trips","width":390,"height":844,"children":[
            {"type":"frame","id":"nav","role":"bottom-tab-bar","children":[
              {"type":"frame","id":"tab-trips","children":[
                {"type":"icon_font","id":"trips-action","iconFontName":"luggage","events":{"onTap":[{"replace":"\"/\""}]}},
                {"type":"text","id":"trips-label","content":"Trips"}
              ]},
              {"type":"frame","id":"tab-explore","children":[{"type":"text","id":"e-label","content":"Explore"}]},
              {"type":"frame","id":"tab-saved","children":[{"type":"text","id":"s-label","content":"Saved"}]},
              {"type":"frame","id":"tab-profile","children":[{"type":"text","id":"p-label","content":"Profile"}]}
            ]}
          ]},
          {"type":"frame","id":"explore","name":"Explore","width":390,"height":844,"children":[]},
          {"type":"frame","id":"saved","name":"Saved","width":390,"height":844,"children":[]},
          {"type":"frame","id":"profile","name":"Profile","width":390,"height":844,"children":[]}
        ]}"#,
    );
    run_pass(&mut state);

    assert!(node_events_json(find_by_id(state.active_children(), "tab-trips").unwrap()).is_none());
    assert!(
        node_events_json(find_by_id(state.active_children(), "trips-action").unwrap()).is_some()
    );
    assert!(
        node_events_json(find_by_id(state.active_children(), "tab-explore").unwrap()).is_some()
    );
}

#[test]
fn public_nav_collector_keeps_icon_only_nav_visible_for_audits() {
    let state = state_from_json(
        r#"{"version":"1.0","children":[
          {"type":"frame","id":"root","children":[
            {"type":"frame","id":"icon-only-nav","role":"bottom-tab-bar","children":[
              {"type":"icon_font","id":"i1","iconFontName":"home"},
              {"type":"icon_font","id":"i2","iconFontName":"search"}
            ]}
          ]}
        ]}"#,
    );
    let root = find_by_id(state.active_children(), "root").unwrap();
    let mut navs = Vec::new();
    collect_nav_containers(root, &mut navs);
    assert_eq!(navs.len(), 1);
    assert_eq!(navs[0].id_str(), "icon-only-nav");
}

#[test]
fn mismatched_tab_label_is_not_bound() {
    let mut state = state_from_json(two_screen_bottom_nav_doc());
    run_pass(&mut state);
    // "Settings" has no matching screen (only Home/Profile exist) — its tab
    // must stay unbound rather than guess.
    let settings_tab = find_by_id(state.active_children(), "tab-settings-in-home").unwrap();
    assert!(node_events_json(settings_tab).is_none());
}

/// De-identified reproduction of `0718-1-glm-1.op`: three screens already
/// spread apart (x = 0 / 430 / 860, non-overlapping), each brand-prefixed
/// ("Wander — Trips" etc.), one bottom nav with bare tab labels ("Trips" /
/// "Saved" / "Explore" / "Profile" — the last two have no matching screen
/// in this 3-screen document, matching the real file). Before the token
/// fallback this bound ZERO tabs.
fn brand_prefixed_three_screen_doc() -> &'static str {
    r#"{"version":"1.0","children":[
        {"type":"frame","id":"trips","name":"Wander — Trips","x":0,"y":0,"width":390,"height":844,
         "layout":"vertical","children":[
            {"type":"frame","id":"nav-trips","name":"Tab Bar","role":"bottom-tab-bar",
             "width":"fill_container","height":72,"layout":"horizontal","children":[
                {"type":"frame","id":"tab-trips","width":80,"height":40,
                 "children":[{"type":"text","id":"t1","content":"Trips"}]},
                {"type":"frame","id":"tab-saved","width":80,"height":40,
                 "children":[{"type":"text","id":"t2","content":"Saved"}]},
                {"type":"frame","id":"tab-explore","width":80,"height":40,
                 "children":[{"type":"text","id":"t3","content":"Explore"}]},
                {"type":"frame","id":"tab-profile","width":80,"height":40,
                 "children":[{"type":"text","id":"t4","content":"Profile"}]}
             ]}
         ]},
        {"type":"frame","id":"destination","name":"Wander — Destination","x":430,"y":0,"width":390,"height":844,
         "layout":"vertical","children":[]},
        {"type":"frame","id":"saved","name":"Wander — Saved","x":860,"y":0,"width":390,"height":844,
         "layout":"vertical","children":[]}
    ]}"#
}

#[test]
fn brand_prefixed_screen_names_get_their_matching_tabs_bound() {
    let mut state = state_from_json(brand_prefixed_three_screen_doc());
    run_pass(&mut state);

    let trips_path = frame_screen(find_by_id(state.active_children(), "trips").unwrap())
        .unwrap()
        .to_string();
    let saved_path = frame_screen(find_by_id(state.active_children(), "saved").unwrap())
        .unwrap()
        .to_string();

    let trips_tab = find_by_id(state.active_children(), "tab-trips").unwrap();
    assert_eq!(
        node_events_json(trips_tab).unwrap()["onTap"][0]["replace"],
        serde_json::json!(format!("\"{trips_path}\"")),
        "\"Trips\" must bind to \"Wander — Trips\" via the token fallback"
    );
    let saved_tab = find_by_id(state.active_children(), "tab-saved").unwrap();
    assert_eq!(
        node_events_json(saved_tab).unwrap()["onTap"][0]["replace"],
        serde_json::json!(format!("\"{saved_path}\"")),
        "\"Saved\" must bind to \"Wander — Saved\" via the token fallback"
    );

    // "Explore" / "Profile" have no matching screen in this 3-screen
    // document (same as the real file) — must stay unbound, not guess.
    let explore_tab = find_by_id(state.active_children(), "tab-explore").unwrap();
    assert!(node_events_json(explore_tab).is_none());
    let profile_tab = find_by_id(state.active_children(), "tab-profile").unwrap();
    assert!(node_events_json(profile_tab).is_none());

    // Destination (a push-style detail screen, no bottom nav of its own)
    // still gets tagged even though it has nothing to bind.
    assert!(frame_screen(find_by_id(state.active_children(), "destination").unwrap()).is_some());
}

#[test]
fn existing_tab_events_are_left_alone() {
    let json = r#"{"version":"1.0","children":[
        {"type":"frame","id":"home","name":"Home","width":390,"height":844,
         "layout":"vertical","children":[
            {"type":"frame","id":"nav-home","name":"Bottom Nav","role":"bottom-tab-bar",
             "width":"fill_container","height":56,"layout":"horizontal","children":[
                {"type":"frame","id":"tab-profile","name":"ProfileTab","width":80,"height":40,
                 "events":{"onTap":[{"custom_action":null}]},
                 "children":[{"type":"text","id":"t1","content":"Profile"}]}
             ]}
         ]},
        {"type":"frame","id":"profile","name":"Profile","width":390,"height":844,
         "layout":"vertical","children":[]}
    ]}"#;
    let mut state = state_from_json(json);
    let before = node_events_json(find_by_id(state.active_children(), "tab-profile").unwrap());
    run_pass(&mut state);
    let after = node_events_json(find_by_id(state.active_children(), "tab-profile").unwrap());
    assert_eq!(
        after, before,
        "a node with existing events is never touched"
    );
}

// ── Cleanup-only interaction boundary ──────────────────────────────────

#[test]
fn preview_fallback_does_not_create_a_temporary_back_binding() {
    let json = r#"{"version":"1.0","children":[
        {"type":"frame","id":"profile","name":"Profile","width":390,"height":844,
         "layout":"none","children":[
            {"type":"frame","id":"back","x":24,"y":80,"width":44,"height":44,
             "layout":"none","children":[
                {"type":"icon_font","id":"back-icon","x":12,"y":12,
                 "iconFontName":"arrow-left","width":20,"height":20}
             ]}
         ]},
        {"type":"frame","id":"home","name":"Home","width":390,"height":844,
         "layout":"vertical","children":[]}
    ]}"#;
    let mut state = state_from_json(json);
    run_pass(&mut state);
    assert!(
        node_events_json(find_by_id(state.active_children(), "back").unwrap()).is_none(),
        "public preview fallback may assign routes/nav, but cleanup-only backfill must write \
         interactions to the real document"
    );
}

#[test]
fn authored_back_binding_is_preserved_by_preview_fallback() {
    let json = r#"{"version":"1.0","children":[
        {"type":"frame","id":"profile","name":"Profile","width":390,"height":844,
         "layout":"none","children":[
            {"type":"frame","id":"back-btn","name":"Back Button","width":44,"height":44,
             "events":{"onTap":[{"pop":null}]},
             "children":[
                {"type":"icon_font","id":"inner-icon","name":"arrow-left","iconFontName":"arrow-left",
                 "width":24,"height":24}
             ]}
         ]},
        {"type":"frame","id":"home","name":"Home","width":390,"height":844,
         "layout":"vertical","children":[]}
    ]}"#;
    let mut state = state_from_json(json);
    let before = node_events_json(find_by_id(state.active_children(), "back-btn").unwrap());
    run_pass(&mut state);
    let after = node_events_json(find_by_id(state.active_children(), "back-btn").unwrap());
    assert_eq!(after, before);
    assert!(node_events_json(find_by_id(state.active_children(), "inner-icon").unwrap()).is_none());
}
