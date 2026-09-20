//! Text / fixture / widget adapter tests — font size + weight
//! resolution, gradient payloads, the shipped `.op` fixtures, clip
//! semantics, styled text runs, and widget prop pass-through. Carved off
//! `adapter_tests.rs` to keep every file under the 800-line cap.

use super::*;

#[test]
fn text_font_size_and_weight_flow_through() {
    // login.op-style headline: 28pt, weight 700.
    let src = r##"{
      "version":"1.0.0","pages":[{"id":"p","name":"P","children":[
        {"type":"text","id":"hd","content":"Welcome Back",
         "fontSize":28,"fontWeight":700,
         "fill":[{"type":"solid","color":"#0F172A"}]}
      ]}],"children":[]
    }"##;
    let r = load(src);
    let n = &r.payload.pages[0].children[0];
    assert_eq!(n.font_size, 28.0);
    assert_eq!(n.font_weight, 700);
}

#[test]
fn text_keyword_weight_resolves() {
    let src = r##"{
      "version":"1.0.0","pages":[{"id":"p","name":"P","children":[
        {"type":"text","id":"t","content":"hi","fontWeight":"semibold"}
      ]}],"children":[]
    }"##;
    let r = load(src);
    assert_eq!(r.payload.pages[0].children[0].font_weight, 600);
}

