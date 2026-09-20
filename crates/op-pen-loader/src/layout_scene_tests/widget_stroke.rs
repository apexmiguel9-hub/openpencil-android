//! A widget stroke that resolves to no paint must not reach the scene as
//! opaque black.
//!
//! `stroke_to_payload` used to fabricate `[0, 0, 0, 1]` whenever the authored
//! stroke had no resolvable solid colour. The design canvas feeds
//! `node.stroke.color` into `resolve_authored_widget_visual` as the *inactive
//! track / border* role, so every control whose author wrote
//! `"stroke": {"thickness": 1}` without a `fill` painted a pure black switch
//! track / select border — glaring on a dark theme, and a regression against
//! the `#D1D5DB` constant the widget painters used before they became
//! token-driven. The new prompt requires controls to carry a stroke, so a
//! missing `stroke.fill` is a high-probability weak-model slip, not an
//! exotic input.

use super::*;

fn scene_node(src: &str, id: &str) -> SceneNode {
    let scene = editor_state_to_layout_scene(&state_from(src));
    scene.pages[0]
        .children
        .iter()
        .find(|n| n.id == id)
        .unwrap_or_else(|| panic!("scene node {id:?}"))
        .clone()
}

fn switch_doc(stroke: &str) -> String {
    format!(
        r##"{{
      "version":"1.0.0",
      "pages":[{{"id":"p","name":"P","children":[
        {{"type":"switch","id":"sw","x":0,"y":0,"width":44,"height":24,
         "checked":false,{stroke}}}
      ]}}],"children":[]
    }}"##
    )
}

fn select_doc(stroke: &str) -> String {
    format!(
        r##"{{
      "version":"1.0.0",
      "pages":[{{"id":"p","name":"P","children":[
        {{"type":"select","id":"sel","x":0,"y":0,"width":220,"height":40,
         "placeholder":"Pick one",
         "options":[{{"value":"a","label":"A"}}],{stroke}}}
      ]}}],"children":[]
    }}"##
    )
}

/// The three shapes an unresolvable stroke paint arrives in.
const UNPAINTED_STROKES: [(&str, &str); 3] = [
    ("no fill key", r#""stroke":{"thickness":1}"#),
    ("empty fill list", r#""stroke":{"thickness":1,"fill":[]}"#),
    (
        "gradient with no stops",
        r#""stroke":{"thickness":1,"fill":[{"type":"linear_gradient","stops":[]}]}"#,
    ),
];

#[test]
fn unpainted_widget_strokes_never_reach_the_scene_as_black() {
    // Collected rather than asserted in place: every input shape should be
    // reported in one run, so a partial fix cannot hide behind the first
    // failure.
    let mut offenders = Vec::new();
    for (label, stroke) in UNPAINTED_STROKES {
        for (kind, doc, id) in [
            ("switch", switch_doc(stroke), "sw"),
            ("select", select_doc(stroke), "sel"),
        ] {
            let node = scene_node(&doc, id);
            assert!(
                node.widget.is_some(),
                "{kind} / {label}: fixture must still be a widget node"
            );
            if let Some(stroke) = node.stroke {
                offenders.push(format!("{kind} / {label} -> {:?}", stroke.color));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "an unpainted stroke must be dropped so the widget resolver falls \
         back to its role defaults; still present: {offenders:#?}"
    );
}

/// The dropped-stroke rule is scoped to the missing-paint case: an author who
/// wrote a real colour — including an explicitly transparent one — keeps it.
#[test]
fn authored_widget_stroke_colours_survive_including_transparent() {
    let opaque = scene_node(
        &switch_doc(r##""stroke":{"thickness":2,"fill":[{"type":"solid","color":"#3B82F6"}]}"##),
        "sw",
    );
    let stroke = opaque.stroke.expect("authored stroke kept");
    assert_eq!(stroke.width, 2.0);
    assert!(
        stroke.color.r > 0.2 && stroke.color.b > 0.9,
        "authored blue must survive, got {:?}",
        stroke.color
    );

    // `#00000000` is a *decision*, not a missing paint — it parses, so it is
    // kept rather than swapped for the role default.
    let transparent = scene_node(
        &switch_doc(r##""stroke":{"thickness":1,"fill":[{"type":"solid","color":"#00000000"}]}"##),
        "sw",
    );
    let stroke = transparent.stroke.expect("transparent stroke is authored");
    assert_eq!(stroke.color.a, 0.0);
}

/// Only widget nodes take the drop path. An ordinary shape keeps the
/// historical opaque-black fallback, so this fix cannot silently erase
/// borders across existing documents.
#[test]
fn ordinary_shapes_keep_the_historical_black_stroke_fallback() {
    let src = r##"{
      "version":"1.0.0",
      "pages":[{"id":"p","name":"P","children":[
        {"type":"rectangle","id":"r","x":0,"y":0,"width":40,"height":40,
         "stroke":{"thickness":1}}
      ]}],"children":[]
    }"##;
    let stroke = scene_node(src, "r").stroke.expect("rect keeps its stroke");
    let rgba = (
        stroke.color.r,
        stroke.color.g,
        stroke.color.b,
        stroke.color.a,
    );
    assert_eq!(rgba, (0.0, 0.0, 0.0, 1.0));
}
