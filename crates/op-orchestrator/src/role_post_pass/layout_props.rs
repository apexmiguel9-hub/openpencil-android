//! I4 layout-PROPERTY cluster — pure property sets, no layout recompute:
//! input sibling consistency, card-row equalization, form input widths,
//! trailing-icon alignment, image clipping and the promo-pane fixes.

use super::*;

pub(super) fn fix_input_sibling_consistency(node: &mut Value) {
    if node.get("layout").and_then(Value::as_str) != Some("vertical") {
        return;
    }
    let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) else {
        return;
    };
    let input_idxs: Vec<usize> = children
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            c.get("type").and_then(Value::as_str) == Some("frame")
                && matches!(role_of(c), Some("input") | Some("form-input"))
                && has_fill(c)
        })
        .map(|(i, _)| i)
        .collect();
    if input_idxs.len() < 2 {
        return;
    }
    let Some(first_color) = get_first_solid_color(&children[input_idxs[0]]) else {
        return;
    };
    let all_match = input_idxs
        .iter()
        .all(|&i| get_first_solid_color(&children[i]).as_deref() == Some(first_color.as_str()));
    if all_match {
        return;
    }
    let source_fill = children[input_idxs[0]].get("fill").cloned();
    let source_stroke = children[input_idxs[0]].get("stroke").cloned();
    for &i in &input_idxs[1..] {
        if let Some(fill) = &source_fill {
            children[i]["fill"] = fill.clone();
        }
        if let Some(stroke) = &source_stroke {
            children[i]["stroke"] = stroke.clone();
        }
    }
}

// ── I4: layout-property fixes (no layout recompute / no font metrics) ───────

/// Equalize the widths of fixed-width card frames so they share a row evenly
/// (port of `equalizeCardRow` AND the near-identical
/// `equalizeHorizontalSiblings` in design-canvas-ops.ts — the dashboard 等宽
/// pass; the badge/pill/tag exclusions come from the latter). Pure property fix
/// — taffy then lays out.
pub(super) fn equalize_card_row(node: &mut Value) {
    if node.get("layout").and_then(Value::as_str) != Some("horizontal") {
        return;
    }
    if node.get("width").and_then(Value::as_str) == Some("fit_content") {
        return;
    }
    let Some(children) = node.get("children").and_then(Value::as_array) else {
        return;
    };
    if children.len() < 2 {
        return;
    }
    let candidates: Vec<usize> = children
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            c.get("type").and_then(Value::as_str) == Some("frame")
                && !matches!(
                    role_of(c),
                    Some("divider")
                        | Some("phone-mockup")
                        | Some("badge")
                        | Some("pill")
                        | Some("tag")
                )
                && size_number(c, "height") > 88.0
        })
        .map(|(i, _)| i)
        .collect();
    // Already an explicit fill_container card in the row → widths are equal,
    // nothing to equalize. (A flexbox `minWidth:0` shrink-fix is NOT viable in
    // the node tree: the canonical PenNode schema has no `minWidth` field, so any
    // stamp is silently dropped on the serialize round-trip. Preventing a wide
    // KPI card from overflowing its share of the row is a jian flex-mapping
    // concern — fill_container columns need flex-basis:0 to shrink — not a
    // post-pass.)
    if candidates
        .iter()
        .any(|&i| children[i].get("width").and_then(Value::as_str) == Some("fill_container"))
    {
        return;
    }
    let fixed: Vec<usize> = candidates
        .into_iter()
        .filter(|&i| {
            children[i]
                .get("width")
                .and_then(Value::as_f64)
                .map(|w| w > 0.0)
                .unwrap_or(false)
        })
        .collect();
    if fixed.len() < 2 {
        return;
    }
    let widths: Vec<f64> = fixed
        .iter()
        .map(|&i| size_number(&children[i], "width"))
        .collect();
    let max_w = widths.iter().cloned().fold(0.0_f64, f64::max);
    let min_w = widths.iter().cloned().fold(f64::INFINITY, f64::min);
    if max_w <= 0.0 || min_w / max_w >= 0.6 {
        return; // widths already similar → leave alone
    }
    let heights: Vec<f64> = fixed
        .iter()
        .map(|&i| size_number(&children[i], "height"))
        .collect();
    let max_h = heights.iter().cloned().fold(0.0_f64, f64::max);
    let min_h = heights.iter().cloned().fold(f64::INFINITY, f64::min);
    if max_h <= 0.0 || min_h / max_h <= 0.5 {
        return; // heights too dissimilar → probably not a card row
    }
    let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) else {
        return;
    };
    for &i in &fixed {
        children[i]["width"] = json!("fill_container");
    }
}

