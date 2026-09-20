//! Per-kind emission tests plus the two invariants the whole exporter
//! rests on: nothing outside the file, and text that is text.

use super::*;
use op_editor_ui::layout_scene::{
    Effect, NodeKind, SceneFillType, SceneGradient, SceneGradientStop, SceneImageFit, SceneStroke,
    SceneStrokeAlign, SceneTextAlign, SceneWidget,
};
use op_editor_ui::{Color, Rect};

/// A 200x100 board at the canvas origin holding `children`.
fn board(children: Vec<SceneNode>) -> SceneNode {
    let mut n = SceneNode::leaf("board", NodeKind::Frame);
    n.bounds = Rect::xywh(0.0, 0.0, 200.0, 100.0);
    n.fill = Some(Color::WHITE);
    n.children = children;
    n
}

/// A board deliberately placed away from the origin, so any emitter
/// that forgot to subtract the board origin produces obviously wrong
/// coordinates rather than accidentally-correct ones.
fn offset_board(children: Vec<SceneNode>) -> SceneNode {
    let mut n = board(children);
    n.bounds = Rect::xywh(1000.0, 500.0, 200.0, 100.0);
    n
}

fn page_with(board: SceneNode) -> ScenePage {
    ScenePage {
        id: "p1".into(),
        name: "Page 1".into(),
        children: vec![board],
    }
}

fn markup_of(board: SceneNode) -> SlideMarkup {
    let page = page_with(board);
    board_slide_markup(&page, "board", "Slide".into()).expect("board emits")
}

fn body_of(board: SceneNode) -> String {
    markup_of(board).body
}

fn text_node(id: &str, x: f32, y: f32, content: &str) -> SceneNode {
    let mut n = SceneNode::leaf(id, NodeKind::Text);
    n.bounds = Rect::xywh(x, y, 160.0, 40.0);
    n.text = Some(content.to_string());
    n.font_size = 24.0;
    n.font_weight = 700;
    n.fill = Some(Color {
        r: 0.0,
        g: 0.5,
        b: 1.0,
        a: 1.0,
    });
    n
}

// ---------------------------------------------------------------- text

#[test]
fn text_lands_as_real_characters_with_its_authored_style() {
    let body = body_of(board(vec![text_node("t", 20.0, 10.0, "Hello deck")]));

    assert!(body.contains(">Hello deck<"), "{body}");
    assert!(body.contains("font-size:24px"), "{body}");
    assert!(body.contains("font-weight:700"), "{body}");
    assert!(body.contains("color:rgb(0,128,255)"), "{body}");
    assert!(body.contains("left:20px;top:10px;"), "{body}");
}

#[test]
fn text_coordinates_are_relative_to_the_board_not_the_canvas() {
    // The node sits at (1020, 510) in doc space on a board whose origin
    // is (1000, 500), so the slide-local position must be (20, 10).
    let body = body_of(offset_board(vec![text_node(
        "t", 1020.0, 510.0, "Anchored",
    )]));

    assert!(body.contains("left:20px;top:10px;"), "{body}");
    assert!(!body.contains("left:1020px"), "{body}");
}