#[test]
fn numeric_string_font_weights_resolve() {
    // pencil-demo.op authors weights as JSON strings — `"fontWeight":"700"`
    // for headlines, `"600"` for navigation chrome, `"500"` etc. The
    // canonical schema picks `FontWeight::Keyword(String)` for every
    // JSON string, so the resolver has to parse numeric keywords back
    // into u16. Otherwise SkiaMeasure picks 400-px glyph advances and
    // "Focus Protocol" (700) measures as regular-weight, mis-sizing
    // every fit_content parent.
    for (encoded, expected) in [
        (r#""700""#, 700u16),
        (r#""600""#, 600),
        (r#""500""#, 500),
        (r#""800""#, 800),
        (r#""900""#, 900),
        (r#""300""#, 300),
        (r#""normal""#, 400),
        (r#""regular""#, 400),
        (r#""bold""#, 700),
        (r#""semibold""#, 600),
        (r#""black""#, 900),
        (r#""thin""#, 100),
    ] {
        let src = format!(
            r##"{{"version":"1.0.0","pages":[{{"id":"p","name":"P","children":[
                {{"type":"text","id":"t","content":"hi","fontWeight":{}}}
            ]}}],"children":[]}}"##,
            encoded
        );
        let r = load(&src);
        let n = &r.payload.pages[0].children[0];
        assert_eq!(
            n.font_weight, expected,
            "fontWeight {} should resolve to {}, got {}",
            encoded, expected, n.font_weight
        );
    }
}

#[test]
fn linear_gradient_falls_back_to_first_stop() {
    let src = r##"{
      "version":"1.0.0","pages":[{"id":"p","name":"P","children":[{
        "type":"rectangle","id":"r","width":100,"height":50,
        "fill":[{"type":"linear_gradient","angle":90,
                 "stops":[{"color":"#FF0000","offset":0},
                          {"color":"#0000FF","offset":1}]}]
      }]}],"children":[]
    }"##;
    let r = load(src);
    let n = &r.payload.pages[0].children[0];
    let fill = n.fill.unwrap();
    assert!(fill[0] > 0.99 && fill[1] < 0.01, "first stop is red");
    assert_eq!(n.fill_type, "linear");
}

#[test]
fn linear_gradient_payload_carries_stops_and_angle() {
    // A first-class gradient body must reach the canvas painter,
    // not just collapse to the first-stop solid fallback.
    let src = r##"{
      "version":"1.0.0","pages":[{"id":"p","name":"P","children":[{
        "type":"rectangle","id":"r","width":100,"height":50,
        "fill":[{"type":"linear_gradient","angle":45,
                 "stops":[{"color":"#FF0000","offset":0},
                          {"color":"#0000FF","offset":1}]}]
      }]}],"children":[]
    }"##;
    let r = load(src);
    let n = &r.payload.pages[0].children[0];
    let gradient = n.gradient.as_ref().expect("gradient must populate");
    match gradient {
        crate::payload::GradientPayload::Linear {
            angle_deg, stops, ..
        } => {
            assert!((angle_deg - 45.0).abs() < 0.01);
            assert_eq!(stops.len(), 2);
            assert!(stops[0].color[2] < 0.01, "first stop is red");
            assert!(stops[1].color[2] > 0.99, "second stop is blue");
        }
        other => panic!("expected linear gradient, got {other:?}"),
    }
}

#[test]
fn radial_gradient_payload_uses_authored_centre_and_radius() {
    let src = r##"{
      "version":"1.0.0","pages":[{"id":"p","name":"P","children":[{
        "type":"rectangle","id":"r","width":100,"height":50,
        "fill":[{"type":"radial_gradient","cx":0.25,"cy":0.75,"radius":0.6,
                 "stops":[{"color":"#FFFFFF","offset":0},
                          {"color":"#000000","offset":1}]}]
      }]}],"children":[]
    }"##;
    let r = load(src);
    let n = &r.payload.pages[0].children[0];
    let gradient = n.gradient.as_ref().expect("gradient must populate");
    match gradient {
        crate::payload::GradientPayload::Radial { cx, cy, radius, .. } => {
            assert!((cx - 0.25).abs() < 0.01);
            assert!((cy - 0.75).abs() < 0.01);
            assert!((radius - 0.6).abs() < 0.01);
        }
        other => panic!("expected radial gradient, got {other:?}"),
    }
}

#[test]
fn solid_fill_leaves_gradient_payload_unset() {
    let src = r##"{
      "version":"1.0.0","pages":[{"id":"p","name":"P","children":[{
        "type":"rectangle","id":"r","width":100,"height":50,
        "fill":[{"type":"solid","color":"#FF0000"}]
      }]}],"children":[]
    }"##;
    let r = load(src);
    let n = &r.payload.pages[0].children[0];
    assert!(
        n.gradient.is_none(),
        "solid fill must not populate gradient"
    );
}

#[test]
fn shape_nodes_keep_authored_size() {
    // Ellipse + polygon + path don't appear in jian-core's
    // `leaf_size` resolver, so taffy returns Size::ZERO for
    // them. The adapter must fall back to authored width /
    // height — otherwise shapes vanish.
    let src = r##"{
      "version":"1.0.0","pages":[{"id":"p","name":"P","children":[
        {"type":"ellipse","id":"e","x":10,"y":20,"width":80,"height":40,
         "fill":[{"type":"solid","color":"#FF0000"}]},
        {"type":"polygon","id":"poly","x":100,"y":50,"width":60,"height":60,
         "polygonCount":6,"fill":[{"type":"solid","color":"#00FF00"}]},
        {"type":"path","id":"pa","x":200,"y":300,"width":120,"height":50,
         "anchors":[{"x":0,"y":0,"handleIn":null,"handleOut":null}]}
      ]}],"children":[]
    }"##;
    let r = load(src);
    let kids = &r.payload.pages[0].children;
    let e = &kids[0];
    let poly = &kids[1];
    let pa = &kids[2];
    assert_eq!((e.w, e.h), (80.0, 40.0), "ellipse keeps authored size");
    assert_eq!(
        (poly.w, poly.h),
        (60.0, 60.0),
        "polygon keeps authored size"
    );
    assert_eq!((pa.w, pa.h), (120.0, 50.0), "path keeps authored size");
}

#[test]
fn line_uses_signed_bounds() {
    // Vertical line: x2=0, y2=100. Encoded as bounds size
    // (0, 100); pure-axis lines pass `aggregate_bounds`' size>0
    // guard because the y axis is non-zero.
    let src = r##"{
      "version":"1.0.0","pages":[{"id":"p","name":"P","children":[{
        "type":"line","id":"l","x":10,"y":20,"x2":0,"y2":100
      }]}],"children":[]
    }"##;
    let r = load(src);
    let n = &r.payload.pages[0].children[0];
    assert_eq!(n.x, 10.0);
    assert_eq!(n.y, 20.0);
    assert_eq!(n.w, 0.0);
    assert_eq!(n.h, 100.0);
}

#[test]
fn login_op_fixture_loads() {
    let path = "/Users/kayshen/Desktop/login.op";
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };
    let src = std::str::from_utf8(&bytes).unwrap();
    let parsed = jian_ops_schema::load_str(src).unwrap();
    let adapted = pen_document_to_payload(&parsed.value);
    assert_eq!(adapted.payload.pages.len(), 1, "expected 1 page");
    let root = &adapted.payload.pages[0].children[0];
    assert_eq!(root.w, 375.0);
    assert_eq!(root.h, 812.0);
    // The 3 vertical sections (brand / form / social) get
    // stretched by taffy to fill the root width (375 px).
    assert_eq!(root.children.len(), 3);
    for section in &root.children {
        assert!(
            section.w > 300.0,
            "fill_container child should be near root width, got {}",
            section.w
        );
    }
}

