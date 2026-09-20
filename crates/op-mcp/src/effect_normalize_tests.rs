//! Unit tests for the `effects` normalizer. The end-to-end cascade
//! regression (a shadow missing `spread` taking its whole subtree down) lives
//! in `batch_program_repair_tests.rs`, where the program executor is driven.

use super::*;

fn normalized(node: serde_json::Value) -> serde_json::Value {
    let mut value = node;
    let obj = value.as_object_mut().expect("object");
    normalize_node_effects(obj);
    value
}

fn effect(node: &serde_json::Value) -> &serde_json::Value {
    &node["effects"][0]
}

/// Deserialize the way the real parse path does — ids are stamped first, so
/// these assertions test the effect repair and not the id plumbing.
fn parse_node(
    node: serde_json::Value,
) -> Result<jian_ops_schema::node::PenNode, serde_json::Error> {
    let mut value = node;
    let mut next = 1usize;
    crate::batch_design::ensure_node_ids(&mut value, &mut next);
    serde_json::from_value(value)
}

#[test]
fn shadow_missing_spread_gains_the_css_identity_default() {
    let out = normalized(serde_json::json!({
        "type": "frame",
        "effects": [{ "type": "shadow", "offsetX": 0, "offsetY": 4, "blur": 12, "color": "#00000014" }],
    }));
    let shadow = effect(&out);
    assert_eq!(shadow["spread"], serde_json::json!(0));
    // The authored values survive untouched — the repair only fills the gap.
    assert_eq!(shadow["offsetY"], serde_json::json!(4));
    assert_eq!(shadow["blur"], serde_json::json!(12));
    assert_eq!(shadow["color"], serde_json::json!("#00000014"));
    let parsed = parse_node(out).expect("deserializes");
    assert!(matches!(parsed, jian_ops_schema::node::PenNode::Frame(_)));
}

#[test]
fn shadow_missing_every_required_field_still_deserializes() {
    let out = normalized(serde_json::json!({
        "type": "rectangle",
        "width": 10,
        "height": 10,
        "effects": [{ "type": "shadow" }],
    }));
    let shadow = effect(&out);
    for key in ["offsetX", "offsetY", "blur", "spread"] {
        assert_eq!(
            shadow[key],
            serde_json::json!(0),
            "{key} defaults to identity"
        );
    }
    assert_eq!(shadow["color"], serde_json::json!(DEFAULT_SHADOW_COLOR));
    parse_node(out).expect("deserializes");
}

#[test]
fn shadow_aliases_and_numeric_strings_are_coerced() {
    let out = normalized(serde_json::json!({
        "type": "frame",
        "effects": [{
            "type": "drop-shadow",
            "x": 0, "y": "8px", "blurRadius": "24", "spreadRadius": -2,
            "colour": "#00000033",
        }],
    }));
    let shadow = effect(&out);
    assert_eq!(shadow["offsetX"], serde_json::json!(0));
    assert_eq!(shadow["offsetY"], serde_json::json!(8));
    assert_eq!(shadow["blur"], serde_json::json!(24));
    assert_eq!(shadow["spread"], serde_json::json!(-2));
    assert_eq!(shadow["color"], serde_json::json!("#00000033"));
    // Every alias is consumed, so no stray key is left behind.
    for alias in ["x", "y", "blurRadius", "spreadRadius", "colour"] {
        assert!(shadow.get(alias).is_none(), "{alias} must be consumed");
    }
    parse_node(out).expect("deserializes");
}

#[test]
fn shadow_color_is_read_out_of_a_fill_array() {
    let out = normalized(serde_json::json!({
        "type": "frame",
        "effects": [{ "type": "shadow", "offsetY": 2, "blur": 4, "fill": [{ "type": "solid", "color": "#112233" }] }],
    }));
    assert_eq!(effect(&out)["color"], serde_json::json!("#112233"));
}

#[test]
fn a_single_effect_object_is_wrapped_into_the_schema_array() {
    let out = normalized(serde_json::json!({
        "type": "frame",
        "effects": { "type": "shadow", "offsetY": 4, "blur": 8 },
    }));
    assert!(out["effects"].is_array());
    assert_eq!(effect(&out)["spread"], serde_json::json!(0));
    parse_node(out).expect("deserializes");
}

