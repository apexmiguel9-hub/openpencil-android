//! Split from `unify_shared_nav_tests.rs` to stay under the 800-line cap —
//! the D (active-tab idempotency) fix + the detail-page Inject exemption,
//! both from the 0718-1-k3-1 postmortem. Reuses the original file's fixture
//! helpers (`pub(super)`) rather than duplicating them.

use super::tests::{
    find_by_id, first_fill, labels_of, nav_json, nav_with_active_indices, run_pass, screen_json,
    state_from, three_screen_missing_nav_doc, two_screen_drifted_doc, ACTIVE, INACTIVE,
};
use super::*;

// ---------------------------------------------------------------------
// D: active-tab idempotency gap (0718-1-k3-1 postmortem).
// The old label-only idempotency gate let a model that
// draws every screen's nav byte-for-byte identical (same active tab baked
// into all of them) used to short-circuit `resolve_target` into `None`
// before `retarget_active_tab` ever ran. These tests exercise the new
// `active_tab_is_correct` check + `SyncTarget::RetargetActiveOnly` path.
// ---------------------------------------------------------------------

fn two_screen_doc_identical_nav(active_index_on_library: usize) -> serde_json::Value {
    serde_json::json!({
        "version": "1.0",
        "children": [
            screen_json("home", "Home", nav_with_active_indices("nav-home", "h", &[0])),
            // Byte-identical LABEL set to Home's nav (no drift at all) —
            // reproduces the 0718-1-k3-1 shape where every screen's nav was
            // literally the same authored content.
            screen_json(
                "library",
                "Library",
                nav_with_active_indices("nav-library", "l", &[active_index_on_library]),
            ),
        ]
    })
}

#[test]
fn labels_match_but_active_wrong_only_retargets_active_styling_in_place() {
    // Library's own nav bakes "Home" (index 0) active — wrong for the
    // Library screen, but same label set as the reference, so the OLD
    // label-only gate would have skipped it entirely.
    let mut state = state_from(two_screen_doc_identical_nav(0));
    run_pass(&mut state);

    let library_root = find_by_id(state.active_children(), "library").unwrap();
    // NB: `ReplaceSubtree` always mints a fresh id for the node it replaces
    // (see `cleanup::apply_root_transform`'s own doc) regardless of which
    // `SyncTarget` branch drove it — "nav-library" itself is expected to be
    // gone either way, so identity is tracked by structure (role), not id,
    // same as every other test in this file.
    let mut navs = Vec::new();
    collect_nav_containers(library_root, &mut navs);
    let library_nav = navs.first().expect("library screen still has a nav");

    assert_eq!(
        labels_of(library_nav),
        vec!["Home", "Search", "Library", "Premium"]
    );
    let tabs = library_nav.children().unwrap();
    assert_eq!(
        first_fill(&tabs[0]).unwrap(),
        INACTIVE,
        "Home must no longer read active on the Library screen"
    );
    assert_eq!(
        first_fill(&tabs[2]).unwrap(),
        ACTIVE,
        "Library must read active on its own screen"
    );
}

#[test]
fn labels_and_active_both_already_correct_is_truly_idempotent() {
    // Library's own nav already bakes "Library" (index 2) active — same
    // labels AND already-correct active tab: nothing for this pass to do.
    let mut state = state_from(two_screen_doc_identical_nav(2));
    let before = serde_json::to_string(state.active_children()).unwrap();
    run_pass(&mut state);
    let after = serde_json::to_string(state.active_children()).unwrap();
    assert_eq!(
        before, after,
        "labels match AND active tab is already correct — zero mutation"
    );
}

#[test]
fn drifted_labels_still_take_the_replace_path_with_correct_active_tab() {
    // Regression against the D fix: a genuinely drifted label set (the
    // pre-existing `two_screen_drifted_doc` fixture) can ONLY reach
    // `resolve_target`'s `Replace` arm by construction — `RetargetActiveOnly`
    // is reachable only when complete shared identity already matches — so
    // this fixture alone proves the Replace path, no id inspection needed.
    let mut state = state_from(two_screen_drifted_doc());
    run_pass(&mut state);

    let library_root = find_by_id(state.active_children(), "library").unwrap();
    let mut navs = Vec::new();
    collect_nav_containers(library_root, &mut navs);
    let tabs = navs.first().unwrap().children().unwrap();
    assert_eq!(
        first_fill(&tabs[2]).unwrap(),
        ACTIVE,
        "Library tab active after Replace"
    );
    assert_eq!(first_fill(&tabs[0]).unwrap(), INACTIVE);
}

