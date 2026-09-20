//! Tests for the fill / stroke / effect helpers.
//!
//! Split out of the `fills` spine (800-line file ceiling); the older
//! sibling suite lives in `crate::fills_tests`.

use super::*;

/// A bare rectangle node fixture, parsed from `.op` JSON so it
/// stays robust to schema growth.
fn rect_node() -> PenNode {
    let src = r#"{"version":"1.0.0","children":[
        {"type":"rectangle","id":"r1","name":"R",
         "x":0,"y":0,"width":10,"height":10}
    ]}"#;
    jian_ops_schema::load_str(src)
        .expect("fixture parses")
        .value
        .children
        .into_iter()
        .next()
        .expect("one node")
}

/// Seed a node with a custom 3-stop linear gradient as its first
/// fill so the conversion has a non-default body to preserve.
fn seed_linear_gradient(node: &mut PenNode) {
    let fills = node_fills_mut(node).expect("rect carries fills");
    fills.clear();
    fills.push(PenFill::LinearGradient(LinearGradientBody {
        angle: Some(45.0),
        stops: vec![
            GradientStop {
                offset: 0.0,
                color: "#ff0000".into(),
            },
            GradientStop {
                offset: 0.5,
                color: "#00ff00".into(),
            },
            GradientStop {
                offset: 1.0,
                color: "#0000ff".into(),
            },
        ],
        explain: None,
        opacity: Some(0.75),
        blend_mode: None,
    }));
}

#[test]
fn first_solid_stroke_opacity_reads_body_opacity_or_defaults() {
    let mut node = rect_node();
    // No stroke at all → opaque default.
    assert_eq!(first_solid_stroke_opacity(&node), 1.0);
    // A stroke whose body opacity is unset → still 1.0.
    assert!(set_primary_stroke_hex(&mut node, "#112233"));
    assert_eq!(first_solid_stroke_opacity(&node), 1.0);
    // Author a sub-100% stroke body opacity → reported verbatim, so a
    // live paint patch can reproduce the loader's baked stroke alpha.
    if let Some(Some(stroke)) = node_stroke_mut(&mut node) {
        if let Some(PenFill::Solid(b)) = stroke.fill.as_mut().and_then(|f| f.first_mut()) {
            b.opacity = Some(0.4);
        }
    }
    assert_eq!(first_solid_stroke_opacity(&node), 0.4);
}

#[test]
fn linear_to_radial_preserves_the_gradient_body() {
    // Fix 6: a fill-type discriminant change must not discard the
    // existing gradient payload — shell-core's `set_selected_fill_type`
    // only flipped a scalar `Node.fill_type` and kept the body, so
    // the canonical port carries the stops / opacity across.
    let mut node = rect_node();
    seed_linear_gradient(&mut node);

    assert!(set_primary_fill_type(&mut node, FillType::RadialGradient));

    let fills = node_fills(&node).expect("rect carries fills");
    match fills.first().expect("a first fill") {
        PenFill::RadialGradient(body) => {
            // The full 3-stop list survived the variant flip.
            assert_eq!(body.stops.len(), 3);
            assert_eq!(body.stops[0].color, "#ff0000");
            assert_eq!(body.stops[1].color, "#00ff00");
            assert_eq!(body.stops[2].color, "#0000ff");
            // Opacity carried across too — not reset to default.
            assert_eq!(body.opacity, Some(0.75));
        }
        other => panic!("expected RadialGradient, got {other:?}"),
    }
}

#[test]
fn flipping_back_and_forth_keeps_the_stops() {
    // Linear → Radial → Linear round-trip must still carry the
    // custom stops (angle has no radial counterpart, so it is the
    // one field allowed to drop).
    let mut node = rect_node();
    seed_linear_gradient(&mut node);

    assert!(set_primary_fill_type(&mut node, FillType::RadialGradient));
    assert!(set_primary_fill_type(&mut node, FillType::LinearGradient));

    let fills = node_fills(&node).expect("rect carries fills");
    match fills.first().expect("a first fill") {
        PenFill::LinearGradient(body) => {
            assert_eq!(body.stops.len(), 3);
            assert_eq!(body.stops[0].color, "#ff0000");
            assert_eq!(body.opacity, Some(0.75));
        }
        other => panic!("expected LinearGradient, got {other:?}"),
    }
}

