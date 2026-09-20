//! End-to-end geometry assertions for the Phase-A layout compatibility work:
//! flex-wrap row chunking, grid `span`, `translate(-50%, -50%)` centering,
//! `aspect-ratio`, per-child `mx-auto` and `position: relative` nudges.

use jian_ops_schema::node::base::NumberOrExpression;
use jian_ops_schema::node::container::{JustifyContent, LayoutMode};
use jian_ops_schema::node::{FrameNode, PenNode};
use jian_ops_schema::sizing::{SizingBehavior, SizingKeyword};

use crate::{import_html, HtmlImportOptions, HtmlImportResult};

const TAILWIND_LAYOUT: &str = include_str!("../tests/fixtures/tailwind_layout.html");

fn tailwind(viewport_height: Option<f64>) -> HtmlImportResult {
    import_html(
        TAILWIND_LAYOUT,
        &HtmlImportOptions {
            viewport_width: 1200.0,
            viewport_height,
            ..Default::default()
        },
    )
}

fn frame(node: &PenNode) -> &FrameNode {
    match node {
        PenNode::Frame(frame) => frame,
        other => panic!("expected a frame, found {other:?}"),
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

/// Locate a frame by the exact text of its first text descendant.
fn with_text<'a>(frame: &'a FrameNode, text: &str) -> Option<&'a FrameNode> {
    for child in children(frame) {
        let PenNode::Frame(candidate) = child else {
            continue;
        };
        if node_text(child).trim() == text {
            return Some(candidate);
        }
        if let Some(found) = with_text(candidate, text) {
            return Some(found);
        }
    }
    None
}

fn node_text(node: &PenNode) -> String {
    match node {
        PenNode::Text(text) => match &text.content {
            jian_ops_schema::node::text::TextContent::Plain(value) => value.clone(),
            other => format!("{other:?}"),
        },
        PenNode::Frame(frame) => children(frame).iter().map(node_text).collect(),
        _ => String::new(),
    }
}

fn number(sizing: Option<&SizingBehavior>) -> f64 {
    match sizing {
        Some(SizingBehavior::Number(value)) => *value,
        other => panic!("expected a numeric size, found {other:?}"),
    }
}

fn shell(result: &HtmlImportResult) -> &FrameNode {
    let root = frame(&result.nodes[0]);
    let page = frame(&children(root)[0]);
    // `.shell` carries `margin-left/right: auto` while its `.bar` sibling does
    // not, so it rides inside a per-child alignment row.
    let alignment = frame(&children(page)[1]);
    assert_eq!(alignment.base.name.as_deref(), Some("Auto margin"));
    assert_eq!(
        alignment.container.justify_content,
        Some(JustifyContent::Center),
        "mx-auto must centre this child alone, not every sibling"
    );
    assert_eq!(
        alignment.container.width,
        Some(SizingBehavior::Keyword(SizingKeyword::FillContainer))
    );
    let shell = frame(&children(alignment)[0]);
    assert_eq!(number(shell.container.width.as_ref()), 960.0);
    shell
}

#[test]
fn flex_wrap_cards_chunk_into_measured_rows() {
    let result = tailwind(None);
    let shell = shell(&result);
    let cards = frame(&children(shell)[0]);
    // 3 × 300px + 2 × 24px gap = 948 ≤ 960; the fourth card spills.
    assert_eq!(cards.container.layout, Some(LayoutMode::Vertical));
    assert_eq!(cards.container.gap, Some(NumberOrExpression::Number(24.0)));
    let rows = children(cards);
    assert_eq!(rows.len(), 2);
    for row in rows {
        let row = frame(row);
        assert_eq!(row.base.name.as_deref(), Some("Wrap row"));
        assert_eq!(row.container.layout, Some(LayoutMode::Horizontal));
        assert_eq!(row.container.gap, Some(NumberOrExpression::Number(24.0)));
    }
    assert_eq!(children(frame(&rows[0])).len(), 3);
    assert_eq!(children(frame(&rows[1])).len(), 1);
    assert_eq!(
        number(
            frame(&children(frame(&rows[0]))[0])
                .container
                .width
                .as_ref()
        ),
        300.0
    );
}