#[test]
fn a_nested_node_is_positioned_against_its_parent_not_the_slide() {
    // Board (0,0) > row (120, 700) > label (371, 700), all absolute in
    // doc space. `position:absolute` resolves against the parent
    // element, so the label must come out at (371-120, 700-700).
    // Subtracting the board origin at every depth instead would emit
    // top:700px and drop the label 700px down the slide.
    let mut row = SceneNode::leaf("row", NodeKind::Frame);
    row.bounds = Rect::xywh(120.0, 700.0, 1680.0, 45.0);
    row.children = vec![text_node("label", 371.0, 700.0, "2026.08")];

    let body = body_of(board(vec![row]));

    assert!(
        body.contains(r#"style="left:251px;top:0px;"#),
        "nested label must be parent-relative: {body}"
    );
}

#[test]
fn nesting_depth_does_not_accumulate_ancestor_offsets() {
    let mut inner = SceneNode::leaf("inner", NodeKind::Frame);
    inner.bounds = Rect::xywh(30.0, 30.0, 40.0, 40.0);
    inner.children = vec![text_node("deep", 40.0, 40.0, "deep")];
    let mut outer = SceneNode::leaf("outer", NodeKind::Frame);
    outer.bounds = Rect::xywh(10.0, 10.0, 100.0, 80.0);
    outer.children = vec![inner];

    let body = body_of(board(vec![outer]));

    // Each level contributes exactly its own offset: 10, then 20, then 10.
    assert!(body.contains("left:10px;top:10px;width:100px"), "{body}");
    assert!(body.contains("left:20px;top:20px;width:40px"), "{body}");
    assert!(body.contains("left:10px;top:10px;width:160px"), "{body}");
}

#[test]
fn a_sibling_after_a_container_is_positioned_against_the_container_parent() {
    // The walk has to restore the parent origin when it leaves a
    // container, or every later sibling inherits the container's box.
    let mut container = SceneNode::leaf("box", NodeKind::Frame);
    container.bounds = Rect::xywh(50.0, 50.0, 40.0, 40.0);
    container.children = vec![text_node("inner", 60.0, 60.0, "in")];

    let body = body_of(board(vec![
        container,
        text_node("after", 5.0, 5.0, "after"),
    ]));

    // `after` is a board child, so it keeps its board-relative rect.
    let at = body.find(">after<").expect("sibling text");
    let tag_start = body[..at].rfind("<div").expect("its element");
    assert!(
        body[tag_start..at].contains("left:5px;top:5px;"),
        "{}",
        &body[tag_start..at]
    );
}

#[test]
fn text_content_is_escaped_rather_than_injected() {
    let body = body_of(board(vec![text_node(
        "t",
        0.0,
        0.0,
        r#"<script>alert("x") & co</script>"#,
    )]));

    assert!(
        body.contains("&lt;script&gt;alert(&quot;x&quot;) &amp; co&lt;/script&gt;"),
        "{body}"
    );
    assert!(!body.contains("<script>"), "raw markup leaked: {body}");
}

#[test]
fn text_carries_alignment_wrapping_and_tracking() {
    let mut node = text_node("t", 0.0, 0.0, "wrapped");
    node.text_align = SceneTextAlign::Center;
    node.text_wrap = true;
    node.letter_spacing = 1.5;
    node.line_height = 1.4;
    node.italic = true;
    node.underline = true;

    let body = body_of(board(vec![node]));

    assert!(body.contains("text-align:center"), "{body}");
    assert!(body.contains("white-space:pre-wrap"), "{body}");
    assert!(body.contains("letter-spacing:1.5px"), "{body}");
    assert!(body.contains("line-height:1.4"), "{body}");
    assert!(body.contains("font-style:italic"), "{body}");
    assert!(body.contains("text-decoration:underline"), "{body}");
}

#[test]
fn a_non_wrapping_text_node_keeps_its_newlines_without_wrapping() {
    let body = body_of(board(vec![text_node("t", 0.0, 0.0, "line one\nline two")]));

    assert!(body.contains("white-space:pre;"), "{body}");
    assert!(!body.contains("white-space:pre-wrap"), "{body}");
    assert!(body.contains("line one\nline two"), "{body}");
}

#[test]
fn styled_runs_become_spans_without_breaking_the_flat_text() {
    let mut node = text_node("t", 0.0, 0.0, "plain BOLD plain");
    node.text_runs = vec![op_editor_ui::layout_scene::SceneTextRun {
        start: 6,
        end: 10,
        font_size: 0.0,
        font_weight: 900,
        fill: Some(Color::RED),
        italic: false,
        underline: false,
        strikethrough: false,
    }];

    let body = body_of(board(vec![node]));

    assert!(body.contains("plain <span"), "{body}");
    assert!(body.contains("font-weight:900"), "{body}");
    assert!(body.contains("color:rgb(255,0,0)"), "{body}");
    assert!(body.contains(">BOLD</span> plain"), "{body}");
}

// --------------------------------------------------------------- boxes

#[test]
fn per_corner_radii_map_across_position_for_position() {
    let mut node = SceneNode::leaf("card", NodeKind::Rect);
    node.bounds = Rect::xywh(10.0, 10.0, 80.0, 40.0);
    node.fill = Some(Color::WHITE);
    node.corner_radii = Some([4.0, 8.0, 12.0, 16.0]);

    let body = body_of(board(vec![node]));

    assert!(
        body.contains("border-radius:4px 8px 12px 16px"),
        "top-left, top-right, bottom-right, bottom-left: {body}"
    );
}

#[test]
fn a_linear_gradient_becomes_a_css_gradient_at_the_same_angle() {
    let mut node = SceneNode::leaf("g", NodeKind::Rect);
    node.bounds = Rect::xywh(0.0, 0.0, 100.0, 50.0);
    node.fill_type = SceneFillType::LinearGradient;
    node.gradient = Some(SceneGradient::Linear {
        angle_deg: 90.0,
        opacity: 1.0,
        stops: vec![
            SceneGradientStop {
                offset: 0.0,
                color: Color::RED,
            },
            SceneGradientStop {
                offset: 1.0,
                color: Color::BLUE,
            },
        ],
    });

    let body = body_of(board(vec![node]));

    assert!(
        body.contains("background-image:linear-gradient(90deg,rgb(255,0,0) 0%,rgb(0,0,255) 100%)"),
        "{body}"
    );
}

#[test]
fn a_radial_gradient_keeps_its_centre_and_radius_in_pixels() {
    let mut node = SceneNode::leaf("g", NodeKind::Rect);
    node.bounds = Rect::xywh(0.0, 0.0, 100.0, 50.0);
    node.fill_type = SceneFillType::RadialGradient;
    node.gradient = Some(SceneGradient::Radial {
        cx: 0.5,
        cy: 0.5,
        radius: 0.5,
        opacity: 1.0,
        stops: vec![
            SceneGradientStop {
                offset: 0.0,
                color: Color::WHITE,
            },
            SceneGradientStop {
                offset: 1.0,
                color: Color::BLACK,
            },
        ],
    });

    let body = body_of(board(vec![node]));

    // radius = max(w, h) * 0.5 = 50; centre = (50, 25).
    assert!(
        body.contains("radial-gradient(circle 50px at 50px 25px,"),
        "{body}"
    );
}

#[test]
fn rotation_becomes_a_transform_pinned_to_the_bounds_centre() {
    let mut node = SceneNode::leaf("r", NodeKind::Rect);
    node.bounds = Rect::xywh(10.0, 10.0, 40.0, 40.0);
    node.fill = Some(Color::RED);
    node.rotation = 0.5;

    let body = body_of(board(vec![node]));

    assert!(body.contains("transform:rotate(0.5rad)"), "{body}");
    assert!(body.contains("transform-origin:50% 50%"), "{body}");
}

#[test]
fn a_stroke_is_an_overlay_so_it_cannot_shift_the_children_it_covers() {
    let mut child = text_node("t", 20.0, 20.0, "inside");
    child.font_size = 12.0;
    let mut node = SceneNode::leaf("panel", NodeKind::Frame);
    node.bounds = Rect::xywh(10.0, 10.0, 100.0, 60.0);
    node.fill = Some(Color::WHITE);
    node.stroke = Some(SceneStroke {
        color: Color::BLACK,
        width: 4.0,
        sides: None,
        align: SceneStrokeAlign::Inside,
    });
    node.children = vec![child];

    let body = body_of(board(vec![node]));

    // The container itself carries no border, so an absolutely
    // positioned child still resolves against the full box.
    assert!(
        body.contains(
            r#"<div class="n k" style="left:0px;top:0px;width:100px;height:60px;border-style:solid;border-color:rgb(0,0,0);border-width:4px;"#
        ),
        "{body}"
    );
    // Overlay comes after the child, so the outline sits on top.
    let child_at = body.find(">inside<").expect("child text");
    let overlay_at = body.find(r#"class="n k""#).expect("stroke overlay");
    assert!(child_at < overlay_at, "{body}");
}

#[test]
fn per_side_stroke_widths_map_onto_the_matching_border_sides() {
    let mut node = SceneNode::leaf("divider", NodeKind::Rect);
    node.bounds = Rect::xywh(0.0, 0.0, 100.0, 10.0);
    node.stroke = Some(SceneStroke {
        color: Color::BLACK,
        width: 0.0,
        sides: Some([0.0, 0.0, 2.0, 0.0]),
        align: SceneStrokeAlign::Inside,
    });

    let body = body_of(board(vec![node]));

    assert!(body.contains("border-width:0px 0px 2px 0px"), "{body}");
}

#[test]
fn an_outside_stroke_outsets_its_overlay_by_the_full_width() {
    let mut node = SceneNode::leaf("ring", NodeKind::Rect);
    node.bounds = Rect::xywh(0.0, 0.0, 40.0, 40.0);
    node.stroke = Some(SceneStroke {
        color: Color::BLACK,
        width: 3.0,
        sides: None,
        align: SceneStrokeAlign::Outside,
    });

    let body = body_of(board(vec![node]));

    assert!(
        body.contains("left:-3px;top:-3px;width:46px;height:46px"),
        "{body}"
    );
}

#[test]
fn clip_content_and_composite_opacity_ride_on_the_container() {
    let mut node = SceneNode::leaf("c", NodeKind::Frame);
    node.bounds = Rect::xywh(0.0, 0.0, 50.0, 50.0);
    node.clip_content = true;
    node.composite_opacity = 0.5;

    let body = body_of(board(vec![node]));

    assert!(body.contains("overflow:hidden"), "{body}");
    assert!(body.contains("opacity:0.5"), "{body}");
}

#[test]
fn effects_split_across_shadow_blur_and_backdrop_channels() {
    let mut node = SceneNode::leaf("fx", NodeKind::Rect);
    node.bounds = Rect::xywh(0.0, 0.0, 50.0, 50.0);
    node.fill = Some(Color::WHITE);
    node.effects = vec![
        Effect::DropShadow(op_editor_ui::layout_scene::DropShadow {
            offset_x: 0.0,
            offset_y: 4.0,
            blur: 12.0,
            color: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.25,
            },
            inner: false,
        }),
        Effect::Blur(op_editor_ui::layout_scene::BlurEffect { radius: 8.0 }),
    ];

    let body = body_of(board(vec![node]));

    assert!(
        body.contains("box-shadow:0px 4px 12px rgba(0,0,0,0.25)"),
        "{body}"
    );
    // CSS `filter: blur()` takes the sigma, which the painter computes
    // as `radius * 0.5`.
    assert!(body.contains("filter:blur(4px)"), "{body}");
}

#[test]
fn paint_order_follows_the_canvas_reversal_of_sibling_order() {
    // Scene children are topmost-first; DOM order is bottom-first.
    let body = body_of(board(vec![
        text_node("front", 0.0, 0.0, "FRONT"),
        text_node("back", 0.0, 0.0, "BACK"),
    ]));

    let back = body.find(">BACK<").expect("back text");
    let front = body.find(">FRONT<").expect("front text");
    assert!(back < front, "topmost sibling must be emitted last: {body}");
}

#[test]
fn a_hidden_node_is_not_emitted_at_all() {
    let mut hidden = text_node("h", 0.0, 0.0, "invisible");
    hidden.hidden = true;

    let body = body_of(board(vec![hidden, text_node("v", 0.0, 50.0, "visible")]));

    assert!(!body.contains("invisible"), "{body}");
    assert!(body.contains(">visible<"), "{body}");
}

// -------------------------------------------------------------- vector

#[test]
fn an_icon_font_node_becomes_an_inline_svg_path() {
    let mut node = SceneNode::leaf("icon", NodeKind::Other("icon_font".into()));
    node.bounds = Rect::xywh(10.0, 10.0, 24.0, 24.0);
    node.text = Some("check".to_string());
    node.fill = Some(Color::RED);

    let body = body_of(board(vec![node]));

    assert!(body.contains("<svg class=\"n\""), "{body}");
    assert!(body.contains("<path d=\""), "no glyph geometry: {body}");
    assert!(body.contains("stroke=\"rgb(255,0,0)\""), "{body}");
    // Inline means no request: the glyph data is in the markup.
    assert!(!body.contains("http"), "{body}");
}

#[test]
fn an_unknown_icon_name_falls_back_to_the_same_dot_the_canvas_draws() {
    let mut node = SceneNode::leaf("icon", NodeKind::Other("icon_font".into()));
    node.bounds = Rect::xywh(0.0, 0.0, 24.0, 24.0);
    node.text = Some("not-a-real-glyph-name".to_string());

    let body = body_of(board(vec![node]));

    assert!(
        body.contains("M12 12m-3 0a3 3 0 1 0 6 0a3 3 0 1 0 -6 0"),
        "{body}"
    );
}

#[test]
fn an_icon_is_centred_in_a_square_inside_a_non_square_box() {
    let mut node = SceneNode::leaf("icon", NodeKind::Other("icon_font".into()));
    node.bounds = Rect::xywh(0.0, 0.0, 40.0, 20.0);
    node.text = Some("check".to_string());

    let body = body_of(board(vec![node]));

    // 20px square centred in a 40x20 box.
    assert!(
        body.contains("left:10px;top:0px;width:20px;height:20px"),
        "{body}"
    );
}

#[test]
fn a_line_node_becomes_an_svg_segment_in_absolute_doc_coordinates() {
    let mut node = SceneNode::leaf("l", NodeKind::Line);
    node.bounds = Rect::xywh(10.0, 10.0, 80.0, 40.0);
    node.stroke = Some(SceneStroke {
        color: Color::BLACK,
        width: 2.0,
        sides: None,
        align: SceneStrokeAlign::Center,
    });

    let body = body_of(board(vec![node]));

    assert!(
        body.contains(r#"<line x1="10" y1="10" x2="90" y2="50""#),
        "{body}"
    );
    assert!(body.contains(r#"stroke-width="2""#), "{body}");
}

#[test]
fn a_horizontal_line_still_gets_a_renderable_svg_viewport() {
    let mut node = SceneNode::leaf("l", NodeKind::Line);
    node.bounds = Rect::xywh(0.0, 20.0, 100.0, 0.0);
    node.stroke = Some(SceneStroke {
        color: Color::BLACK,
        width: 1.0,
        sides: None,
        align: SceneStrokeAlign::Center,
    });

    let body = body_of(board(vec![node]));

    // A zero-height box would render nothing at all.
    assert!(!body.contains("height:0px"), "{body}");
}

#[test]
fn a_pen_path_becomes_an_svg_polyline_through_its_points() {
    let mut node = SceneNode::leaf("p", NodeKind::Path);
    node.bounds = Rect::xywh(0.0, 0.0, 60.0, 60.0);
    node.points = vec![
        op_editor_ui::Point2D::new(0.0, 0.0),
        op_editor_ui::Point2D::new(30.0, 40.0),
        op_editor_ui::Point2D::new(60.0, 10.0),
    ];
    node.fill = Some(Color::BLACK);

    let body = body_of(board(vec![node]));

    assert!(body.contains(r#"d="M0 0 L30 40 L60 10""#), "{body}");
}

#[test]
fn a_polygon_becomes_an_svg_polygon_over_its_vertices() {
    let mut node = SceneNode::leaf("tri", NodeKind::Polygon);
    node.bounds = Rect::xywh(0.0, 0.0, 40.0, 40.0);
    node.polygon_sides = 3;
    node.fill = Some(Color::RED);

    let body = body_of(board(vec![node]));

    assert!(body.contains("<polygon points=\""), "{body}");
    assert!(body.contains(r#"fill="rgb(255,0,0)""#), "{body}");
}

// ------------------------------------------------------------ fallback

/// Decode every `data:image/png;base64,` payload in the markup.
fn embedded_pngs(body: &str) -> Vec<Vec<u8>> {
    use base64::Engine as _;
    body.split("src=\"data:image/png;base64,")
        .skip(1)
        .map(|rest| {
            let payload = rest.split('"').next().expect("closing quote");
            base64::engine::general_purpose::STANDARD
                .decode(payload)
                .expect("payload is valid base64")
        })
        .collect()
}

/// Strip every `data:` URI so an assertion about external references
/// cannot be fooled by a byte sequence inside a base64 payload.
fn without_data_uris(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(at) = rest.find("data:") {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        // A data URI in this markup always terminates at the quote that
        // closes the attribute it sits in.
        match rest.find('"') {
            Some(end) => rest = &rest[end..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

#[test]
fn an_externally_linked_image_is_rasterised_rather_than_linked() {
    let mut node = SceneNode::leaf("img", NodeKind::Rect);
    node.bounds = Rect::xywh(20.0, 20.0, 80.0, 40.0);
    node.fill = Some(Color::WHITE);
    node.image_src = Some("https://example.com/photo.png".into());
    node.image_fit = SceneImageFit::Fill;

    let markup = markup_of(board(vec![node]));

    assert_eq!(
        markup.fallback_reasons,
        vec!["image source is not embedded bytes"]
    );
    let pngs = embedded_pngs(&markup.body);
    assert_eq!(pngs.len(), 1, "the node must be embedded as PNG bytes");
    assert!(pngs[0].starts_with(b"\x89PNG\r\n\x1a\n"), "not a PNG");
    // The link must be gone, not merely unused.
    let text = without_data_uris(&markup.body);
    assert!(!text.contains("example.com"), "{text}");
    assert!(!text.contains("http"), "{text}");
}

#[test]
fn an_unknown_node_kind_is_rasterised_rather_than_guessed_at() {
    let mut node = SceneNode::leaf("mystery", NodeKind::Other("hologram".into()));
    node.bounds = Rect::xywh(10.0, 10.0, 60.0, 30.0);
    node.fill = Some(Color::RED);
    node.children = vec![text_node("inner", 20.0, 15.0, "swallowed")];

    let markup = markup_of(board(vec![node]));

    assert_eq!(markup.fallback_reasons, vec!["unknown node kind"]);
    assert_eq!(embedded_pngs(&markup.body).len(), 1);
    // The subtree is in the pixels; emitting it again would double-paint.
    assert!(!markup.body.contains("swallowed"), "{}", markup.body);
}

#[test]
fn a_rastered_node_is_placed_at_its_own_rect_not_the_boards() {
    let mut node = SceneNode::leaf("mystery", NodeKind::Other("hologram".into()));
    node.bounds = Rect::xywh(1030.0, 520.0, 60.0, 30.0);
    node.fill = Some(Color::RED);

    let markup = markup_of(offset_board(vec![node]));

    assert!(
        markup
            .body
            .contains("left:30px;top:20px;width:60px;height:30px"),
        "{}",
        markup.body
    );
}

#[test]
fn every_unexpressible_paint_takes_the_raster_path_with_a_stated_reason() {
    #[allow(clippy::type_complexity)]
    let cases: Vec<(&str, Box<dyn Fn(&mut SceneNode)>)> = vec![
        (
            "mesh gradient fill",
            Box::new(|n: &mut SceneNode| {
                n.fill_type = SceneFillType::MeshGradient;
                n.gradient = Some(SceneGradient::Mesh {
                    rows: 2,
                    cols: 2,
                    colors: vec![Color::RED, Color::BLUE, Color::WHITE, Color::BLACK],
                    opacity: 1.0,
                });
            }),
        ),
        (
            "composite widget",
            Box::new(|n: &mut SceneNode| {
                n.widget = Some(SceneWidget {
                    kind: "switch".into(),
                    ..SceneWidget::default()
                });
            }),
        ),
        (
            "ellipse arc",
            Box::new(|n: &mut SceneNode| {
                n.kind = NodeKind::Ellipse;
                n.arc_sweep_angle = Some(180.0);
            }),
        ),
        (
            "image colour adjustments",
            Box::new(|n: &mut SceneNode| {
                n.image_src = Some("data:image/png;base64,AAAA".into());
                n.image_adjustments = ImageAdjustments {
                    contrast: 20.0,
                    ..ImageAdjustments::default()
                };
            }),
        ),
        (
            "image crop transform",
            Box::new(|n: &mut SceneNode| {
                n.image_src = Some("data:image/png;base64,AAAA".into());
                n.image_transform = Some([1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
            }),
        ),
    ];

    for (expected, mutate) in cases {
        let mut node = SceneNode::leaf("n", NodeKind::Rect);
        node.bounds = Rect::xywh(10.0, 10.0, 40.0, 40.0);
        node.fill = Some(Color::WHITE);
        mutate(&mut node);

        let markup = markup_of(board(vec![node]));

        assert_eq!(markup.fallback_reasons, vec![expected]);
        assert_eq!(
            embedded_pngs(&markup.body).len(),
            1,
            "{expected} did not raster"
        );
    }
}

#[test]
fn an_embedded_image_rides_as_a_background_not_a_fallback() {
    // A 1x1 transparent PNG, the smallest honest `data:` payload.
    const PIXEL: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
    let mut node = SceneNode::leaf("img", NodeKind::Rect);
    node.bounds = Rect::xywh(0.0, 0.0, 60.0, 60.0);
    node.image_src = Some(PIXEL.into());
    node.image_fit = SceneImageFit::Fit;

    let markup = markup_of(board(vec![node]));

    assert!(
        markup.fallback_reasons.is_empty(),
        "{:?}",
        markup.fallback_reasons
    );
    assert!(
        markup
            .body
            .contains("background-image:url(\"data:image/png"),
        "{}",
        markup.body
    );
    assert!(
        markup.body.contains("background-size:contain"),
        "{}",
        markup.body
    );
}

// ---------------------------------------------------- whole-file rules

#[test]
fn a_whole_slide_carries_no_external_reference_of_any_kind() {
    let mut icon = SceneNode::leaf("icon", NodeKind::Other("icon_font".into()));
    icon.bounds = Rect::xywh(150.0, 10.0, 24.0, 24.0);
    icon.text = Some("check".to_string());
    let mut linked = SceneNode::leaf("linked", NodeKind::Rect);
    linked.bounds = Rect::xywh(10.0, 50.0, 40.0, 40.0);
    linked.image_src = Some("http://cdn.example.com/a.jpg".into());

    let markup = markup_of(board(vec![
        text_node("t", 10.0, 10.0, "Title"),
        icon,
        linked,
    ]));
    let text = without_data_uris(&markup.body);

    for forbidden in ["http://", "https://", "<link", "@import", "fetch("] {
        assert!(!text.contains(forbidden), "found {forbidden} in {text}");
    }
    // …and the text really is text, not only bytes inside a payload.
    assert!(text.contains(">Title<"), "{text}");
}

#[test]
fn the_slide_reports_what_it_structured_and_what_it_could_not() {
    let mut linked = SceneNode::leaf("linked", NodeKind::Rect);
    linked.bounds = Rect::xywh(10.0, 50.0, 40.0, 40.0);
    linked.image_src = Some("http://cdn.example.com/a.jpg".into());

    let markup = markup_of(board(vec![text_node("t", 10.0, 10.0, "Title"), linked]));

    // Board + text; the linked image is the one fallback.
    assert_eq!(markup.structured_nodes, 2);
    assert_eq!(markup.raster_fallbacks(), 1);
    assert_eq!(markup.width, 200.0);
    assert_eq!(markup.height, 100.0);
}

#[test]
fn a_board_with_nothing_expressible_degrades_to_a_single_image() {
    let mut b = board(vec![text_node("t", 10.0, 10.0, "lost")]);
    b.widget = Some(SceneWidget {
        kind: "switch".into(),
        ..SceneWidget::default()
    });

    let markup = markup_of(b);

    assert_eq!(markup.structured_nodes, 0);
    assert_eq!(markup.raster_fallbacks(), 1);
    assert_eq!(embedded_pngs(&markup.body).len(), 1);
}

#[test]
fn a_missing_board_names_itself_in_the_error() {
    let page = page_with(board(Vec::new()));

    let err = board_slide_markup(&page, "nope", "Slide".into()).expect_err("unknown board");

    assert!(err.to_string().contains("nope"), "{err}");
}
