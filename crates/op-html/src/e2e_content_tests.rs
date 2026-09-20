//! End-to-end assertions for the Phase-B content compatibility work: list
//! markers, table column geometry, responsive image selection, `@font-face`
//! visibility, background placement and text shadows.

use jian_ops_schema::node::container::LayoutMode;
use jian_ops_schema::node::text::TextContent;
use jian_ops_schema::node::{FrameNode, PenNode};
use jian_ops_schema::sizing::SizingBehavior;
use jian_ops_schema::style::{ImageFillMode, PenEffect, PenFill};

use crate::{import_html, HtmlImportOptions, HtmlImportResult};

const CONTENT_FIDELITY: &str = include_str!("../tests/fixtures/content_fidelity.html");

fn fixture() -> HtmlImportResult {
    import_html(
        CONTENT_FIDELITY,
        &HtmlImportOptions {
            viewport_width: 1200.0,
            ..Default::default()
        },
    )
}

fn root(result: &HtmlImportResult) -> &FrameNode {
    match &result.nodes[0] {
        PenNode::Frame(frame) => frame,
        other => panic!("expected a root frame, found {other:?}"),
    }
}

fn children(frame: &FrameNode) -> &[PenNode] {
    frame.children.as_deref().unwrap_or_default()
}

fn named<'a>(frame: &'a FrameNode, name: &str) -> Option<&'a FrameNode> {
    for child in children(frame) {
        if let PenNode::Frame(candidate) = child {
            if candidate.base.name.as_deref() == Some(name) {
                return Some(candidate);
            }
            if let Some(found) = named(candidate, name) {
                return Some(found);
            }
        }
    }
    None
}

fn all_named<'a>(frame: &'a FrameNode, name: &str, out: &mut Vec<&'a FrameNode>) {
    for child in children(frame) {
        if let PenNode::Frame(candidate) = child {
            if candidate.base.name.as_deref() == Some(name) {
                out.push(candidate);
            }
            all_named(candidate, name, out);
        }
    }
}

fn text_of(node: &PenNode) -> Option<String> {
    match node {
        PenNode::Text(text) => Some(match &text.content {
            TextContent::Plain(value) => value.clone(),
            TextContent::Styled(segments) => segments
                .iter()
                .map(|segment| segment.text.as_str())
                .collect(),
        }),
        _ => None,
    }
}

/// Every text run in the subtree, in document order.
fn texts(frame: &FrameNode, out: &mut Vec<String>) {
    for child in children(frame) {
        if let Some(text) = text_of(child) {
            out.push(text);
        } else if let PenNode::Frame(nested) = child {
            texts(nested, out);
        }
    }
}

fn first_image(nodes: &[PenNode]) -> Option<&jian_ops_schema::node::ImageNode> {
    for node in nodes {
        if let PenNode::Image(image) = node {
            return Some(image);
        }
        let nested: &[PenNode] = match node {
            PenNode::Frame(frame) => frame.children.as_deref().unwrap_or_default(),
            PenNode::Group(group) => group.children.as_deref().unwrap_or_default(),
            _ => &[],
        };
        if let Some(found) = first_image(nested) {
            return Some(found);
        }
    }
    None
}

/// B8. Nested `<ul>` cycles disc → circle, `<ol start>` and `value` set the
/// counter, and an authored `list-style-type` replaces the whole style.
#[test]
fn list_markers_are_prepended_to_every_item() {
    let result = fixture();
    let navigation = named(root(&result), "nav").expect("nav");
    let mut runs = Vec::new();
    texts(navigation, &mut runs);
    assert_eq!(
        runs,
        [
            "\u{2022} Top level",
            "\u{2022} Has children",
            "\u{25e6} Nested one",
            "\u{25e6} Nested two",
            "3. Third",
            "9. Ninth",
            "10. Tenth",
            "i. One",
            "ii. Two",
            "iii. Three",
            "iv. Four",
        ]
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|warning| warning.contains("list-style-position")),
        "the hanging-marker approximation is reported once: {:?}",
        result.warnings
    );
}