#[test]
fn pencil_demo_op_fixture_loads() {
    let path = "/Users/kayshen/Desktop/pencil-demo.op";
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };
    let src = std::str::from_utf8(&bytes).unwrap();
    let parsed = crate::payload::load_canonical(src).expect("canonical load");
    let adapted = pen_document_to_payload(&parsed.value);
    assert!(
        !adapted.payload.pages.is_empty(),
        "expected at least one page"
    );
    let total_nodes: usize = adapted
        .payload
        .pages
        .iter()
        .map(|p| count_nodes(&p.children))
        .sum();
    assert!(
        total_nodes > 10,
        "pencil-demo.op should yield a substantial node tree, got {}",
        total_nodes
    );
    // Any ellipse in the fixture must come out non-zero —
    // taffy's `Size::ZERO` for unmeasured leaves used to wipe
    // authored width/height on shapes.
    let mut ellipse_zero = 0usize;
    let mut ellipse_total = 0usize;
    fn walk_ellipses(nodes: &[crate::payload::NodePayload], total: &mut usize, zero: &mut usize) {
        for n in nodes {
            if n.kind == "ellipse" {
                *total += 1;
                if n.w == 0.0 && n.h == 0.0 {
                    *zero += 1;
                }
            }
            walk_ellipses(&n.children, total, zero);
        }
    }
    for page in &adapted.payload.pages {
        walk_ellipses(&page.children, &mut ellipse_total, &mut ellipse_zero);
    }
    if ellipse_total > 0 {
        assert_eq!(
            ellipse_zero, 0,
            "{}/{} ellipses lost their authored size",
            ellipse_zero, ellipse_total
        );
    }
    fn count_nodes(nodes: &[crate::payload::NodePayload]) -> usize {
        nodes.iter().map(|n| 1 + count_nodes(&n.children)).sum()
    }
}

#[test]
fn clip_content_threads_from_schema_and_only_legacy_root_frames_clip() {
    // Legacy root frames (no authored clipContent) clip like artboards, while
    // an explicit false remains open; nested containers retain authored state.
    let src = r##"{
      "version":"1.0.0",
      "pages":[{
        "id":"p","name":"P",
        "children":[{
          "type":"frame","id":"root","width":300,"height":200,
          "children":[
            {"type":"frame","id":"clipped","width":100,"height":100,"clipContent":true},
            {"type":"frame","id":"open","width":100,"height":100},
            {"type":"group","id":"g","clipContent":true,"children":[]}
          ]
        },
        {"type":"frame","id":"open-root","width":100,"height":100,
         "clipContent":false,"children":[]}
        ]
      }],
      "children":[]
    }"##;
    let r = load(src);
    let root = &r.payload.pages[0].children[0];
    assert!(root.clip_content, "root frame clips implicitly (TS rule)");
    assert!(
        !r.payload.pages[0].children[1].clip_content,
        "explicitly open root frame must not be rewritten as legacy"
    );
    assert!(root.children[0].clip_content, "authored clipContent:true");
    assert!(!root.children[1].clip_content, "nested frame defaults open");
    assert!(
        root.children[2].clip_content,
        "groups carry clipContent too"
    );
}

#[test]
fn root_group_does_not_clip_implicitly() {
    let src = r##"{
      "version":"1.0.0",
      "pages":[{"id":"p","name":"P","children":[
        {"type":"group","id":"g","children":[]}
      ]}],
      "children":[]
    }"##;
    let r = load(src);
    assert!(
        !r.payload.pages[0].children[0].clip_content,
        "the implicit root clip applies to frames only"
    );
}

#[test]
fn styled_text_builds_runs_and_keeps_flat_string() {
    let src = r##"{
      "version":"1.0.0",
      "pages":[{"id":"p","name":"P","children":[
        {"type":"text","id":"t","fontSize":16,"fontStyle":"italic","underline":true,
         "content":[
           {"text":"Hello ","fontWeight":700,"fill":"#ff0000"},
           {"text":"world","fontSize":24,"fontStyle":"normal","underline":false,"strikethrough":true}
         ]}
      ]}],
      "children":[]
    }"##;
    let r = load(src);
    let t = &r.payload.pages[0].children[0];
    assert_eq!(t.text.as_deref(), Some("Hello world"));
    assert!(t.italic, "node-level fontStyle: italic");
    assert!(t.underline, "node-level underline");
    assert_eq!(t.text_runs.len(), 2);

    let first = &t.text_runs[0];
    assert_eq!(first.text, "Hello ");
    assert_eq!(first.font_weight, 700);
    assert_eq!(first.font_size, 0.0, "no per-seg size → inherit sentinel");
    assert_eq!(first.fill, Some([1.0, 0.0, 0.0, 1.0]));
    assert!(first.italic, "no per-seg style → inherits node italic");
    assert!(first.underline, "inherits node underline");
    assert!(!first.strikethrough);

    let second = &t.text_runs[1];
    assert_eq!(second.text, "world");
    assert_eq!(second.font_size, 24.0);
    assert_eq!(second.font_weight, 0, "inherit sentinel");
    assert_eq!(second.fill, None, "no per-seg fill → inherit node fill");
    assert!(!second.italic, "explicit normal overrides node italic");
    assert!(!second.underline, "explicit false overrides node underline");
    assert!(second.strikethrough);
}