#[test]
fn grid_span_two_occupies_two_tracks_and_the_gap_between_them() {
    let result = tailwind(None);
    let shell = shell(&result);
    let grid = frame(&children(shell)[1]);
    let rows = children(grid);
    assert_eq!(rows.len(), 2, "span 2 + 1 fills the first row exactly");
    let first = frame(&rows[0]);
    assert_eq!(first.base.name.as_deref(), Some("Grid row"));
    let cells = children(first);
    assert_eq!(cells.len(), 2);
    // Free space 960 - 2 gaps = 920 over three 1fr tracks → 306.667 each.
    let track = 920.0 / 3.0;
    let wide = number(frame(&cells[0]).container.width.as_ref());
    assert!(
        (wide - (track * 2.0 + 20.0)).abs() < 0.01,
        "span-2 cell should be {} wide, got {wide}",
        track * 2.0 + 20.0
    );
    let single = number(frame(&cells[1]).container.width.as_ref());
    assert!((single - track).abs() < 0.01, "got {single}");
    assert_eq!(children(frame(&rows[1])).len(), 2);
}

#[test]
fn absolute_translate_centers_the_hero_copy_on_its_containing_block() {
    let result = tailwind(None);
    let shell = shell(&result);
    let hero = frame(&children(shell)[2]);
    assert_eq!(number(hero.container.width.as_ref()), 800.0);
    let copy = with_text(hero, "Centered hero").expect("hero copy frame");
    // left/top 50% of 800×400 is (400, 200); translate(-50%, -50%) of the
    // 400×120 box pulls it back by (200, 60) so the centres coincide.
    assert_eq!(copy.base.x, Some(200.0));
    assert_eq!(copy.base.y, Some(140.0));
    let width = number(copy.container.width.as_ref());
    let height = number(copy.container.height.as_ref());
    assert_eq!(
        (
            copy.base.x.unwrap() + width / 2.0,
            copy.base.y.unwrap() + height / 2.0
        ),
        (400.0, 200.0)
    );
}

#[test]
fn aspect_video_box_bakes_the_missing_axis() {
    let result = tailwind(None);
    let shell = shell(&result);
    let media = frame(&children(shell)[3]);
    assert_eq!(number(media.container.width.as_ref()), 640.0);
    assert_eq!(number(media.container.height.as_ref()), 360.0);
}

#[test]
fn relative_badge_is_nudged_inside_a_flow_preserving_wrapper() {
    let result = tailwind(None);
    let shell = shell(&result);
    let wrapper = frame(&children(shell)[5]);
    assert_eq!(wrapper.base.name.as_deref(), Some("Offset"));
    // The wrapper keeps the badge's original 40×20 box in flow.
    assert_eq!(number(wrapper.container.width.as_ref()), 40.0);
    assert_eq!(number(wrapper.container.height.as_ref()), 20.0);
    let badge = frame(&children(wrapper)[0]);
    assert_eq!((badge.base.x, badge.base.y), (Some(8.0), Some(-6.0)));
    assert!(
        result
            .warnings
            .iter()
            .any(|warning| warning.contains("CSS in-flow offsets")
                && warning.contains("fixed-size wrapper")),
        "{:?}",
        result.warnings
    );
}

#[test]
fn viewport_height_option_drives_vh_units() {
    let default_height = {
        let result = tailwind(None);
        let shell = shell(&result);
        number(frame(&children(shell)[4]).container.height.as_ref())
    };
    // Default viewport height is width * 0.625 = 750, so 50vh is 375.
    assert_eq!(default_height, 375.0);

    let pinned = {
        let result = tailwind(Some(900.0));
        let shell = shell(&result);
        number(frame(&children(shell)[4]).container.height.as_ref())
    };
    assert_eq!(pinned, 450.0);
}

#[test]
fn the_fixture_reports_only_the_expected_approximations() {
    let result = tailwind(None);
    let unexpected: Vec<_> = result
        .warnings
        .iter()
        .filter(|warning| {
            !warning.contains("CSS in-flow offsets")
                && !warning.contains("percentage absolute offsets")
        })
        .collect();
    assert!(unexpected.is_empty(), "{unexpected:?}");
    assert!(named(frame(&result.nodes[0]), "Wrap row").is_some());
}

