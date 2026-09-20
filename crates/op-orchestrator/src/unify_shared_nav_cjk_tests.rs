//! CJK continuation-screen active tab — split from
//! `unify_shared_nav_active_tab_tests.rs`, which is at the 800-line cap.
//!
//! Reuses that file's neighbours' fixture helpers (`pub(super)`) rather than
//! duplicating them.

use super::tests::{run_pass, screen_json, state_from, ACTIVE, INACTIVE};
use super::*;

/// A CJK two-screen app: the continuation screen cloned screen 1's nav with
/// screen 1's tab still active. `0808-k3-2.op`'s shape.
fn cjk_two_screen_doc() -> serde_json::Value {
    fn cjk_nav(nav_id: &str, prefix: &str, active_label: &str) -> serde_json::Value {
        let tabs = [
            ("今夜", "sparkles"),
            ("星图", "globe"),
            ("地点", "map-pin"),
            ("我的", "user"),
        ];
        let children: Vec<serde_json::Value> = tabs
            .iter()
            .map(|(label, icon)| {
                let active = *label == active_label;
                let color = if active { ACTIVE } else { INACTIVE };
                let mut kids = vec![
                    serde_json::json!({"type":"icon_font","id":format!("{prefix}-{icon}-icon"),
                        "iconFontName":icon,"width":24,"height":24,
                        "fill":[{"type":"solid","color":color}]}),
                    serde_json::json!({"type":"text","id":format!("{prefix}-{icon}-lbl"),
                        "content":label,"fontSize":12,
                        "fill":[{"type":"solid","color":color}]}),
                ];
                if active {
                    kids.push(serde_json::json!({"type":"rectangle",
                        "id":format!("{prefix}-{icon}-dot"),"width":20,"height":2,
                        "fill":[{"type":"solid","color":color}]}));
                }
                serde_json::json!({"type":"frame","id":format!("{prefix}-{icon}-item"),
                    "width":80,"height":56,"layout":"vertical","children":kids})
            })
            .collect();
        serde_json::json!({"type":"frame","id":nav_id,"name":"Bottom Nav",
            "role":"bottom-tab-bar","width":"fill_container","height":72,
            "layout":"horizontal","children":children})
    }
    serde_json::json!({
        "version": "1.0",
        "children": [
            screen_json("s1", "Nocturne 今夜", cjk_nav("nav-1", "a", "今夜")),
            // The continuation screen: 星图, but 今夜 is still the active tab.
            screen_json("s2", "星图", cjk_nav("nav-2", "b", "今夜")),
        ]
    })
}

#[test]
fn a_cjk_continuation_screen_gets_its_own_tab_active() {
    // `normalize_label` used to filter on `is_ascii_alphanumeric`, so every
    // CJK label normalized to "" — `find_tab_index_for_screen` matched
    // nothing and this whole retarget degraded to a no-op on Chinese apps.
    // Measured on `0808-k3-2.op`, where the 星图 screen shipped highlighting
    // 今夜 and no tab received an `onTap` action either.
    let mut state = state_from(cjk_two_screen_doc());
    run_pass(&mut state);

    let nav = find_nav_on_screen(&state, "星图").expect("星图 keeps a nav");
    let active: Vec<String> = nav
        .children()
        .expect("tabs")
        .iter()
        .filter(|tab| tab.children().is_some_and(|kids| kids.len() == 3))
        .filter_map(|tab| first_text_content(tab).map(str::to_string))
        .collect();
    assert_eq!(
        active,
        vec!["星图".to_string()],
        "the continuation screen highlights its OWN tab, exactly one of them"
    );

    let reference = find_nav_on_screen(&state, "Nocturne 今夜").expect("screen 1 keeps a nav");
    let reference_active: Vec<String> = reference
        .children()
        .expect("tabs")
        .iter()
        .filter(|tab| tab.children().is_some_and(|kids| kids.len() == 3))
        .filter_map(|tab| first_text_content(tab).map(str::to_string))
        .collect();
    assert_eq!(
        reference_active,
        vec!["今夜".to_string()],
        "and the reference screen is left as authored"
    );
}

/// The `bottom-tab-bar` inside the screen frame named `screen_name`.
fn find_nav_on_screen<'a>(
    state: &'a op_editor_core::EditorState,
    screen_name: &str,
) -> Option<&'a PenNode> {
    fn nav_in(node: &PenNode) -> Option<&PenNode> {
        if node.base().role.as_deref() == Some("bottom-tab-bar") {
            return Some(node);
        }
        node.children()?.iter().find_map(nav_in)
    }
    state
        .active_children()
        .iter()
        .find(|root| root.base().name.as_deref() == Some(screen_name))
        .and_then(|root| nav_in(root))
}
