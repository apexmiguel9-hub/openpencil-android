//! Card image corner clipping, the notification-badge overlay fix and the
//! stacked-overlay-to-absolute conversion.

use super::*;

// ── clip_card_image_corners (port of the TS pass) ────────────────────────────
//
// A card-shape frame (scalar cornerRadius > 0) whose FIRST child is a
// full-width image carrying its OWN scalar cornerRadius renders the image's
// bottom corners rounded inside the card while the title below sits flush —
// the ragged look. Enforce `clipContent:true` on the card (its outer radius
// clips the image) + drop the image's own radius. Port of
// clip-card-image-corners.ts.
pub(super) fn is_image_node(node: &Value) -> bool {
    match node.get("type").and_then(Value::as_str) {
        Some("image") => true,
        Some("frame") => role_of(node) == Some("image-placeholder"),
        _ => false,
    }
}

/// A card's leading header image authored at a FIXED width narrower than the
/// card leaves a white gap beside it — a 160 px image inside a 252 px dish
/// card fills only ~63 % of the width. A vertical card's header image should
/// span the full card width, so set the leading image to
/// `width: fill_container`. The renderer's object-fit cover (parity with TS)
/// keeps the photo from distorting once it widens.
///
/// Scoped tightly to avoid touching avatars / logos / inline thumbnails: only
/// a LARGE (>= 80 px) leading image, in a `vertical` container that is NOT
/// center-aligned (centered leading images are avatars/logos, not full-bleed
/// headers), with at least one sibling below it.
pub(super) fn fill_card_leading_image_width(node: &mut Value) {
    let is_container = matches!(
        node.get("type").and_then(Value::as_str),
        Some("frame") | Some("group")
    );
    let is_vertical = node.get("layout").and_then(Value::as_str) == Some("vertical");
    let is_centered = node.get("alignItems").and_then(Value::as_str) == Some("center");
    if is_container && is_vertical && !is_centered && child_count(node) >= 2 {
        let lead_is_wide_fixed_image = children_of(node)
            .first()
            .map(|c| {
                is_image_node(c)
                    && c.get("width")
                        .and_then(Value::as_f64)
                        .map(|w| w >= 80.0)
                        .unwrap_or(false)
            })
            .unwrap_or(false);
        if lead_is_wide_fixed_image {
            if let Some(first) = node
                .get_mut("children")
                .and_then(Value::as_array_mut)
                .and_then(|a| a.first_mut())
                .and_then(Value::as_object_mut)
            {
                first.insert("width".to_string(), json!("fill_container"));
            }
        }
    }
    if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
        for child in children.iter_mut() {
            fill_card_leading_image_width(child);
        }
    }
}

// ── fix_notification_badge_overlay ───────────────────────────────────────

/// A small, roughly-square icon button whose two children are exactly one icon
/// glyph and one tiny filled dot — the notification-badge pattern.
pub(super) fn is_small_icon_button(node: &Value) -> bool {
    if node.get("type").and_then(Value::as_str) != Some("frame") {
        return false;
    }
    let (Some(w), Some(h)) = (
        node.get("width").and_then(Value::as_f64),
        node.get("height").and_then(Value::as_f64),
    ) else {
        return false;
    };
    w <= 56.0 && h <= 56.0 && (w - h).abs() <= 12.0 && child_count(node) == 2
}

pub(super) fn is_icon_glyph(node: &Value) -> bool {
    node.get("type").and_then(Value::as_str) == Some("icon_font") || is_image_node(node)
}

/// A tiny (<= 14 px), roughly-square, filled, childless shape — a badge dot.
pub(super) fn is_badge_dot(node: &Value) -> bool {
    if !matches!(
        node.get("type").and_then(Value::as_str),
        Some("frame") | Some("rectangle") | Some("ellipse")
    ) {
        return false;
    }
    let (Some(w), Some(h)) = (
        node.get("width").and_then(Value::as_f64),
        node.get("height").and_then(Value::as_f64),
    ) else {
        return false;
    };
    let has_fill = node
        .get("fill")
        .and_then(Value::as_array)
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    w <= 14.0 && h <= 14.0 && (w - h).abs() <= 4.0 && has_fill && children_of(node).is_empty()
}