/// The grid-placement and wrap-margin carriers travel in `PenNodeBase::theme`;
/// nothing may survive into a serialized document, including on the
/// early-return paths (single-column grid, every `flex-wrap` bail) and through
/// the offset wrapper.
#[test]
fn grid_placement_carriers_never_reach_the_output() {
    fn assert_no_theme(node: &PenNode) {
        let base = crate::mapper::node_access::node_base(node);
        assert!(
            base.theme.is_none(),
            "{:?} leaked a private carrier: {:?}",
            base.name,
            base.theme
        );
        if let PenNode::Frame(frame) = node {
            for child in children(frame) {
                assert_no_theme(child);
            }
        }
    }
    for html in [
        "<div style='display:grid;grid-template-columns:repeat(3,1fr)'>\
           <div style='grid-column:span 2'>a</div><div>b</div><div>c</div></div>",
        // Single-column grid: the post-pass returns before any chunking.
        "<div style='display:grid'><div style='grid-column:span 2'>a</div></div>",
        // Placement plus a relative nudge, so the item is re-parented.
        "<div style='display:grid;grid-template-columns:repeat(2,1fr)'>\
           <div style='grid-column:span 2;position:relative;top:4px;width:50px;height:20px'>a</div>\
           <div>b</div></div>",
        // Wrap margin carriers on every `apply_flex_wrap` bail path.
        "<div style='display:flex;flex-wrap:wrap;width:400px'>\
           <div style='margin:0 8px'>a</div><div style='margin:0 8px'>b</div></div>",
        "<div style='display:flex;flex-wrap:wrap;flex-direction:column;width:400px'>\
           <div style='margin:0 8px;width:100px;height:10px'>a</div></div>",
        "<div style='display:flex;flex-wrap:wrap;width:4000px'>\
           <div style='margin:0 8px;width:100px;height:10px'>a</div>\
           <div style='margin:0 8px;width:100px;height:10px'>b</div></div>",
        // ... and on the successful path, including a replaced-element child.
        "<div style='display:flex;flex-wrap:wrap;width:300px'>\
           <div style='margin:0 8px;width:200px;height:10px'>a</div>\
           <img src='a.png' style='margin:0 8px;width:200px;height:10px'></div>",
    ] {
        let result = import_html(html, &HtmlImportOptions::default());
        assert_no_theme(&result.nodes[0]);
    }
}

#[test]
fn flex_wrap_degrades_with_a_warning_when_children_cannot_be_measured() {
    let result = import_html(
        "<div style='display:flex;flex-wrap:wrap;width:400px'>\
           <div>auto</div><div>auto</div></div>",
        &HtmlImportOptions::default(),
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|warning| warning.contains("flex-wrap ignored")),
        "{:?}",
        result.warnings
    );
    let root = frame(&result.nodes[0]);
    let flex = frame(&children(root)[0]);
    assert_eq!(flex.container.layout, Some(LayoutMode::Horizontal));
    assert!(named(flex, "Wrap row").is_none());
}

/// Helper: the direct child names of a frame, in output (paint) order.
fn child_names(frame: &FrameNode) -> Vec<String> {
    children(frame)
        .iter()
        .map(|child| match child {
            PenNode::Frame(child) => child.base.name.clone().unwrap_or_default(),
            other => format!("{other:?}"),
        })
        .collect()
}

fn root_child(result: &HtmlImportResult, index: usize) -> &FrameNode {
    frame(&children(frame(&result.nodes[0]))[index])
}

