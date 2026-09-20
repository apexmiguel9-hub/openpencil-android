//! Tests for the app-shell restructure pass. The positive case is the reported
//! glm barbershop dashboard; the negatives are the false-positives the
//! adversarial design review flagged (top-nav, short header, mobile, already
//! horizontal, already-narrow sidebar, too-few sections, restaurant "Menu",
//! "Navy" hero, `fit_content` nav, no dashboard content, multi-screen file).

use super::*;

fn node(v: Value) -> PenNode {
    serde_json::from_value::<PenNode>(v).expect("valid PenNode fixture")
}

fn val(n: &PenNode) -> Value {
    serde_json::to_value(n).expect("serialize PenNode")
}

/// A leaf section frame with a name + sizing (children optional).
fn section(name: &str, width: Value, height: Value) -> Value {
    json!({
        "type": "frame", "id": name.replace(' ', "-"), "name": name,
        "width": width, "height": height, "layout": "vertical", "children": []
    })
}

/// The reported bug shape: vertical 1200-wide root, full-width sidebar first.
fn bug_wrapper() -> PenNode {
    node(json!({
        "type": "frame", "id": "root", "name": "Barbershop Client Management",
        "width": 1200, "height": 1775, "layout": "vertical", "gap": 32,
        "children": [
            { "type": "frame", "id": "n2", "name": "Sidebar Navigation",
              "width": 1200, "height": 605, "layout": "vertical",
              "children": [ section("Logo", json!(1152), json!(27)) ] },
            section("Top Header", json!(1200), json!(94)),
            section("Key Metrics", json!(1200), json!(117)),
            section("Client Table Section", json!(1200), json!(488)),
            section("Upcoming Appointments", json!(1200), json!(391)),
        ]
    }))
}

// Cluster test modules — this file keeps the shared fixtures.
#[path = "app_shell_detect_tests.rs"]
mod detect_tests;
#[path = "app_shell_sidebar_tests.rs"]
mod sidebar_tests;