/// When a vertical group has a `fill_container` frame sibling, promote
/// fixed-width inputs to `fill_container` too (port of `normalizeFormInputWidths`).
pub(super) fn normalize_form_input_widths(node: &mut Value) {
    if node.get("layout").and_then(Value::as_str) != Some("vertical") {
        return;
    }
    if node.get("width").and_then(Value::as_str) == Some("fit_content") {
        return;
    }
    let Some(children) = node.get("children").and_then(Value::as_array) else {
        return;
    };
    if children.len() < 2 {
        return;
    }
    let has_fill_sibling = children.iter().any(|c| {
        c.get("type").and_then(Value::as_str) == Some("frame")
            && c.get("width").and_then(Value::as_str) == Some("fill_container")
            && role_of(c) != Some("divider")
    });
    if !has_fill_sibling {
        return;
    }
    let targets: Vec<usize> = children
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            c.get("type").and_then(Value::as_str) == Some("frame")
                && matches!(role_of(c), Some("input") | Some("form-input"))
                && c.get("width").and_then(Value::as_f64).is_some()
        })
        .map(|(i, _)| i)
        .collect();
    let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) else {
        return;
    };
    for i in targets {
        children[i]["width"] = json!("fill_container");
    }
}

pub(super) fn is_icon_like(node: &Value) -> bool {
    match node.get("type").and_then(Value::as_str) {
        Some("path") | Some("image") => true,
        Some("frame") => {
            if matches!(role_of(node), Some("icon") | Some("icon-button")) {
                return true;
            }
            let w = size_number(node, "width");
            let h = size_number(node, "height");
            w > 0.0 && h > 0.0 && w.max(h) <= 32.0
        }
        _ => false,
    }
}

/// In an input row with a trailing icon, make the text children `fill_container`
/// so the icon is pushed right while text stays left (port of
/// `normalizeInputTrailingIconAlignment`).
pub(super) fn normalize_input_trailing_icon_alignment(node: &mut Value) {
    if !matches!(role_of(node), Some("input") | Some("form-input")) {
        return;
    }
    match node.get("justifyContent").and_then(Value::as_str) {
        None | Some("start") => {}
        _ => return,
    }
    let Some(children) = node.get("children").and_then(Value::as_array) else {
        return;
    };
    let visible: Vec<usize> = children
        .iter()
        .enumerate()
        .filter(|(_, c)| c.get("visible").and_then(Value::as_bool) != Some(false))
        .map(|(i, _)| i)
        .collect();
    if visible.len() < 2 {
        return;
    }
    let last = *visible.last().unwrap();
    if !is_icon_like(&children[last]) {
        return;
    }
    let text_idxs: Vec<usize> = visible[..visible.len() - 1]
        .iter()
        .copied()
        .filter(|&i| children[i].get("type").and_then(Value::as_str) == Some("text"))
        .collect();
    if text_idxs.is_empty() {
        return;
    }
    let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) else {
        return;
    };
    for i in text_idxs {
        if children[i].get("width").and_then(Value::as_str) != Some("fill_container") {
            children[i]["width"] = json!("fill_container");
        }
        if children[i].get("textGrowth").is_none() {
            children[i]["textGrowth"] = json!("fixed-width");
        }
    }
}