#[test]
fn inject_path_still_activates_the_correct_tab_after_the_retarget_active_only_addition() {
    // Regression against the D fix: a nav-less screen eligible for
    // injection must still land with the RIGHT tab active — unaffected by
    // the new `RetargetActiveOnly` branch living alongside `Inject`.
    let mut state = state_from(three_screen_missing_nav_doc());
    run_pass(&mut state);

    let library_root = find_by_id(state.active_children(), "library").unwrap();
    let mut navs = Vec::new();
    collect_nav_containers(library_root, &mut navs);
    let tabs = navs.first().expect("nav injected").children().unwrap();
    assert_eq!(
        first_fill(&tabs[2]).unwrap(),
        ACTIVE,
        "Library active after Inject"
    );
    assert_eq!(
        first_fill(&tabs[0]).unwrap(),
        INACTIVE,
        "Home inactive after Inject"
    );
}

/// 0718-1-k3-1 real-structure regression (de-identified vocabulary, same
/// shape): three screens, every one authored the SAME nav byte-for-byte
/// (the first tab baked active on ALL three — the real file's "Trips" tab
/// lighting up on Trips/Destination/Saved alike). Before this fix,
/// the old label-only identity gate read every screen as already-unified, so
/// `retarget_active_tab` never ran on any of them.
#[test]
fn three_screen_byte_identical_nav_regression_each_screen_ends_up_with_its_own_active_tab() {
    let nav_for =
        |id_prefix: &str| nav_with_active_indices(&format!("nav-{id_prefix}"), id_prefix, &[0]);
    let doc = serde_json::json!({
        "version": "1.0",
        "children": [
            screen_json("home", "Home", nav_for("h")),
            screen_json("search", "Search", nav_for("se")),
            screen_json("library", "Library", nav_for("li")),
        ]
    });
    let mut state = state_from(doc);
    run_pass(&mut state);

    // Home is the reference screen (document-order first with a nav) — its
    // own nav is left untouched, and Home correctly stays active there.
    let home_root = find_by_id(state.active_children(), "home").unwrap();
    let home_nav = find_by_id(home_root.children().unwrap(), "nav-h").unwrap();
    assert_eq!(
        first_fill(&home_nav.children().unwrap()[0]).unwrap(),
        ACTIVE
    );

    // Search and Library both inherited "Home active" verbatim pre-fix —
    // after the fix each must show ITS OWN tab active instead.
    for (screen_id, active_tab_index) in [("search", 1usize), ("library", 2usize)] {
        let root = find_by_id(state.active_children(), screen_id).unwrap();
        let mut navs = Vec::new();
        collect_nav_containers(root, &mut navs);
        let tabs = navs.first().unwrap().children().unwrap();
        for (i, tab) in tabs.iter().enumerate() {
            let expected = if i == active_tab_index {
                ACTIVE
            } else {
                INACTIVE
            };
            assert_eq!(
                first_fill(tab).unwrap(),
                expected,
                "screen `{screen_id}` tab index {i}"
            );
        }
    }
}

// ---------------------------------------------------------------------
// Nested outer -> inner bottom-nav regression (0722-1-gem postmortem).
// The real generated shape wraps the tab row in a full-width outer chrome
// surface (divider/background/padding). Both layers are nav-shaped, so the
// old pre-order `.next()` lookup selected the OUTER node for comparison.
// That node's direct children are divider + inner row, not tabs: labels
// therefore collapsed to the first nested label and glyph/metric drift was
// mistaken for an already-unified nav.
// ---------------------------------------------------------------------