/// B9. `thead` / `tbody` flatten, the caption leads, and cells share one
/// column grid with `colspan` occupying two tracks.
#[test]
fn the_pricing_table_shares_one_column_grid() {
    let result = fixture();
    let table = named(root(&result), "table").expect("table");
    let table_children = children(table);
    assert!(
        matches!(&table_children[0], PenNode::Frame(caption)
            if caption.base.name.as_deref() == Some("caption")),
        "the caption leads the table"
    );

    let mut rows = Vec::new();
    all_named(table, "tr", &mut rows);
    assert_eq!(rows.len(), 3, "thead + two tbody rows flatten in order");
    assert_eq!(rows[0].container.layout, Some(LayoutMode::Horizontal));

    let widths = |row: &FrameNode| -> Vec<f64> {
        children(row)
            .iter()
            .map(|cell| match cell {
                PenNode::Frame(frame) => match frame.container.width {
                    Some(SizingBehavior::Number(width)) => width,
                    ref other => panic!("expected a numeric width, found {other:?}"),
                },
                other => panic!("expected a cell frame, found {other:?}"),
            })
            .collect()
    };
    // `border-collapse: collapse` removes the spacing, so 900 splits three ways
    // and the `colspan="2"` header takes two of those tracks.
    assert_eq!(widths(rows[0]), [600.0, 300.0]);
    assert_eq!(widths(rows[1]), [300.0, 300.0, 300.0]);
    assert_eq!(widths(rows[2]), [300.0, 300.0, 300.0]);
}

/// B10. The desktop viewport skips the narrow `media` source and the AVIF
/// source, then takes the widest candidate that covers the slot.
#[test]
fn the_picture_selects_the_wide_decodable_candidate() {
    let result = fixture();
    let image = first_image(&result.nodes).expect("picture image");
    assert_eq!(image.src.as_str(), "wide-1600.jpg");

    let phone = import_html(
        CONTENT_FIDELITY,
        &HtmlImportOptions {
            viewport_width: 480.0,
            ..Default::default()
        },
    );
    assert_eq!(
        first_image(&phone.nodes)
            .expect("picture image")
            .src
            .as_str(),
        "phone-480.jpg"
    );
}

/// B11. Only the declared family imported text really asks for is reported.
#[test]
fn the_referenced_web_font_is_reported_once() {
    let result = fixture();
    let reported: Vec<&String> = result
        .warnings
        .iter()
        .filter(|warning| warning.contains("@font-face"))
        .collect();
    assert_eq!(reported.len(), 1, "{:?}", result.warnings);
    assert!(reported[0].contains("'Inter'"), "{}", reported[0]);
    assert!(
        !reported[0].contains("Never Used"),
        "an unreferenced family is not a degradation of this document"
    );
}

/// B12. `cover` + `center` is exact; an explicit size + offset becomes a crop
/// transform whose unit square is the painted window.
#[test]
fn background_placement_uses_the_expressible_subset() {
    let result = fixture();
    let hero = named(root(&result), "section").expect("hero section");
    let Some([PenFill::Image(hero_fill)]) = hero.container.fill.as_deref() else {
        panic!(
            "hero should carry one image fill: {:?}",
            hero.container.fill
        )
    };
    assert_eq!(hero_fill.mode, Some(ImageFillMode::Fill));
    assert!(hero_fill.transform.is_none());

    let badge = children(root(&result))
        .iter()
        .find_map(|node| match node {
            PenNode::Frame(frame)
                if frame.base.name.as_deref() == Some("div") && frame.container.fill.is_some() =>
            {
                Some(frame)
            }
            _ => None,
        })
        .expect("badge div");
    let Some([PenFill::Image(badge_fill)]) = badge.container.fill.as_deref() else {
        panic!("badge should carry one image fill")
    };
    assert_eq!(badge_fill.mode, Some(ImageFillMode::Crop));
    let transform = badge_fill.transform.as_ref().expect("crop transform");
    assert_eq!((transform.m00, transform.m11), (2.0, 2.0));
    assert_eq!((transform.m02, transform.m12), (-0.25, -0.2));
    assert!(
        !result
            .warnings
            .iter()
            .any(|warning| warning.contains("background-position")),
        "every authored position in the fixture is expressible: {:?}",
        result.warnings
    );
}

/// B13. The first `text-shadow` layer lands on the text node and `overflow:
/// auto` clips.
#[test]
fn text_shadows_and_scroll_containers_survive() {
    let result = fixture();
    let heading = named(root(&result), "h1").expect("h1");
    let PenNode::Text(title) = &children(heading)[0] else {
        panic!("expected the heading text")
    };
    let Some([PenEffect::Shadow(shadow)]) = title.effects.as_deref() else {
        panic!(
            "expected exactly one text shadow, found {:?}",
            title.effects
        )
    };
    assert_eq!(
        (shadow.offset_x, shadow.offset_y, shadow.blur),
        (0.0, 2.0, 6.0)
    );
    assert_eq!(shadow.color, "#00000099");
    assert!(
        result
            .warnings
            .iter()
            .any(|warning| warning.contains("text-shadow layers after the first")),
        "{:?}",
        result.warnings
    );

    let scroller = children(root(&result))
        .iter()
        .rev()
        .find_map(|node| match node {
            PenNode::Frame(frame) if frame.container.clip_content == Some(true) => Some(frame),
            _ => None,
        })
        .expect("the overflow:auto div clips");
    assert_eq!(
        scroller.container.width,
        Some(SizingBehavior::Number(320.0))
    );
}
