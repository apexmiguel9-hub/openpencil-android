//! Lenient normalization for a node's `effects` array.
//!
//! `PenEffect` is an internally-tagged enum whose bodies carry REQUIRED
//! fields — `ShadowBody` needs `offsetX` / `offsetY` / `blur` / `spread` /
//! `color`, `BlurBody` needs `radius` — so a model that writes a shadow and
//! forgets one of them fails the WHOLE node, not just its decoration. In the
//! `I(parent, node)` program DSL that failure CASCADES: the rejected node
//! never gets a binding, so every subsequent line targeting it as a parent
//! dies with "Insert parent not found", and an entire card silently
//! disappears from the design. (Measured 2026-07-28: one missing `spread` on
//! a "Challenge Card" frame took the card plus 19 descendant lines with it.)
//!
//! Every default injected here is the identity value for the property — the
//! same value CSS uses when the author omits it, so filling it in cannot
//! change how a well-formed effect renders. `color` is the one property with
//! no identity, and its default is the neutral 25%-black every skill example
//! already uses.

use serde_json::{Map, Value};

/// Neutral shadow tint used when a model omits `color`. Matches the value the
/// skill corpus teaches, so a recovered shadow looks like an authored one.
const DEFAULT_SHADOW_COLOR: &str = "#00000040";

/// Which `PenEffect` variant an authored effect object means.
#[derive(Clone, Copy)]
enum EffectKind {
    /// `inner` carries the inner-shadow / inner-glow spelling forward.
    Shadow {
        inner: bool,
    },
    Blur,
    BackgroundBlur,
}

/// Normalize the `effects` field of a node object in place.
///
/// Also accepts the singular `effect` spelling: the schema field is `effects`,
/// and serde drops the singular key without a word, so a model writing
/// `effect` loses its shadow silently rather than loudly.
pub fn normalize_node_effects(obj: &mut Map<String, Value>) {
    if !obj.contains_key("effects") {
        if let Some(value) = obj.remove("effect") {
            obj.insert("effects".into(), value);
        }
    }
    if let Some(effects) = obj.get_mut("effects") {
        normalize_effects(effects);
    }
}

/// Normalize an `effects` value: accept a single effect object (or a bare
/// kind name) where the schema wants an array, then repair each entry.
pub fn normalize_effects(value: &mut Value) {
    match value {
        Value::Object(_) | Value::String(_) => {
            let single = std::mem::take(value);
            *value = Value::Array(vec![single]);
        }
        Value::Array(_) => {}
        _ => return,
    }
    let Value::Array(items) = value else {
        return;
    };
    for item in items {
        normalize_effect(item);
    }
}

/// Repair one effect entry: canonicalize its `type`, then fill the required
/// body fields its variant needs.
fn normalize_effect(value: &mut Value) {
    // A bare kind name (`"effects": ["shadow"]`) becomes an empty object of
    // that kind; the body pass below fills every required field.
    if let Value::String(name) = value {
        let name = name.clone();
        *value = serde_json::json!({ "type": name });
    }
    let Value::Object(obj) = value else {
        return;
    };
    // An unrecognized kind is left exactly as authored — guessing a variant
    // for it would invent a look the model never asked for, and the node
    // still fails loudly rather than rendering something wrong.
    let Some(kind) = canonical_kind(obj) else {
        return;
    };
    let tag = match kind {
        EffectKind::Shadow { .. } => "shadow",
        EffectKind::Blur => "blur",
        EffectKind::BackgroundBlur => "background_blur",
    };
    obj.insert("type".into(), Value::String(tag.into()));
    match kind {
        EffectKind::Shadow { inner } => {
            if inner {
                obj.insert("inner".into(), Value::Bool(true));
            }
            normalize_shadow_body(obj);
        }
        EffectKind::Blur | EffectKind::BackgroundBlur => normalize_blur_body(obj),
    }
}

/// Resolve the effect's variant from its `type`, inferring one from the body
/// shape when `type` is missing entirely.
fn canonical_kind(obj: &Map<String, Value>) -> Option<EffectKind> {
    let Some(raw) = obj.get("type").and_then(Value::as_str) else {
        return infer_kind(obj);
    };
    let canon: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    // Background/backdrop blur must be tested before the plain `blur`
    // substring, which it also contains.
    if canon.contains("background") || canon.contains("backdrop") {
        return Some(EffectKind::BackgroundBlur);
    }
    // A glow IS a shadow — zero offset, wide blur, tinted colour — and the
    // dark style guides describe their accents in exactly those words, so
    // models reach for the name. Routing it to `shadow` keeps the intent.
    if canon.contains("shadow") || canon.contains("glow") {
        return Some(EffectKind::Shadow {
            inner: canon.contains("inner"),
        });
    }
    if canon.contains("blur") {
        return Some(EffectKind::Blur);
    }
    None
}