/// B1. Row capacity subtracts the INLINE padding. `LtrB` is
/// `[top, right, bottom, left]` and `XY` is `[vertical, horizontal]`, so the
/// two arms used to read the block axis and wrap the wrong containers.
#[test]
fn wrap_capacity_uses_the_inline_padding_axis() {
    let card = "<div style='width:230px;height:20px'>x</div>";
    let container = |padding: &str| {
        format!(
            "<div style='display:flex;flex-wrap:wrap;width:700px;box-sizing:border-box;\
             padding:{padding}'>{card}{card}{card}</div>"
        )
    };
    // Content width 700 - 200 = 500: two 230px cards fit, the third spills.
    let result = import_html(&container("0 100px"), &HtmlImportOptions::default());
    let flex = root_child(&result, 0);
    assert_eq!(flex.container.layout, Some(LayoutMode::Vertical));
    assert_eq!(child_names(flex), ["Wrap row", "Wrap row"]);
    assert_eq!(children(frame(&children(flex)[0])).len(), 2);

    // Block-axis padding leaves the full 700 for the row: 3 × 230 = 690 fits.
    let result = import_html(&container("100px 0"), &HtmlImportOptions::default());
    let flex = root_child(&result, 0);
    assert_eq!(
        flex.container.layout,
        Some(LayoutMode::Horizontal),
        "vertical padding must not shrink the row capacity"
    );
    assert_eq!(children(flex).len(), 3);
}

/// B2. `apply_flex_wrap` runs after `layer_absolute_children`, so it must not
/// re-partition: the stacking pass already put the positive-z overlay first
/// and the negative-z one last, and Jian paints the first child on top.
#[test]
fn wrap_preserves_the_paint_order_the_stacking_pass_computed() {
    let overlays = "<header style='position:absolute;left:0;top:0;z-index:5;\
                     width:10px;height:10px'>top</header>\
                    <footer style='position:absolute;left:0;top:0;z-index:-1;\
                     width:10px;height:10px'>under</footer>";
    let wrapping_cards = "<div style='width:200px;height:20px'>a</div>\
                          <div style='width:200px;height:20px'>b</div>\
                          <div style='width:200px;height:20px'>c</div>";
    let result = import_html(
        &format!(
            "<div style='display:flex;flex-wrap:wrap;width:500px;box-sizing:border-box'>\
             {overlays}{wrapping_cards}</div>"
        ),
        &HtmlImportOptions::default(),
    );
    let flex = root_child(&result, 0);
    assert_eq!(
        child_names(flex),
        ["header", "Wrap row", "Wrap row", "footer"],
        "synthetic rows splice in where the flow band began"
    );

    // Every bail path hands the children back untouched, including the order.
    let auto_cards = "<div>a</div><div>b</div><div>c</div>";
    for (html, reason) in [
        (
            format!(
                "<div style='display:flex;flex-wrap:wrap;width:500px'>{overlays}{auto_cards}</div>"
            ),
            "indeterminate child widths",
        ),
        (
            format!(
                "<div style='display:flex;flex-wrap:wrap;flex-direction:column;width:500px'>\
                 {overlays}{wrapping_cards}</div>"
            ),
            "column direction",
        ),
        (
            format!(
                "<div style='display:flex;flex-wrap:wrap;width:2000px'>\
                 {overlays}{wrapping_cards}</div>"
            ),
            "nothing wraps",
        ),
    ] {
        let result = import_html(&html, &HtmlImportOptions::default());
        let flex = root_child(&result, 0);
        let names = child_names(flex);
        assert_eq!(
            names.first().map(String::as_str),
            Some("header"),
            "{reason}"
        );
        assert_eq!(names.last().map(String::as_str), Some("footer"), "{reason}");
        assert_eq!(names.len(), 5, "{reason}");
    }
}