#[test]
fn singular_effect_key_is_renamed_to_the_schema_field() {
    // `effect` is an unknown key: serde drops it without a word, so the
    // shadow disappears silently instead of failing loudly.
    let out = normalized(serde_json::json!({
        "type": "frame",
        "effect": [{ "type": "shadow", "offsetY": 4, "blur": 8 }],
    }));
    assert!(out.get("effect").is_none());
    assert_eq!(effect(&out)["type"], serde_json::json!("shadow"));
}

#[test]
fn glow_and_inner_shadow_spellings_route_to_the_shadow_variant() {
    let out = normalized(serde_json::json!({
        "type": "frame",
        "effects": [
            { "type": "glow", "blur": 20, "color": "#A855F766" },
            { "type": "inner-shadow", "offsetY": 2, "blur": 4, "color": "#00000022" },
        ],
    }));
    assert_eq!(out["effects"][0]["type"], serde_json::json!("shadow"));
    assert_eq!(out["effects"][1]["type"], serde_json::json!("shadow"));
    assert_eq!(out["effects"][1]["inner"], serde_json::json!(true));
    parse_node(out).expect("deserializes");
}

#[test]
fn blur_variants_gain_their_required_radius() {
    let out = normalized(serde_json::json!({
        "type": "frame",
        "effects": [
            { "type": "blur", "amount": 10 },
            { "type": "backdrop-blur" },
        ],
    }));
    assert_eq!(out["effects"][0]["radius"], serde_json::json!(10));
    assert_eq!(
        out["effects"][1]["type"],
        serde_json::json!("background_blur")
    );
    assert_eq!(out["effects"][1]["radius"], serde_json::json!(0));
    parse_node(out).expect("deserializes");
}

#[test]
fn a_typeless_effect_body_is_classified_by_its_fields() {
    let out = normalized(serde_json::json!({
        "type": "frame",
        "effects": [{ "offsetY": 4, "blur": 8, "color": "#00000020" }],
    }));
    assert_eq!(effect(&out)["type"], serde_json::json!("shadow"));
    parse_node(out).expect("deserializes");
}

#[test]
fn string_flags_are_coerced_and_junk_flags_are_dropped() {
    let out = normalized(serde_json::json!({
        "type": "frame",
        "effects": [{ "type": "shadow", "offsetY": 4, "blur": 8, "visible": "true", "inner": "maybe" }],
    }));
    assert_eq!(effect(&out)["visible"], serde_json::json!(true));
    assert!(effect(&out).get("inner").is_none());
    parse_node(out).expect("deserializes");
}

#[test]
fn an_unknown_effect_kind_is_left_exactly_as_authored() {
    // The fallback is deliberate: inventing a variant for a name we don't
    // recognise would paint something the model never asked for. The node
    // still fails loudly, which is the documented tail behaviour.
    let out = normalized(serde_json::json!({
        "type": "frame",
        "effects": [{ "type": "noise", "intensity": 3 }],
    }));
    assert_eq!(effect(&out)["type"], serde_json::json!("noise"));
    assert_eq!(effect(&out)["intensity"], serde_json::json!(3));
    parse_node(out).expect_err("a genuinely unknown effect still fails");
}

#[test]
fn a_well_formed_effect_is_byte_for_byte_unchanged() {
    let authored = serde_json::json!({
        "type": "frame",
        "effects": [{ "type": "shadow", "offsetX": 0, "offsetY": 4, "blur": 12, "spread": 0, "color": "#00000014" }],
    });
    assert_eq!(normalized(authored.clone()), authored);
}

#[test]
fn nested_children_effects_are_normalized_too() {
    // The node-shape pass recurses into `children`; this pins that the
    // effects repair rides along with it.
    let mut value = serde_json::json!({
        "type": "frame",
        "children": [{
            "type": "rectangle", "width": 4, "height": 4,
            "effects": [{ "type": "shadow", "offsetY": 4, "blur": 8 }],
        }],
    });
    crate::batch_design::normalize_node_shape(&mut value);
    assert_eq!(
        value["children"][0]["effects"][0]["spread"],
        serde_json::json!(0)
    );
    parse_node(value).expect("deserializes");
}