/// Clip a rounded frame that contains an image so the image respects the
/// corner radius (port of the `clipContent` branch of resolveTreePostPass).
pub(super) fn apply_clip_content_for_image(node: &mut Value) {
    if node.get("clipContent").and_then(Value::as_bool) == Some(true) {
        return;
    }
    if corner_radius(node) <= 0.0 {
        return;
    }
    let has_image = node
        .get("children")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .any(|c| c.get("type").and_then(Value::as_str) == Some("image"))
        })
        .unwrap_or(false);
    if has_image {
        node["clipContent"] = json!(true);
    }
}

pub(super) fn has_cta_descendant(node: &Value) -> bool {
    if matches!(
        role_of(node),
        Some("button") | Some("cta") | Some("primary-cta")
    ) {
        return true;
    }
    let label = identity_label(node);
    if label.contains("cta") || label.contains("order now") || label.contains("shop now") {
        return true;
    }
    if node.get("type").and_then(Value::as_str) == Some("text") {
        let content = node
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_lowercase();
        if content.contains("order now")
            || content.contains("buy now")
            || content.contains("shop now")
            || content.contains("立即")
            || content.contains("购买")
            || content.contains("下单")
        {
            return true;
        }
    }
    node.get("children")
        .and_then(Value::as_array)
        .map(|children| children.iter().any(has_cta_descendant))
        .unwrap_or(false)
}

pub(super) fn is_promo_like_container(node: &Value) -> bool {
    let role = role_of(node).unwrap_or("");
    if matches!(
        role,
        "banner" | "feature-card" | "promo-card" | "offer-card"
    ) {
        return true;
    }
    let label = identity_label(node);
    ["promo", "offer", "deal", "discount", "limited", "banner"]
        .iter()
        .any(|needle| label.contains(needle))
}

pub(super) fn relax_clipping_promo_height(node: &mut Value) {
    if node.get("type").and_then(Value::as_str) != Some("frame") {
        return;
    }
    if node.get("height").and_then(Value::as_f64).is_none() {
        return;
    }
    let clips = node.get("clipContent").and_then(Value::as_bool) == Some(true);
    if has_cta_descendant(node) && (clips || is_promo_like_container(node)) {
        node["height"] = json!("fit_content");
    }
}

pub(super) fn normalize_horizontal_promo_copy_pane(node: &mut Value, canvas_width: f64) {
    if canvas_width > 480.0
        || node.get("type").and_then(Value::as_str) != Some("frame")
        || node.get("layout").and_then(Value::as_str) != Some("horizontal")
        || !is_promo_like_container(node)
    {
        return;
    }
    let Some(children) = node.get("children").and_then(Value::as_array) else {
        return;
    };
    if !children
        .iter()
        .any(|child| child.get("type").and_then(Value::as_str) == Some("image"))
    {
        return;
    }
    let copy_idx = children.iter().position(|child| {
        child.get("type").and_then(Value::as_str) == Some("frame")
            && child.get("layout").and_then(Value::as_str) == Some("vertical")
            && child
                .get("children")
                .and_then(Value::as_array)
                .map(|kids| {
                    kids.iter()
                        .any(|kid| kid.get("type").and_then(Value::as_str) == Some("text"))
                })
                .unwrap_or(false)
    });
    let Some(copy_idx) = copy_idx else {
        return;
    };
    let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) else {
        return;
    };
    let copy = &mut children[copy_idx];
    copy["width"] = json!("fill_container");
    copy["minWidth"] = json!(0);
    let Some(copy_children) = copy.get_mut("children").and_then(Value::as_array_mut) else {
        return;
    };
    for child in copy_children {
        if child.get("type").and_then(Value::as_str) != Some("text") {
            continue;
        }
        child["width"] = json!("fill_container");
        child["textGrowth"] = json!("fixed-width");
        if numeric_prop(child, "fontSize")
            .map(|size| size > 28.0)
            .unwrap_or(false)
        {
            child["fontSize"] = json!(28);
            child["lineHeight"] = json!(1.12);
        }
    }
}

// ── walk ──────────────────────────────────────────────────────────────────