/// Infer the variant from which fields the object carries. Only fires when
/// `type` is absent — a present-but-unknown `type` is never overridden.
fn infer_kind(obj: &Map<String, Value>) -> Option<EffectKind> {
    const SHADOW_ONLY: [&str; 9] = [
        "offsetX",
        "offsetY",
        "offset_x",
        "offset_y",
        "spread",
        "spreadRadius",
        "dx",
        "dy",
        "color",
    ];
    if SHADOW_ONLY.iter().any(|key| obj.contains_key(*key)) {
        return Some(EffectKind::Shadow { inner: false });
    }
    if obj.contains_key("radius") || obj.contains_key("blur") {
        return Some(EffectKind::Blur);
    }
    None
}

/// Fill `ShadowBody`'s five required fields, accepting the aliases models
/// borrow from CSS and Figma.
fn normalize_shadow_body(obj: &mut Map<String, Value>) {
    ensure_number(obj, "offsetX", &["offset_x", "offsetx", "x", "dx"], 0.0);
    ensure_number(obj, "offsetY", &["offset_y", "offsety", "y", "dy"], 0.0);
    ensure_number(obj, "blur", &["blurRadius", "blur_radius", "radius"], 0.0);
    ensure_number(obj, "spread", &["spreadRadius", "spread_radius"], 0.0);
    let mut color = None;
    for alias in ["color", "colour", "shadowColor", "shadow_color", "fill"] {
        let candidate = obj.remove(alias);
        if color.is_none() {
            color = candidate.as_ref().and_then(extract_color);
        }
    }
    obj.insert(
        "color".into(),
        Value::String(color.unwrap_or_else(|| DEFAULT_SHADOW_COLOR.to_string())),
    );
    normalize_flag(obj, "inner");
    normalize_flag(obj, "visible");
}

/// Fill `BlurBody`'s required `radius`.
fn normalize_blur_body(obj: &mut Map<String, Value>) {
    ensure_number(
        obj,
        "radius",
        &["blur", "blurRadius", "amount", "size"],
        0.0,
    );
    normalize_flag(obj, "visible");
}

/// Write `key` as a JSON number, taking the first alias present and coercing
/// a numeric string (`"8"` / `"8px"`) on the way. Every alias is consumed so
/// none survives as a stray key.
fn ensure_number(obj: &mut Map<String, Value>, key: &str, aliases: &[&str], default: f64) {
    let mut found = obj.remove(key).as_ref().and_then(as_number);
    for alias in aliases {
        let candidate = obj.remove(*alias);
        if found.is_none() {
            found = candidate.as_ref().and_then(as_number);
        }
    }
    let value = found.unwrap_or(default);
    let number = if value.fract() == 0.0 && value.is_finite() {
        serde_json::json!(value as i64)
    } else {
        serde_json::json!(value)
    };
    obj.insert(key.into(), number);
}

/// Coerce an optional boolean field. A string `"true"` or a 0/1 number fails
/// `Option<bool>` as hard as a missing required field does; an
/// uninterpretable value is dropped so the schema default applies.
fn normalize_flag(obj: &mut Map<String, Value>, key: &str) {
    let Some(value) = obj.get(key) else {
        return;
    };
    let coerced = match value {
        Value::Bool(_) => return,
        Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
            "true" | "yes" | "1" => Some(true),
            "false" | "no" | "0" => Some(false),
            _ => None,
        },
        Value::Number(number) => number.as_f64().map(|n| n != 0.0),
        _ => None,
    };
    match coerced {
        Some(flag) => {
            obj.insert(key.into(), Value::Bool(flag));
        }
        None => {
            obj.remove(key);
        }
    }
}

fn as_number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => {
            let trimmed = text.trim().trim_end_matches("px").trim();
            trimmed.parse::<f64>().ok()
        }
        _ => None,
    }
}

/// Pull a colour string out of the shapes a model writes it in: a bare
/// string, a fill array, or a `{type,color}` fill object.
fn extract_color(value: &Value) -> Option<String> {
    match value {
        Value::String(text) if !text.trim().is_empty() => Some(text.trim().to_string()),
        Value::Array(items) => items.iter().find_map(extract_color),
        Value::Object(map) => map.get("color").and_then(extract_color),
        _ => None,
    }
}

#[cfg(test)]
#[path = "effect_normalize_tests.rs"]
mod tests;