fn nested_nav_tab_json(
    id_prefix: &str,
    label: &str,
    icon: &str,
    icon_size: f64,
    route: &str,
    active: bool,
) -> serde_json::Value {
    let color = if active { ACTIVE } else { INACTIVE };
    let mut children = vec![
        serde_json::json!({
            "type": "icon_font", "id": format!("{id_prefix}-icon"),
            "iconFontName": icon, "width": icon_size, "height": icon_size,
            "fill": [{"type":"solid", "color": color}]
        }),
        serde_json::json!({
            "type": "text", "id": format!("{id_prefix}-label"), "content": label,
            "fontSize": 12, "fill": [{"type":"solid", "color": color}]
        }),
    ];
    if active {
        children.push(serde_json::json!({
            "type": "ellipse", "id": format!("{id_prefix}-indicator"),
            "width": 4, "height": 4, "fill": [{"type":"solid", "color": color}]
        }));
    }
    serde_json::json!({
        "type": "frame", "id": format!("{id_prefix}-tab"),
        "width": "fill_container", "height": 56, "layout": "vertical",
        "padding": [4, 0],
        "events": {"onTap": [{"replace": format!("\"{route}\"")}]},
        "children": children
    })
}

struct NestedNavStyle<'a> {
    icon_size: f64,
    outer_height: f64,
    outer_padding: [f64; 4],
    outer_fill: &'a str,
    divider_fill: &'a str,
    inner_padding: [f64; 4],
    inner_gap: f64,
}

fn nested_nav_json(
    id_prefix: &str,
    active_label: &str,
    icons: [&str; 4],
    routes: [&str; 4],
    style: NestedNavStyle<'_>,
) -> serde_json::Value {
    let labels = ["Trips", "Explore", "Saved", "Profile"];
    let tabs = labels
        .iter()
        .zip(icons)
        .zip(routes)
        .enumerate()
        .map(|(index, ((label, icon), route))| {
            nested_nav_tab_json(
                &format!("{id_prefix}-{index}"),
                label,
                icon,
                style.icon_size,
                route,
                *label == active_label,
            )
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "type": "frame", "id": format!("{id_prefix}-outer"),
        "name": "Bottom Navigation Bar", "role": "bottom-tab-bar",
        "width": "fill_container", "height": style.outer_height, "layout": "vertical",
        "padding": style.outer_padding,
        "fill": [{"type":"solid", "color": style.outer_fill}],
        "children": [
            {
                "type": "rectangle", "id": format!("{id_prefix}-divider"),
                "name": "Nav Divider", "width": "fill_container", "height": 1,
                "fill": [{"type":"solid", "color": style.divider_fill}]
            },
            {
                "type": "frame", "id": format!("{id_prefix}-inner"),
                "name": "Tab Bar", "role": "tab-row", "width": "fill_container",
                "height": 64, "layout": "horizontal", "gap": style.inner_gap,
                "padding": style.inner_padding, "children": tabs
            }
        ]
    })
}

fn two_screen_nested_nav_chrome_drift_doc() -> serde_json::Value {
    serde_json::json!({
        "version": "1.0",
        "children": [
            screen_json(
                "trips",
                "Trips",
                nested_nav_json(
                    "reference",
                    "Trips",
                    ["luggage", "compass", "bookmark", "user"],
                    ["/", "/explore", "/saved", "/profile"],
                    NestedNavStyle {
                        icon_size: 22.0,
                        outer_height: 88.0,
                        outer_padding: [8.0, 16.0, 8.0, 16.0],
                        outer_fill: "#FFFDFC",
                        divider_fill: "#E7E2DE",
                        inner_padding: [0.0, 4.0, 0.0, 4.0],
                        inner_gap: 8.0,
                    },
                ),
            ),
            // Labels and active destination are already correct, but this
            // independently redrawn nav drifted in glyphs AND full-surface
            // geometry. A label-only idempotency gate must not preserve it.
            screen_json(
                "saved",
                "Saved",
                nested_nav_json(
                    "drifted",
                    "Saved",
                    ["compass", "search", "heart", "user"],
                    [
                        "/target-trips",
                        "/target-explore",
                        "/target-saved",
                        "/target-profile",
                    ],
                    NestedNavStyle {
                        icon_size: 20.0,
                        outer_height: 76.0,
                        outer_padding: [0.0, 24.0, 0.0, 24.0],
                        outer_fill: "#F4F4F5",
                        divider_fill: "#FF00AA",
                        inner_padding: [0.0, 12.0, 0.0, 12.0],
                        inner_gap: 20.0,
                    },
                ),
            ),
        ]
    })
}

