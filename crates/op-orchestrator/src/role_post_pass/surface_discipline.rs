//! Surface colour discipline — strip page-tone/state fills that repaint the
//! root background (`enforce_surface_color_discipline`'s worker).

use super::*;

/// Run the contrast post-pass over a forest of sub-agent section roots. Each
/// root is round-tripped through JSON; on any (de)serialize failure that root
/// is left untouched (a fix can never drop a node).
/// Semantic state-feedback tokens. Legit only on status/alert elements; glm
/// grabs them as "a light color" for decorative surfaces (a `$color-danger-bg`
/// search input renders pink and clashes with the theme).
pub(super) const STATE_BG_REFS: &[&str] = &[
    "$color-danger-bg",
    "$color-info-bg",
    "$color-success-bg",
    "$color-warning-bg",
];

/// The page-background token. Only the page root paints it; an inner node using
/// it just repaints a redundant — or theme-clashing (cool `#F8FAFC` over a warm
/// page) — panel.
pub(super) const PAGE_BG_REF: &str = "$color-bg-deep";

/// A status / feedback element — the ONLY legitimate user of a state-bg token.
pub(super) fn is_status_element(node: &Value) -> bool {
    if let Some(role) = role_of(node) {
        if matches!(role, "badge" | "alert" | "toast" | "status") {
            return true;
        }
    }
    if let Some(name) = node.get("name").and_then(Value::as_str) {
        let l = name.to_lowercase();
        return [
            "error",
            "success",
            "warning",
            "alert",
            "danger",
            "status",
            "toast",
            "notification",
        ]
        .iter()
        .any(|k| l.contains(k));
    }
    false
}

/// Surface-color discipline — a deterministic floor walking EVERY node type
/// (incl. `text_input`, which the frame-only `post_pass_value` skips). The TS
/// pipeline relies on the prompt for this; weak models (glm-5.2) ignore it, so
/// Rust enforces it after the fact:
///   1. A state-bg token misused as a decorative surface → neutral
///      `$color-surface-2`. (the pink search input / chips)
///   2. `$color-bg-deep` on any inner node → transparent. (the cool grey panel
///      behind the search row / a nav tab repainting the page bg)
///
/// Refs are still UNRESOLVED here (binding runs later), so match token names.
pub(super) fn node_has_effects(node: &Value) -> bool {
    node.get("effects")
        .and_then(Value::as_array)
        .map(|a| !a.is_empty())
        .unwrap_or(false)
}

pub(super) fn has_any_stroke(node: &Value) -> bool {
    node.get("stroke").map(|s| !s.is_null()).unwrap_or(false)
}

/// Entry point. Captures the root's own ground once so the page-bg strip below
/// can prove the redundancy it claims, then walks the subtree.
pub(super) fn fix_surface_color_discipline(node: &mut Value, is_root: bool) {
    // Entering mid-tree leaves the page's ground unknown; an unprovable
    // redundancy is not repaired (see `walk_surface_color_discipline`).
    let page_bg = if is_root {
        get_first_solid_color(node)
    } else {
        None
    };
    walk_surface_color_discipline(node, is_root, page_bg.as_deref());
}

fn walk_surface_color_discipline(node: &mut Value, is_root: bool, page_bg: Option<&str>) {
    if let Some(color) = get_first_solid_color(node) {
        if STATE_BG_REFS.contains(&color.as_str()) && !is_status_element(node) {
            node["fill"] = solid_fill("$color-surface-2");
        } else if !is_root && color == PAGE_BG_REF && page_bg == Some(PAGE_BG_REF) {
            // REDUNDANCY repair, not a taste call: the strip is only sound
            // where the root paints this very token, so the inner band is
            // provably invisible. A root grounded in anything else — a literal
            // hex, a gradient, no fill at all — makes `$color-bg-deep` a
            // DISTINCT surface, and emptying it deletes an authored band
            // (measured: 0808-gm-1.op's `#0A0A0A` page, whose two
            // `$color-bg-deep` (#0F172A) sections lost their darker ground).
            node["fill"] = json!([]);
        } else if color.starts_with("$color-text-") && is_container_kind(node) {
            // A CONTAINER filled with a TEXT token is a slot-category error —
            // a search pill painted `$color-text-primary` rendered as a WHITE
            // capsule on the dark luxury theme (measured: ATELIER's search +
            // FILTER pills). Text tokens color glyphs; the container slot for
            // inputs/chips is surface-2. Its dark literal text (styled for
            // the accidental white) flips to the text ladder with it.
            node["fill"] = solid_fill("$color-surface-2");
            rebind_dark_literal_text(node);
        }
    }
    // An elevation shadow needs a surface to sit on. A frame with no visible
    // fill and no stroke that still carries a drop-shadow renders the shadow as
    // a gray "ghost box" floating around its children — strip it. This runs last
    // (after binding + every fill-stripping pass), so it sees the FINAL fill
    // state and catches both our own injected card shadows on wrappers that got
    // emptied and model-authored shadows on bare wrapper frames.
    if node_has_effects(node) && !has_visible_fill(node) && !has_any_stroke(node) {
        if let Some(obj) = node.as_object_mut() {
            obj.remove("effects");
        }
    }
    if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
        for child in children.iter_mut() {
            walk_surface_color_discipline(child, false, page_bg);
        }
    }
}
