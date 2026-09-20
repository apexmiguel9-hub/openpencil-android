//! Opacity baking across fills, gradients, strokes and shadows, plus
//! the container-isolation rule.

use super::*;

#[test]
fn empty_editor_state_yields_empty_single_page_scene() {
    let scene = editor_state_to_layout_scene(&EditorState::new());
    // The loader's single-page fallback yields one empty page.
    assert_eq!(scene.pages.len(), 1);
    assert!(scene.pages[0].children.is_empty());
}

#[test]
fn node_opacity_bakes_into_resolved_fill_alpha() {
    // Node-level `opacity` must be folded into the resolved fill
    // alpha at scene-build time (TS parity: a 50%-opacity shape with
    // an opaque fill paints at 50% alpha). Mirrors the risk-monitor
    // pedestal, whose translucent stacked layers depend on it.
    let src = r##"{
      "version":"1.0.0",
      "pages":[{"id":"p","name":"P","children":[
        {"type":"rectangle","id":"r","x":0,"y":0,"width":10,"height":10,
         "opacity":0.5,
         "fill":[{"type":"solid","color":"#ffffff"}]}
      ]}],"children":[]
    }"##;
    let scene = editor_state_to_layout_scene(&state_from(src));
    let fill = scene.pages[0].children[0].fill.expect("rect has a fill");
    assert!(
        (fill.a - 0.5).abs() < 1e-3,
        "node opacity 0.5 should bake into fill alpha, got {}",
        fill.a
    );
}

#[test]
fn container_opacity_isolated_from_leaf_paint_opacity() {
    // The frame's 0.5 alpha applies once to its assembled subtree, while the
    // leaf keeps its own 0.5 direct-paint fast path. The visible result is
    // still 0.25, but overlapping siblings no longer accumulate frame alpha.
    let src = r##"{
      "version":"1.0.0",
      "pages":[{"id":"p","name":"P","children":[
        {"type":"frame","id":"f","x":0,"y":0,"width":100,"height":100,
         "opacity":0.5,
         "children":[
           {"type":"rectangle","id":"r","x":0,"y":0,"width":10,"height":10,
            "opacity":0.5,
            "fill":[{"type":"solid","color":"#ffffff"}]}
         ]}
      ]}],"children":[]
    }"##;
    let scene = editor_state_to_layout_scene(&state_from(src));
    let frame = &scene.pages[0].children[0];
    let child = frame
        .children
        .iter()
        .find(|n| n.id == "r")
        .expect("child r");
    let fill = child.fill.expect("rect fill");
    assert!((frame.opacity - 1.0).abs() < 1e-3);
    assert!((frame.composite_opacity - 0.5).abs() < 1e-3);
    assert!((child.opacity - 0.5).abs() < 1e-3);
    assert!((child.composite_opacity - 1.0).abs() < 1e-3);
    assert!(
        (fill.a - 0.5).abs() < 1e-3,
        "only the leaf's 0.5 opacity should bake into its fill, got {}",
        fill.a
    );
}

#[test]
fn node_opacity_scales_gradient_multiplier_not_stops() {
    // A 0.5-opacity node with a linear-gradient fill: node opacity
    // folds into the gradient's `opacity` multiplier only. The
    // backend already applies `opacity` to every stop when it builds
    // the shader, so scaling stop alpha here too would dim twice.
    let src = r##"{
      "version":"1.0.0",
      "pages":[{"id":"p","name":"P","children":[
        {"type":"rectangle","id":"r","x":0,"y":0,"width":10,"height":10,
         "opacity":0.5,
         "fill":[{"type":"linear_gradient","angle":0,"opacity":1,
                  "stops":[{"offset":0,"color":"#ffffff"},{"offset":1,"color":"#000000"}]}]}
      ]}],"children":[]
    }"##;
    let scene = editor_state_to_layout_scene(&state_from(src));
    match scene.pages[0].children[0]
        .gradient
        .as_ref()
        .expect("gradient")
    {
        SceneGradient::Linear { opacity, stops, .. } => {
            assert!(
                (opacity - 0.5).abs() < 1e-3,
                "gradient opacity should scale to 0.5, got {opacity}"
            );
            assert!(
                (stops[0].color.a - 1.0).abs() < 1e-3,
                "stop alpha must stay authored (1.0), got {}",
                stops[0].color.a
            );
        }
        _ => panic!("expected linear gradient"),
    }
}

#[test]
fn node_opacity_bakes_into_stroke_alpha() {
    // Node opacity dims the stroke alongside the fill.
    let src = r##"{
      "version":"1.0.0",
      "pages":[{"id":"p","name":"P","children":[
        {"type":"rectangle","id":"r","x":0,"y":0,"width":10,"height":10,
         "opacity":0.5,
         "stroke":{"thickness":2,"fill":[{"type":"solid","color":"#ff0000"}]}}
      ]}],"children":[]
    }"##;
    let scene = editor_state_to_layout_scene(&state_from(src));
    let stroke = scene.pages[0].children[0].stroke.expect("stroke");
    assert!(
        (stroke.color.a - 0.5).abs() < 1e-3,
        "node opacity should bake into stroke alpha, got {}",
        stroke.color.a
    );
}

#[test]
fn node_opacity_dims_drop_shadow_color() {
    // A drop shadow is part of the node's paint, so node opacity dims
    // it too (opaque-black shadow on a 0.5 node → 0.5 alpha).
    let src = r##"{
      "version":"1.0.0",
      "pages":[{"id":"p","name":"P","children":[
        {"type":"rectangle","id":"r","x":0,"y":0,"width":10,"height":10,
         "opacity":0.5,
         "fill":[{"type":"solid","color":"#ffffff"}],
         "effects":[{"type":"shadow","offsetX":2,"offsetY":4,"blur":8,"spread":0,"color":"#000000"}]}
      ]}],"children":[]
    }"##;
    let scene = editor_state_to_layout_scene(&state_from(src));
    let effects = &scene.pages[0].children[0].effects;
    assert_eq!(effects.len(), 1, "expected one drop shadow");
    let Effect::DropShadow(s) = &effects[0] else {
        panic!("expected a drop shadow effect");
    };
    assert!(
        (s.color.a - 0.5).abs() < 1e-3,
        "node opacity should dim shadow alpha, got {}",
        s.color.a
    );
}

#[test]
fn inner_shadow_flag_survives_into_scene() {
    // An `inner: true` shadow in the .op must reach the scene as an
    // inset shadow (the risk-monitor pedestal's top hexagon uses one
    // for its recessed glow).
    let src = r##"{
      "version":"1.0.0",
      "pages":[{"id":"p","name":"P","children":[
        {"type":"rectangle","id":"r","x":0,"y":0,"width":10,"height":10,
         "fill":[{"type":"solid","color":"#ffffff"}],
         "effects":[{"type":"shadow","inner":true,"offsetX":0,"offsetY":0,"blur":4,"spread":0,"color":"#000000"}]}
      ]}],"children":[]
    }"##;
    let scene = editor_state_to_layout_scene(&state_from(src));
    let effects = &scene.pages[0].children[0].effects;
    assert_eq!(effects.len(), 1, "expected one shadow");
    let Effect::DropShadow(s) = &effects[0] else {
        panic!("expected a drop shadow effect");
    };
    assert!(s.inner, "inner:true must survive into the scene effect");
}