fn first_icon_name(node: &PenNode) -> Option<&str> {
    if let PenNode::IconFont(icon) = node {
        return Some(icon.icon_font_name.as_str());
    }
    node.children()?.iter().find_map(first_icon_name)
}

fn assert_saved_nested_nav_matches_reference(state: &op_editor_core::EditorState) {
    let saved_root = find_by_id(state.active_children(), "saved").expect("Saved screen");
    let outer = saved_root
        .children()
        .and_then(|children| children.first())
        .expect("Saved keeps one complete outer nav surface");
    let outer_json = serde_json::to_value(outer).expect("outer nav serializes");
    assert_eq!(outer_json["height"], serde_json::json!(88.0));
    assert_eq!(outer_json["fill"][0]["color"], serde_json::json!("#FFFDFC"));
    assert_eq!(
        outer_json["padding"],
        serde_json::json!([8.0, 16.0, 8.0, 16.0]),
        "outer surface padding must come from the reference nav, not just the tab row"
    );

    let outer_children = outer.children().expect("divider + inner tab row");
    assert_eq!(outer_children.len(), 2, "copy the complete outer chrome");
    let divider_json = serde_json::to_value(&outer_children[0]).expect("divider serializes");
    assert_eq!(divider_json["height"], serde_json::json!(1.0));
    assert_eq!(
        divider_json["fill"][0]["color"],
        serde_json::json!("#E7E2DE")
    );

    let inner = &outer_children[1];
    assert_eq!(
        labels_of(inner),
        vec!["Trips", "Explore", "Saved", "Profile"]
    );
    let inner_json = serde_json::to_value(inner).expect("inner tab row serializes");
    assert_eq!(
        inner_json["padding"],
        serde_json::json!([0.0, 4.0, 0.0, 4.0])
    );
    assert_eq!(inner_json["gap"], serde_json::json!(8.0));

    let tabs = inner.children().expect("four tabs");
    let icons = tabs
        .iter()
        .map(|tab| first_icon_name(tab).expect("tab icon"))
        .collect::<Vec<_>>();
    assert_eq!(icons, vec!["luggage", "compass", "bookmark", "user"]);
    for tab in tabs {
        let icon = tab
            .children()
            .and_then(|children| children.first())
            .expect("icon is first tab child");
        let icon_json = serde_json::to_value(icon).expect("icon serializes");
        assert_eq!(icon_json["width"], serde_json::json!(22.0));
        assert_eq!(icon_json["height"], serde_json::json!(22.0));
    }
    assert_eq!(first_fill(&tabs[0]).as_deref(), Some(INACTIVE));
    assert_eq!(
        first_fill(&tabs[2]).as_deref(),
        Some(ACTIVE),
        "the cloned reference chrome must retarget active styling to Saved"
    );
}

#[test]
fn nested_nav_same_labels_but_glyph_size_and_padding_drift_replaces_complete_outer_chrome() {
    let mut state = state_from(two_screen_nested_nav_chrome_drift_doc());
    run_pass(&mut state);
    assert_saved_nested_nav_matches_reference(&state);
}

#[test]
fn nested_nav_complete_outer_sync_is_idempotent_on_second_run() {
    let mut state = state_from(two_screen_nested_nav_chrome_drift_doc());
    run_pass(&mut state);
    assert_saved_nested_nav_matches_reference(&state);
    let once = serde_json::to_string(state.active_children()).expect("first pass snapshot");
    run_pass(&mut state);
    let twice = serde_json::to_string(state.active_children()).expect("second pass snapshot");
    assert_eq!(once, twice, "nested nav sync must be a second-run no-op");
}

