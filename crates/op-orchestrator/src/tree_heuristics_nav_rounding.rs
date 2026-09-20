//! Bottom-nav bar detection, active-tab pill rounding and the backdrop
//! child-fill merge.

use super::*;

/// Run the post-streaming heuristics over a subtask forest. Each root is a
/// page-root child (a section). MUST run BEFORE variable binding —
/// `strip_redundant_section_fill` matches literal hedge HEX that binding would
/// otherwise convert to `$color-*` refs. `page_bg` is the plan root frame's
/// fill hex when known (enables the "section fill == page bg" strip case).
/// A bottom-tab-bar's ACTIVE tab is frequently authored as a solid-filled frame
/// with NO `cornerRadius` — a sharp rectangle that pokes out of the rounded nav
/// pill (the "active block is an unreasonable square overflowing the navbar" the
/// user flagged). The TS references render the active item as a rounded pill.
/// Round any filled tab-item frame inside a bottom-tab-bar so it reads as a
/// contained highlight; `cornerRadius: 999` lets the renderer clamp to height/2
/// → a pill. The bar / pill container itself (already carrying a radius) and the
/// inactive tabs (no fill) are untouched.
/// Roles / structures that identify a bottom navigation bar. The role set is
/// broad (raw-JSON models tag the bar `bottom-tab-bar`; others use `tab-bar` /
/// `nav` / `tab-row`), and as a fallback ANY horizontal container whose children
/// are nav tab items (`nav-item` / `nav-item-active`, the manifest
/// element-builder roles) counts — so the active-tab rounding fires regardless
/// of which generation path produced the bar.
pub(super) const NAV_BAR_ROLES: &[&str] = &[
    "bottom-tab-bar",
    "tab-bar",
    "nav",
    "tab-row",
    "navbar",
    "bottom-nav",
];

pub(super) fn is_nav_bar_container(node: &Value) -> bool {
    if NAV_BAR_ROLES.contains(&role_of(node).unwrap_or_default()) {
        return true;
    }
    let horizontal = node.get("layout").and_then(Value::as_str) == Some("horizontal");
    horizontal
        && node
            .get("children")
            .and_then(Value::as_array)
            .map(|kids| {
                kids.iter()
                    .any(|c| matches!(role_of(c), Some("nav-item") | Some("nav-item-active")))
            })
            .unwrap_or(false)
}

pub(super) fn round_active_nav_tab(node: &mut Value, accent: &str) {
    if is_nav_bar_container(node) {
        // A rounded-pill bar MUST clip its children — a full-height active block
        // that pokes out past the pill's rounded corners gets confined.
        let bar_radius = node
            .get("cornerRadius")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        if bar_radius >= 8.0 {
            if let Some(obj) = node.as_object_mut() {
                obj.insert("clipContent".to_string(), Value::Bool(true));
            }
        }
        round_nav_tab_items(node, true, accent);
    } else if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
        for child in children {
            round_active_nav_tab(child, accent);
        }
    }
}

pub(super) fn round_nav_tab_items(node: &mut Value, is_bar_root: bool, accent: &str) {
    // Cover both `frame` tab items AND a bare `rectangle` highlight behind the
    // icon/label — either is a sharp overflowing square if left un-rounded.
    let is_block = matches!(
        node.get("type").and_then(Value::as_str),
        Some("frame") | Some("rectangle")
    );
    let role = role_of(node).map(str::to_string).unwrap_or_default();
    let is_active_tab = role == "nav-item-active";
    let has_fill = node
        .get("fill")
        .and_then(Value::as_array)
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    let is_full_row = node.get("width").and_then(Value::as_str) == Some("fill_container")
        && node.get("layout").and_then(Value::as_str) == Some("horizontal");
    let radius = node
        .get("cornerRadius")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    if !is_bar_root && is_block && !is_full_row {
        if is_active_tab {
            // The active tab MUST read as a CONTAINED rounded pill. Round it, and
            // if it carries no visible highlight fill, inject the design accent so
            // the active state is a proper orange pill (the manifest element
            // builder leaves `nav-item-active` unfilled; glm sometimes gives it a
            // sharp filled square). Either path → rounded accent pill.
            if radius < 8.0 {
                node["cornerRadius"] = Value::from(999.0);
            }
            if !has_renderable_fill(node) {
                node["fill"] = json!([{ "type": "solid", "color": accent }]);
            }
        } else if has_fill && radius < 8.0 {
            node["cornerRadius"] = Value::from(999.0);
        }
    }
    if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
        for child in children {
            round_nav_tab_items(child, false, accent);
        }
    }
}

/// A model authors an "active background" as a FLEX SIBLING: a fill-less
/// container whose first child is an EMPTY `fill_container` × `fill_container`
/// rectangle/frame carrying the fill. In flex flow that backdrop is a real
/// item — it eats half the row and shoves the actual content sideways
/// (measured: a sidebar nav item's icon+label pushed outside the 260px rail by
/// its own "Active Bg" rectangle). The fill/cornerRadius belong on the
/// CONTAINER: move them there and delete the backdrop child. The double-fill
/// empty box is unambiguous — a divider is thin (one fixed axis), a spacer
/// carries no fill, an avatar placeholder has a fixed size.
pub(super) fn merge_backdrop_child_fill(v: &mut Value) {
    let is_backdrop = |c: &Value| -> bool {
        matches!(
            c.get("type").and_then(Value::as_str),
            Some("rectangle" | "frame")
        ) && c
            .get("children")
            .and_then(Value::as_array)
            .map(|k| k.is_empty())
            .unwrap_or(true)
            && c.get("width").and_then(Value::as_str) == Some("fill_container")
            && c.get("height").and_then(Value::as_str) == Some("fill_container")
            && c.get("fill")
                .and_then(Value::as_array)
                .is_some_and(|f| !f.is_empty())
    };
    let container_has_fill = v
        .get("fill")
        .and_then(Value::as_array)
        .is_some_and(|f| !f.is_empty())
        || v.get("fill").and_then(Value::as_str).is_some();
    let eligible = v.get("type").and_then(Value::as_str) == Some("frame")
        && !container_has_fill
        && v.get("children")
            .and_then(Value::as_array)
            .is_some_and(|kids| kids.len() >= 2 && is_backdrop(&kids[0]));
    if eligible {
        if let Some(obj) = v.as_object_mut() {
            if let Some(Value::Array(mut kids)) = obj.remove("children") {
                let backdrop = kids.remove(0);
                if let Some(fill) = backdrop.get("fill").cloned() {
                    obj.insert("fill".into(), fill);
                }
                if obj.get("cornerRadius").is_none() {
                    if let Some(radius) = backdrop.get("cornerRadius").cloned() {
                        obj.insert("cornerRadius".into(), radius);
                    }
                }
                obj.insert("children".into(), Value::Array(kids));
            }
        }
    }
    if let Some(kids) = v.get_mut("children").and_then(Value::as_array_mut) {
        for c in kids.iter_mut() {
            merge_backdrop_child_fill(c);
        }
    }
}