#[test]
fn plain_text_has_no_runs() {
    let src = r##"{
      "version":"1.0.0",
      "pages":[{"id":"p","name":"P","children":[
        {"type":"text","id":"t","content":"plain"}
      ]}],
      "children":[]
    }"##;
    let r = load(src);
    let t = &r.payload.pages[0].children[0];
    assert_eq!(t.text.as_deref(), Some("plain"));
    assert!(t.text_runs.is_empty());
    assert!(!t.italic && !t.underline && !t.strikethrough);
}

#[test]
fn widget_nodes_carry_props_through_to_payload() {
    // One of each widget family, side by side under a root frame, so
    // the design-surface painter has the props it needs to draw the
    // composite static visual.
    let src = r##"{
      "version":"1.0.0",
      "pages":[{"id":"p","name":"P","children":[
        {"type":"frame","id":"root","x":0,"y":0,"width":600,"height":400,"children":[
          {"type":"switch","id":"sw","checked":true},
          {"type":"checkbox","id":"cb","checked":false,"label":"Agree"},
          {"type":"slider","id":"sl","min":0,"max":50,"step":5,"value":25},
          {"type":"progress","id":"pg","value":40,"max":80},
          {"type":"select","id":"se","value":"a",
           "options":[{"value":"a","label":"Apple"},{"value":"b","label":"Banana"}]},
          {"type":"radio_group","id":"rg","value":"b",
           "options":[{"value":"a","label":"A"},{"value":"b","label":"B"}]},
          {"type":"text_input","id":"ti","value":"hi","placeholder":"Type"},
          {"type":"number_input","id":"ni","value":7,"min":0,"max":10,"step":1},
          {"type":"tabs","id":"tb","value":"one",
           "tabs":[{"value":"one","label":"One"},{"value":"two","label":"Two"}]}
        ]}
      ]}],
      "children":[]
    }"##;
    let r = load(src);
    let kids = &r.payload.pages[0].children[0].children;
    let by = |kind: &str| {
        kids.iter()
            .find(|c| c.widget.as_ref().map(|w| w.kind.as_str()) == Some(kind))
            .unwrap_or_else(|| panic!("missing widget kind {kind}"))
            .widget
            .as_ref()
            .unwrap()
    };

    assert_eq!(by("switch").checked, Some(true));

    let cb = by("checkbox");
    assert_eq!(cb.checked, Some(false));
    assert_eq!(cb.label.as_deref(), Some("Agree"));

    let sl = by("slider");
    assert_eq!(sl.value_num, Some(25.0));
    assert_eq!(
        (sl.min, sl.max, sl.step),
        (Some(0.0), Some(50.0), Some(5.0))
    );

    let pg = by("progress");
    assert_eq!((pg.value_num, pg.max), (Some(40.0), Some(80.0)));

    let se = by("select");
    assert_eq!(se.value_str.as_deref(), Some("a"));
    assert_eq!(se.options.len(), 2);
    assert_eq!(se.options[0].label, "Apple");

    let rg = by("radio_group");
    assert_eq!(rg.value_str.as_deref(), Some("b"));
    assert_eq!(rg.options.len(), 2);

    let ti = by("text_input");
    assert_eq!(ti.value_str.as_deref(), Some("hi"));
    assert_eq!(ti.placeholder.as_deref(), Some("Type"));

    let ni = by("number_input");
    assert_eq!(ni.value_num, Some(7.0));

    let tb = by("tabs");
    assert_eq!(tb.value_str.as_deref(), Some("one"));
    assert_eq!(tb.options.len(), 2);
    assert_eq!(tb.options[1].label, "Two");
}

#[test]
fn ordinary_shapes_have_no_widget_descriptor() {
    let src = r##"{
      "version":"1.0.0",
      "pages":[{"id":"p","name":"P","children":[
        {"type":"rectangle","id":"r","x":0,"y":0,"width":10,"height":10}
      ]}],
      "children":[]
    }"##;
    let r = load(src);
    assert!(r.payload.pages[0].children[0].widget.is_none());
}