#[test]
fn nested_nav_retarget_preserves_reference_route_at_each_label_position() {
    // Every reference tab is already wired to a distinct destination. The
    // target's independently-authored routes deliberately disagree, so a
    // passing result proves both halves of the contract: the complete nav
    // came from the reference, and moving active styling from Trips to
    // Saved did NOT move each tab's event along with that styling.
    let mut state = state_from(two_screen_nested_nav_chrome_drift_doc());
    run_pass(&mut state);

    let saved_root = find_by_id(state.active_children(), "saved").expect("Saved screen");
    let outer = saved_root
        .children()
        .and_then(|children| children.first())
        .expect("complete outer nav");
    let inner = outer
        .children()
        .and_then(|children| children.get(1))
        .expect("inner tab row after divider");
    let tabs = inner.children().expect("four tabs");
    assert_eq!(
        labels_of(inner),
        vec!["Trips", "Explore", "Saved", "Profile"]
    );

    let routes = tabs
        .iter()
        .map(|tab| {
            serde_json::to_value(tab).expect("tab serializes")["events"]["onTap"][0]["replace"]
                .as_str()
                .expect("replace route is a string literal")
                .to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        routes,
        vec!["\"/\"", "\"/explore\"", "\"/saved\"", "\"/profile\""],
        "routes belong to label positions; active-style retargeting must not swap them"
    );
    assert_eq!(first_fill(&tabs[0]).as_deref(), Some(INACTIVE));
    assert_eq!(first_fill(&tabs[2]).as_deref(), Some(ACTIVE));
}

#[test]
fn route_only_drift_still_adopts_reference_routes() {
    let style = || NestedNavStyle {
        icon_size: 22.0,
        outer_height: 88.0,
        outer_padding: [8.0, 16.0, 8.0, 16.0],
        outer_fill: "#FFFDFC",
        divider_fill: "#E7E2DE",
        inner_padding: [0.0, 4.0, 0.0, 4.0],
        inner_gap: 8.0,
    };
    let doc = serde_json::json!({
        "version": "1.0",
        "children": [
            screen_json("trips", "Trips", nested_nav_json(
                "reference", "Trips", ["luggage", "compass", "bookmark", "user"],
                ["/", "/explore", "/saved", "/profile"], style(),
            )),
            screen_json("saved", "Saved", nested_nav_json(
                "target", "Saved", ["luggage", "compass", "bookmark", "user"],
                ["/wrong-1", "/wrong-2", "/wrong-3", "/wrong-4"], style(),
            )),
        ]
    });
    let mut state = state_from(doc);
    run_pass(&mut state);

    let saved = find_by_id(state.active_children(), "saved").unwrap();
    let tabs = saved.children().unwrap()[0].children().unwrap()[1]
        .children()
        .unwrap();
    let routes = tabs
        .iter()
        .map(|tab| serde_json::to_value(tab).unwrap()["events"]["onTap"][0]["replace"].clone())
        .collect::<Vec<_>>();
    assert_eq!(
        serde_json::Value::Array(routes),
        serde_json::json!(["\"/\"", "\"/explore\"", "\"/saved\"", "\"/profile\""])
    );
}

#[test]
fn active_retarget_preserves_descendant_routes_by_tab_position() {
    let mut nav_json = nested_nav_json(
        "reference",
        "Trips",
        ["luggage", "compass", "bookmark", "user"],
        ["/", "/explore", "/saved", "/profile"],
        NestedNavStyle {
            icon_size: 22.0,
            outer_height: 88.0,
            outer_padding: [8.0, 16.0, 8.0, 16.0],
            outer_fill: "#FFFDFC",
            divider_fill: "#E7E2DE",
            inner_padding: [0.0, 4.0, 0.0, 4.0],
            inner_gap: 8.0,
        },
    );
    for tab in nav_json["children"][1]["children"].as_array_mut().unwrap() {
        let events = tab.as_object_mut().unwrap().remove("events").unwrap();
        tab["children"][0]["events"] = events;
    }
    let mut nav: PenNode = serde_json::from_value(nav_json).unwrap();
    retarget_active_tab_at_path(&mut nav, &[1], "Saved");

    let tabs = nav.children().unwrap()[1].children().unwrap();
    let routes = tabs
        .iter()
        .map(|tab| {
            serde_json::to_value(&tab.children().unwrap()[0]).unwrap()["events"]["onTap"][0]
                ["replace"]
                .clone()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        serde_json::Value::Array(routes),
        serde_json::json!(["\"/\"", "\"/explore\"", "\"/saved\"", "\"/profile\""])
    );
}

// ---------------------------------------------------------------------
// Detail-page Inject exemption (0718-1-k3-1 postmortem, product decision).
// A push-in detail screen (back header, no corresponding bottom-nav tab)
// must never get a bottom-nav forced onto it — even in the edge case where
// its bare name happens to token-match a reference tab label. Inject-only:
// an authored nav (Replace / RetargetActiveOnly) is never touched.
// ---------------------------------------------------------------------

/// A nav-less screen with a back-shaped header control (chevron-left /
/// arrow-left icon as the first child — resolves within the header region
/// exactly like the legacy shared-nav detail exemption).
fn screen_json_no_nav_with_back_header(id: &str, name: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "frame", "id": id, "name": name, "width": 390, "height": 844,
        "layout": "vertical", "children": [
            { "type": "icon_font", "id": format!("{id}-back"), "name": "Back",
              "iconFontName": "arrow-left", "width": 24, "height": 24 },
            { "type": "text", "id": format!("{id}-body"), "content": "Body", "fontSize": 16 }
        ]
    })
}