#[test]
fn same_type_is_a_no_op_keeping_the_exact_body() {
    // Setting the type the node already has must leave the body
    // byte-for-byte identical (no default-body overwrite).
    let mut node = rect_node();
    seed_linear_gradient(&mut node);
    let before = node_fills(&node).unwrap().first().cloned();

    assert!(set_primary_fill_type(&mut node, FillType::LinearGradient));

    let after = node_fills(&node).unwrap().first().cloned();
    assert_eq!(before, after);
}

#[test]
fn solid_to_gradient_seeds_stops_from_the_solid_colour() {
    // Cross-family flip: there is no gradient body to carry, so the
    // representative colour seeds the first stop.
    let mut node = rect_node();
    {
        let fills = node_fills_mut(&mut node).unwrap();
        fills.clear();
        fills.push(solid_fill("#abcdef".into()));
    }
    assert!(set_primary_fill_type(&mut node, FillType::LinearGradient));
    match node_fills(&node).unwrap().first().unwrap() {
        PenFill::LinearGradient(body) => {
            assert_eq!(
                body.stops.first().map(|s| s.color.as_str()),
                Some("#abcdef")
            );
        }
        other => panic!("expected LinearGradient, got {other:?}"),
    }
}

#[test]
fn indexed_opacity_updates_mesh_and_shader_fills() {
    let mut node = rect_node();
    let fills = node_fills_mut(&mut node).expect("rect carries fills");
    fills.clear();
    fills.push(default_fill_of_type(FillType::MeshGradient, "#112233"));
    fills.push(default_fill_of_type(FillType::Shader, "#445566"));

    assert!(set_fill_opacity_at(&mut node, 0, 0.35));
    assert!(set_fill_opacity_at(&mut node, 1, 0.65));

    let fills = node_fills(&node).expect("rect carries fills");
    assert!(matches!(
        &fills[0],
        PenFill::MeshGradient(body) if body.opacity == Some(0.35)
    ));
    assert!(matches!(
        &fills[1],
        PenFill::Shader(body) if body.opacity == Some(0.65)
    ));
}

#[test]
fn transformed_legacy_stretch_summary_reports_crop_preview_payload() {
    let src = r#"{"version":"1.0.0","children":[{
        "type":"rectangle","id":"crop","name":"Legacy crop",
        "x":0,"y":0,"width":191,"height":236,
        "fill":[{"type":"image","url":"data:image/png;base64,AA==",
            "mode":"stretch",
            "originalSize":{"width":1179,"height":2556},
            "transform":{"m00":0.5089059,"m01":0.0,"m02":0.490246,
                "m10":0.0,"m11":0.28951487,"m12":0.37636933}}]
    }]}"#;
    let node = jian_ops_schema::load_str(src)
        .expect("legacy crop fixture parses")
        .value
        .children
        .into_iter()
        .next()
        .expect("one node");

    let summary = first_image_fill_summary(&node).expect("image summary");
    assert_eq!(summary.mode, ImageFillMode::Crop);
    assert_eq!(
        summary.transform,
        Some([0.5089059, 0.0, 0.490246, 0.0, 0.28951487, 0.37636933])
    );
    assert_eq!(summary.original_size, Some([1179.0, 2556.0]));
    assert_eq!(summary.tile_scale, Some(1.0));
}

#[test]
fn untransformed_stretch_summary_keeps_historical_fill_fallback() {
    let src = r#"{"version":"1.0.0","children":[{
        "type":"rectangle","id":"stretch","name":"Stretch",
        "x":0,"y":0,"width":10,"height":10,
        "fill":[{"type":"image","url":"data:image/png;base64,AA==",
            "mode":"stretch",
            "transform":{"m00":1.0,"m01":0.0,"m02":0.0,
                "m10":0.0,"m11":1.0,"m12":0.0}}]
    }]}"#;
    let node = jian_ops_schema::load_str(src)
        .expect("stretch fixture parses")
        .value
        .children
        .into_iter()
        .next()
        .expect("one node");

    let summary = first_image_fill_summary(&node).expect("image summary");
    assert_eq!(summary.mode, ImageFillMode::Fill);
    assert_eq!(summary.transform, Some([1.0, 0.0, 0.0, 0.0, 1.0, 0.0]));
    assert_eq!(summary.original_size, None);
}