/// B3. A percentage translation is a fraction of the element's OWN box. On an
/// auto-sized axis that box is fabricated from the containing block, so the
/// canonical `top:50%; translate(-50%,-50%)` modal used to land at y = 0.
#[test]
fn percentage_translate_is_dropped_on_an_axis_with_no_definite_size() {
    let result = import_html(
        "<div style='position:fixed;left:50%;top:50%;width:320px;\
         transform:translate(-50%,-50%)'>modal</div>",
        &HtmlImportOptions {
            viewport_width: 1000.0,
            viewport_height: Some(900.0),
            ..Default::default()
        },
    );
    let modal = root_child(&result, 0);
    // The definite 320px width still pulls back by half of it.
    assert_eq!(modal.base.x, Some(500.0 - 160.0));
    // The auto height keeps the element's top on the 50% line rather than
    // pulling it back by half the VIEWPORT and landing at 0.
    assert_eq!(modal.base.y, Some(450.0));
    assert!(
        result
            .warnings
            .iter()
            .any(|warning| warning.contains("percentage transform translation dropped")),
        "{:?}",
        result.warnings
    );

    // The tooltip idiom on an auto-width element loses only the X component.
    let result = import_html(
        "<div style='position:relative;width:400px;height:200px'>\
           <span style='position:absolute;left:50%;transform:translateX(-50%)'>tip</span>\
         </div>",
        &HtmlImportOptions::default(),
    );
    let tooltip = frame(&children(root_child(&result, 0))[0]);
    assert_eq!(tooltip.base.x, Some(200.0), "no fabricated pull-back");
}

/// B4. The centre-derived translation assumes the scale landed on the node's
/// size. `scale_axis` only multiplies numeric axes, so an axis the bake
/// declined must be re-derived with a factor of 1.
#[test]
fn translate_is_re_derived_for_axes_the_scale_bake_declined() {
    let result = import_html(
        "<div style='position:relative;width:800px;height:400px'>\
           <div style='position:absolute;left:10px;top:10px;transform:scale(2)'>auto</div>\
         </div>",
        &HtmlImportOptions::default(),
    );
    let scaled = frame(&children(root_child(&result, 0))[0]);
    assert_eq!(
        (scaled.base.x, scaled.base.y),
        (Some(10.0), Some(10.0)),
        "a declined scale must not move the node"
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|warning| warning.contains("scale ignored on an auto-sized element")),
        "{:?}",
        result.warnings
    );

    // Mixed: the numeric width takes the scale, the auto height does not.
    let result = import_html(
        "<div style='position:relative;width:800px;height:400px'>\
           <div style='position:absolute;left:10px;top:10px;width:100px;\
            transform:scale(2)'>mixed</div>\
         </div>",
        &HtmlImportOptions::default(),
    );
    let scaled = frame(&children(root_child(&result, 0))[0]);
    assert_eq!(number(scaled.container.width.as_ref()), 200.0);
    // X pulls back by half the SCALED width, Y not at all.
    assert_eq!((scaled.base.x, scaled.base.y), (Some(-40.0), Some(10.0)));
}

/// B5. `resolved_axis` falls back to `containing_width`, so a `FillContainer`
/// anchor inside a shrink-to-fit ancestor used to bake a hard pixel height out
/// of the viewport width.
#[test]
fn aspect_ratio_needs_a_definite_containing_width_to_derive_from() {
    let result = import_html(
        "<div style='display:flex'><div>\
           <div style='width:100%;aspect-ratio:2'>x</div>\
         </div></div>",
        &HtmlImportOptions::default(),
    );
    let item = frame(&children(root_child(&result, 0))[0]);
    let box_ = frame(&children(item)[0]);
    assert_eq!(
        box_.container.height,
        Some(SizingBehavior::Keyword(SizingKeyword::FitContent)),
        "no height may be invented from an indefinite containing width"
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|warning| warning.contains("aspect-ratio ignored")),
        "{:?}",
        result.warnings
    );

    // A definite ancestor still resolves the ratio.
    let result = import_html(
        "<div style='width:600px'><div style='width:100%;aspect-ratio:2'>x</div></div>",
        &HtmlImportOptions::default(),
    );
    let box_ = frame(&children(root_child(&result, 0))[0]);
    assert_eq!(number(box_.container.height.as_ref()), 300.0);
}