#[test]
fn detail_page_shape_with_matching_name_and_back_header_is_exempted_from_injection() {
    // "Library" coincidentally matches a reference tab label (so the old
    // `eligible` check alone would have injected a nav), but it also carries
    // a back-shaped header control — the detail-page signal that overrides
    // the name-match.
    let doc = serde_json::json!({
        "version": "1.0",
        "children": [
            screen_json("home", "Home", nav_json("nav-home", "h", "Home", "Library")),
            screen_json_no_nav_with_back_header("library", "Library"),
        ]
    });
    let mut state = state_from(doc);
    run_pass(&mut state);

    let library_root = find_by_id(state.active_children(), "library").unwrap();
    let mut navs = Vec::new();
    collect_nav_containers(library_root, &mut navs);
    assert!(
        navs.is_empty(),
        "a screen whose name coincidentally matches a tab, but has a back-shaped header \
         control, must be exempted from nav injection (detail-page shape)"
    );
}

#[test]
fn matching_name_without_back_header_still_gets_injected_as_before() {
    // Same "Library" name-match shape, but NO back-shaped header control —
    // the pre-existing Inject behavior must be completely unaffected by the
    // new gate (only satisfying ONE of the two exemption signals).
    let mut state = state_from(three_screen_missing_nav_doc());
    run_pass(&mut state);

    let library_root = find_by_id(state.active_children(), "library").unwrap();
    let mut navs = Vec::new();
    collect_nav_containers(library_root, &mut navs);
    assert!(
        !navs.is_empty(),
        "a name-matching screen with no back-header signal must still get its nav injected"
    );
}

/// 0718-1-k3-1's real Destination shape: it AUTHORED its own nav (so it
/// goes through Replace / RetargetActiveOnly, never Inject) and ALSO
/// carries a header back control. This regression proves the Inject-only
/// gate structurally cannot touch it — an authored-nav screen never reaches
/// the `None =>` branch of `resolve_target` where the gate lives.
#[test]
fn authored_nav_with_back_header_is_unaffected_by_the_inject_only_gate() {
    let doc = serde_json::json!({
        "version": "1.0",
        "children": [
            screen_json("home", "Home", nav_json("nav-home", "h", "Home", "Library")),
            {
                "type": "frame", "id": "destination", "name": "Destination",
                "width": 390, "height": 844, "layout": "vertical",
                "children": [
                    { "type": "icon_font", "id": "destination-back", "name": "Back",
                      "iconFontName": "arrow-left", "width": 24, "height": 24 },
                    nav_json("nav-destination", "d", "Home", "Library")
                ]
            },
        ]
    });
    let mut state = state_from(doc);
    run_pass(&mut state);

    let destination_root = find_by_id(state.active_children(), "destination").unwrap();
    let mut navs = Vec::new();
    collect_nav_containers(destination_root, &mut navs);
    assert_eq!(
        navs.len(),
        1,
        "authored nav must still be present — Replace path unaffected by the Inject-only gate"
    );
    assert_eq!(
        labels_of(navs[0]),
        vec!["Home", "Search", "Library", "Premium"]
    );
}

// ---------------------------------------------------------------------
// Chrome-role stamping (0718-1-k3-1 review fix). `nav_json` above always
// carries `role: "bottom-tab-bar"`, but `find_reference_nav` /
// `is_nav_container` match a reference nav by role OR name/id substring —
// a purely name-matched reference (no role at all) is a real, reachable
// authored shape. `unfilled_screens.rs`'s chrome exclusion is role-only,
// so an unstamped clone's own tab-label text would read as real content,
// the same class of bug `unify_shared_status_bar`'s `stamp_chrome_role`
// closes for status bars.
// ---------------------------------------------------------------------

