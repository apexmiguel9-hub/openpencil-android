use crate::kiwi::FigValue;
use crate::text_cache::compact_transient_text_caches;
use crate::tree::guid_to_string;
use std::collections::HashMap;

/// Copy style-node values inline onto the nodes that reference them,
/// so downstream converters never chase a style ref. Mutates the
/// node-change list in place.
pub fn resolve_style_references(node_changes: &mut [FigValue]) {
    crate::component_props::resolve_component_property_swaps(node_changes);
    let mut style_map: HashMap<String, FigValue> = HashMap::new();
    for nc in node_changes.iter() {
        if nc.get("styleType").is_some() {
            if let Some(key) = nc.get("guid").and_then(guid_to_string) {
                style_map.insert(key, nc.clone());
            }
            // Library styles are referenced by `assetRef.key` — index
            // the embedded style node under its publish key too. The
            // "key:" namespace keeps hex publish keys from ever
            // colliding with "session:local" guid strings.
            if let Some(asset_key) = nc.get_str("key") {
                if !asset_key.is_empty() {
                    style_map.insert(format!("key:{asset_key}"), nc.clone());
                }
            }
        }
    }
    if style_map.is_empty() {
        compact_transient_text_caches(node_changes);
        return;
    }
    for nc in node_changes.iter_mut() {
        resolve_on_node(nc, &style_map, TextStylePriority::DerivedMetrics);
        // Resolve style refs inside instance symbol overrides too.
        if let Some(overrides) = nc
            .get_mut("symbolData")
            .and_then(|data| data.get_array_mut("symbolOverrides"))
        {
            for ov in overrides {
                resolve_on_node(ov, &style_map, TextStylePriority::ExplicitFields);
            }
        }
    }
    compact_transient_text_caches(node_changes);
}

fn lookup_style<'a>(
    nc: &FigValue,
    field: &str,
    style_map: &'a HashMap<String, FigValue>,
) -> Option<&'a FigValue> {
    let sid = nc.get(field)?;
    // Local style: guid reference. Library style: assetRef publish key.
    if let Some(key) = sid.get("guid").and_then(guid_to_string) {
        return style_map.get(&key);
    }
    let asset_key = sid.get("assetRef").and_then(|a| a.get_str("key"))?;
    style_map.get(&format!("key:{asset_key}"))
}

pub(super) fn non_empty_array(v: &FigValue, key: &str) -> bool {
    v.get_array(key).map(|a| !a.is_empty()).unwrap_or(false)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TextStylePriority {
    DerivedMetrics,
    ExplicitFields,
}

fn nearly_equal(a: f64, b: f64) -> bool {
    (a - b).abs() <= 0.001
}

fn uniform_derived_metric(node: &FigValue, entries: &str, field: &str) -> Option<f64> {
    let values = node.get("derivedTextData")?.get_array(entries)?;
    let first = values.first()?.get_f64(field)?;
    values
        .iter()
        .all(|value| value.get_f64(field).is_some_and(|v| nearly_equal(v, first)))
        .then_some(first)
}

fn effective_line_height_px(text: &FigValue) -> Option<f64> {
    let line_height = text.get("lineHeight")?;
    let value = line_height.get_f64("value")?;
    match line_height.get_str("units")? {
        "PIXELS" => Some(value),
        "PERCENT" => Some(text.get_f64("fontSize")? * value / 100.0),
        "RAW" => Some(text.get_f64("fontSize")? * value),
        _ => None,
    }
}

/// Direct text nodes can carry stale top-level metrics even though
/// their rendered `derivedTextData` reflects the linked text style.
/// Only replace a present metric when the derived data is uniform and
/// independently confirms the style; this preserves legitimate local
/// overrides that retain a `styleIdForText` reference.
fn derived_confirms_text_style(node: &FigValue, style: &FigValue, field: &str) -> bool {
    let (current, referenced, derived) = match field {
        "fontSize" => (
            node.get_f64("fontSize"),
            style.get_f64("fontSize"),
            uniform_derived_metric(node, "glyphs", "fontSize"),
        ),
        "lineHeight" => (
            effective_line_height_px(node),
            effective_line_height_px(style),
            uniform_derived_metric(node, "baselines", "lineHeight"),
        ),
        _ => return false,
    };
    matches!(
        (current, referenced, derived),
        (Some(current), Some(referenced), Some(derived))
            if !nearly_equal(current, referenced) && nearly_equal(derived, referenced)
    )
}

fn resolve_on_node(
    nc: &mut FigValue,
    style_map: &HashMap<String, FigValue>,
    text_priority: TextStylePriority,
) {
    // FILL.
    if let Some(fs) = lookup_style(nc, "styleIdForFill", style_map) {
        if non_empty_array(fs, "fillPaints") {
            if let Some(v) = fs.get("fillPaints").cloned() {
                nc.set("fillPaints", v);
            }
        }
    }
    // STROKE — reads the style node's fillPaints, writes strokePaints.
    if let Some(ss) = lookup_style(nc, "styleIdForStrokeFill", style_map) {
        if non_empty_array(ss, "fillPaints") {
            if let Some(v) = ss.get("fillPaints").cloned() {
                nc.set("strokePaints", v);
            }
        }
    }
    // Direct nodes occasionally carry stale cached text metrics. Use
    // the linked style only when derived glyph/baseline data confirms
    // it; instance overrides always keep explicit authored fields.
    if let Some(ts) = lookup_style(nc, "styleIdForText", style_map).cloned() {
        let replace_font_size = text_priority == TextStylePriority::DerivedMetrics
            && derived_confirms_text_style(nc, &ts, "fontSize");
        let replace_line_height = text_priority == TextStylePriority::DerivedMetrics
            && derived_confirms_text_style(nc, &ts, "lineHeight");
        for key in [
            "fontName",
            "fontSize",
            "lineHeight",
            "letterSpacing",
            "textAlignHorizontal",
            "textAlignVertical",
            "textDecoration",
            "textCase",
        ] {
            let replace_metric = match key {
                "fontSize" => replace_font_size,
                "lineHeight" => replace_line_height,
                _ => false,
            };
            if replace_metric || nc.get(key).is_none() {
                if let Some(v) = ts.get(key).cloned() {
                    nc.set(key, v);
                }
            }
        }
        // A text style's cached paint is a fallback for direct TEXT nodes.
        // On a symbol override, `styleIdForText` only authors typography;
        // synthesizing `fillPaints` here would overwrite the target variant's
        // independent `styleIdForFill` (for example selected-menu blue).
        if text_priority == TextStylePriority::DerivedMetrics
            && !non_empty_array(nc, "fillPaints")
            && non_empty_array(&ts, "fillPaints")
        {
            if let Some(v) = ts.get("fillPaints").cloned() {
                nc.set("fillPaints", v);
            }
        }
    }
    // EFFECT.
    if let Some(es) = lookup_style(nc, "styleIdForEffect", style_map) {
        if non_empty_array(es, "effects") && !non_empty_array(nc, "effects") {
            if let Some(v) = es.get("effects").cloned() {
                nc.set("effects", v);
            }
        }
    }
}