/// C3. `map_special` returns early, so replaced elements used to miss the
/// three in-flow post-passes an ordinary element gets: grid placement,
/// the offset wrapper, and auto-margin alignment.
#[test]
fn replaced_elements_get_the_same_in_flow_post_passes_as_frames() {
    // grid-column: span 2 on an <img> really occupies two tracks.
    let result = import_html(
        "<div style='display:grid;grid-template-columns:repeat(3,1fr);width:900px'>\
           <img src='a.png' style='grid-column:span 2'><div>b</div><div>c</div></div>",
        &HtmlImportOptions::default(),
    );
    let grid = root_child(&result, 0);
    let PenNode::Image(image) = &children(frame(&children(grid)[0]))[0] else {
        panic!("expected the image first in the grid row")
    };
    assert_eq!(image.width, Some(SizingBehavior::Number(600.0)));

    // `mx-auto` on a block-level <img> rides an alignment row.
    let result = import_html(
        "<div style='width:600px'>\
           <img src='a.png' style='display:block;margin-left:auto;margin-right:auto;\
            width:50px;height:20px'><div>x</div></div>",
        &HtmlImportOptions::default(),
    );
    let block = root_child(&result, 0);
    assert_eq!(child_names(block), ["Auto margin", "div"]);

    // A relative inset and a static transform translation both build the
    // offset wrapper instead of vanishing.
    for style in ["position:relative;left:20px", "transform:translateX(20px)"] {
        let result = import_html(
            &format!(
                "<div style='width:600px'>\
                 <img src='a.png' style='display:block;{style};width:50px;height:20px'></div>"
            ),
            &HtmlImportOptions::default(),
        );
        let block = root_child(&result, 0);
        assert_eq!(child_names(block), ["Offset"], "{style}");
        let wrapper = frame(&children(block)[0]);
        assert_eq!(number(wrapper.container.width.as_ref()), 50.0);
        let PenNode::Image(image) = &children(wrapper)[0] else {
            panic!("expected the image inside the offset wrapper")
        };
        assert_eq!(image.base.x, Some(20.0), "{style}");
        assert!(
            result
                .warnings
                .iter()
                .any(|warning| warning.contains("CSS in-flow offsets")),
            "{style}: {:?}",
            result.warnings
        );
    }
}

/// C4. `bake_scale` mutates the container before the wrapper is built, so the
/// wrapper has to be told the ORIGINAL box — CSS scaling never changes what
/// the element reserves in flow.
#[test]
fn the_offset_wrapper_reserves_the_unscaled_box() {
    let result = import_html(
        "<div style='width:600px'>\
           <div style='position:relative;left:5px;width:100px;height:40px;\
            transform:scale(1.5)'>s</div></div>",
        &HtmlImportOptions::default(),
    );
    let wrapper = frame(&children(root_child(&result, 0))[0]);
    assert_eq!(wrapper.base.name.as_deref(), Some("Offset"));
    assert_eq!(number(wrapper.container.width.as_ref()), 100.0);
    assert_eq!(number(wrapper.container.height.as_ref()), 40.0);
    let scaled = frame(&children(wrapper)[0]);
    assert_eq!(number(scaled.container.width.as_ref()), 150.0);
}

/// C9 + C11. The alignment row exists to reproduce a push the parent does not
/// already make, on a box where CSS auto margins actually do something.
#[test]
fn the_alignment_row_is_skipped_when_it_would_change_nothing() {
    // The parent's own `align-items:center` already centres this child.
    let result = import_html(
        "<div style='width:600px;align-items:center'>\
           <div style='margin-left:auto;margin-right:auto;width:50px'>a</div>\
           <div>b</div></div>",
        &HtmlImportOptions::default(),
    );
    assert_eq!(child_names(root_child(&result, 0)), ["div", "div"]);

    // ... but a one-sided auto margin asks for something else, so it wraps.
    let result = import_html(
        "<div style='width:600px;align-items:center'>\
           <div style='margin-left:auto;width:50px'>a</div><div>b</div></div>",
        &HtmlImportOptions::default(),
    );
    assert_eq!(child_names(root_child(&result, 0)), ["Auto margin", "div"]);

    // Auto margins are inert on an inline-level box in CSS.
    let result = import_html(
        "<div style='width:600px'>\
           <span style='display:inline-block;margin-left:auto;margin-right:auto;\
            width:50px'>a</span><div>b</div></div>",
        &HtmlImportOptions::default(),
    );
    assert!(
        named(root_child(&result, 0), "Auto margin").is_none(),
        "inline-level auto margins must not build a row"
    );
}