/// A nav matched purely by NAME ("Bottom Nav" substring) — carries no
/// `role` at all, the authored shape `is_nav_container`'s name-fallback
/// path exists for.
fn roleless_nav_json(nav_id: &str) -> serde_json::Value {
    let tab = |label: &str, suffix: &str| {
        serde_json::json!({
            "type": "frame", "id": format!("{nav_id}-{suffix}"), "width": 80, "height": 56,
            "layout": "vertical", "children": [
                { "type": "text", "id": format!("{nav_id}-{suffix}-lbl"), "content": label, "fontSize": 12 }
            ]
        })
    };
    serde_json::json!({
        "type": "frame", "id": nav_id, "name": "Bottom Nav",
        "width": "fill_container", "height": 72, "layout": "horizontal",
        "children": [
            tab("Home", "home"), tab("Search", "search"),
            tab("Library", "library"), tab("Premium", "star")
        ]
    })
}

fn empty_screen_json(id: &str, name: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "frame", "id": id, "name": name, "width": 390, "height": 844,
        "layout": "vertical", "children": []
    })
}

fn home_plus_empty_library_doc() -> serde_json::Value {
    serde_json::json!({
        "version": "1.0",
        "children": [
            screen_json("home", "Home", roleless_nav_json("nav-home")),
            empty_screen_json("library", "Library"),
        ]
    })
}

#[test]
fn injected_nav_clone_gets_the_chrome_role_stamped() {
    let mut state = state_from(home_plus_empty_library_doc());
    run_pass(&mut state);

    let library_root = find_by_id(state.active_children(), "library").unwrap();
    let mut navs = Vec::new();
    collect_nav_containers(library_root, &mut navs);
    let injected = navs.first().expect("nav injected onto Library");
    assert_eq!(
        injected.base().role.as_deref(),
        Some("bottom-tab-bar"),
        "an injected clone must carry role:\"bottom-tab-bar\" even though \
         the authored reference has none, so unfilled_screens' CHROME_ROLES \
         check recognizes it as chrome, not model-authored content"
    );
}

#[test]
fn roleless_reference_with_stamped_clone_is_idempotent_on_second_run() {
    let mut state = state_from(home_plus_empty_library_doc());
    run_pass(&mut state);
    let once = serde_json::to_string(state.active_children()).expect("first pass snapshot");

    run_pass(&mut state);
    let twice = serde_json::to_string(state.active_children()).expect("second pass snapshot");
    assert_eq!(
        once, twice,
        "the stamped clone must compare equal to its roleless reference after canonicalization"
    );
}

#[test]
fn authored_reference_nav_role_is_never_touched() {
    let mut state = state_from(home_plus_empty_library_doc());
    run_pass(&mut state);

    let home_root = find_by_id(state.active_children(), "home").unwrap();
    let reference = find_by_id(home_root.children().unwrap(), "nav-home").unwrap();
    assert_eq!(
        reference.base().role,
        None,
        "role-stamping must only ever touch the CLONE — the authored \
         reference screen's own nav must be left exactly as authored"
    );
}

/// The regression this fix exists for: a genuinely unfilled screen (zero
/// real content, no nav at all) that gains ONLY an injected nav must still
/// read as unfilled afterward — the injected chrome's own tab-label text
/// must not count as "the model did something here".
#[test]
fn screen_that_only_gained_a_nav_still_reads_as_unfilled() {
    let mut state = state_from(home_plus_empty_library_doc());
    run_pass(&mut state);

    let library_root = find_by_id(state.active_children(), "library").unwrap();
    let mut navs = Vec::new();
    collect_nav_containers(library_root, &mut navs);
    assert!(
        !navs.is_empty(),
        "setup: Library must have gained the injected nav"
    );

    let unfilled = crate::unfilled_screens::detect_unfilled_screens(&state);
    assert!(
        unfilled.iter().any(|hit| hit.name == "Library"),
        "Library gained ONLY chrome (a nav) — it must still be reported \
         unfilled, not silently flipped to filled: {unfilled:?}"
    );
}