/// A notification badge authored as a flex SIBLING of the icon inside a small
/// icon button renders BESIDE the icon (a square dot to its left, per the live
/// `Notification Button` = horizontal frame > [8×8 dot, bell]) instead of
/// overlaying the icon's top-right corner. Detect the pattern and convert it to
/// a corner-overlay badge: the button becomes `layout: none`, the icon is
/// centered, and the dot is rounded to a circle (square dots only — ellipses
/// are already round) and pinned to the icon's top-right corner.
pub(super) fn fix_notification_badge_overlay(node: &mut Value) {
    if is_small_icon_button(node) {
        let kids = children_of(node);
        let mut icon_i: Option<usize> = None;
        let mut dot_i: Option<usize> = None;
        for (i, c) in kids.iter().enumerate() {
            if is_icon_glyph(c) {
                icon_i = Some(i);
            } else if is_badge_dot(c) {
                dot_i = Some(i);
            }
        }
        if let (Some(ii), Some(di)) = (icon_i, dot_i) {
            let bw = node.get("width").and_then(Value::as_f64).unwrap_or(0.0);
            let bh = node.get("height").and_then(Value::as_f64).unwrap_or(0.0);
            let iw = kids[ii].get("width").and_then(Value::as_f64).unwrap_or(0.0);
            let ih = kids[ii]
                .get("height")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let dw = kids[di].get("width").and_then(Value::as_f64).unwrap_or(0.0);
            let icon_x = ((bw - iw) * 0.5).max(0.0);
            let icon_y = ((bh - ih) * 0.5).max(0.0);
            let dot_x = (icon_x + iw - dw * 0.5).min((bw - dw).max(0.0));
            let dot_y = (icon_y - dw * 0.5).max(0.0);
            if let Some(obj) = node.as_object_mut() {
                obj.insert("layout".to_string(), json!("none"));
                obj.remove("gap");
            }
            if let Some(arr) = node.get_mut("children").and_then(Value::as_array_mut) {
                if let Some(icon) = arr.get_mut(ii).and_then(Value::as_object_mut) {
                    icon.insert("x".to_string(), json!(icon_x));
                    icon.insert("y".to_string(), json!(icon_y));
                }
                if let Some(dot) = arr.get_mut(di).and_then(Value::as_object_mut) {
                    dot.insert("x".to_string(), json!(dot_x));
                    dot.insert("y".to_string(), json!(dot_y));
                    if dot.get("type").and_then(Value::as_str) != Some("ellipse") {
                        dot.insert("cornerRadius".to_string(), json!(dw * 0.5));
                    }
                }
            }
        }
    }
    if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
        for child in children.iter_mut() {
            fix_notification_badge_overlay(child);
        }
    }
}

pub(super) fn clip_card_image_corners(node: &mut Value) {
    let is_container = matches!(
        node.get("type").and_then(Value::as_str),
        Some("frame") | Some("group")
    );
    if is_container {
        let radius_ok = node
            .get("cornerRadius")
            .and_then(Value::as_f64)
            .map(|r| r > 0.0)
            .unwrap_or(false);
        let first_image_with_radius = children_of(node)
            .first()
            .map(|c| {
                is_image_node(c)
                    && c.get("cornerRadius")
                        .and_then(Value::as_f64)
                        .map(|r| r > 0.0)
                        .unwrap_or(false)
            })
            .unwrap_or(false);
        if radius_ok && child_count(node) >= 2 && first_image_with_radius {
            if let Some(obj) = node.as_object_mut() {
                obj.insert("clipContent".to_string(), json!(true));
            }
            if let Some(first) = node
                .get_mut("children")
                .and_then(Value::as_array_mut)
                .and_then(|a| a.first_mut())
                .and_then(Value::as_object_mut)
            {
                first.remove("cornerRadius");
            }
        }
    }
    if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
        for child in children.iter_mut() {
            clip_card_image_corners(child);
        }
    }
}

// ── convert_stacked_overlay_to_absolute (port of the TS pass) ────────────────
//
// Hero/banner overlay: a `layout:vertical` frame with a NUMERIC height whose
// children include ≥2 full-height bg-like nodes (a full-bleed image + a
// gradient-overlay rect/frame) that the model intends to LAYER, not stack.
// Vertical layout sequences them instead → the image + overlay collapse to
// h=0 and the content overflows the section (the broken "Featured Restaurants"
// promo card: Restaurant Image h=0, Card Details spilling below the section).
// Switch to `layout:none` so children stack at their own x/y (0,0 default),
// which is the layered intent. Conservative: requires EXPLICIT vertical +
// numeric height + ≥2 same-height/fill bg children (false positives are worse
// than misses). Port of convert-stacked-overlay-to-absolute.ts.
pub(super) fn convert_stacked_overlay_to_absolute(node: &mut Value) {
    if node.get("type").and_then(Value::as_str) == Some("frame")
        && node.get("layout").and_then(Value::as_str) == Some("vertical")
    {
        if let Some(h) = node.get("height").and_then(Value::as_f64) {
            let mut bg_like = 0;
            for child in children_of(node) {
                let t = child.get("type").and_then(Value::as_str);
                if !matches!(t, Some("image") | Some("rectangle") | Some("frame")) {
                    continue;
                }
                let ch = child.get("height");
                let matches_h = ch.and_then(Value::as_f64).map(|x| x == h).unwrap_or(false)
                    || ch.and_then(Value::as_str) == Some("fill_container");
                if matches_h {
                    bg_like += 1;
                }
            }
            if bg_like >= 2 {
                if let Some(obj) = node.as_object_mut() {
                    obj.insert("layout".to_string(), json!("none"));
                }
            }
        }
    }
    if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
        for child in children.iter_mut() {
            convert_stacked_overlay_to_absolute(child);
        }
    }
}

// ── forest entry ─────────────────────────────────────────────────────────────
