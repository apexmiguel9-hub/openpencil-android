//! Tabs paint contracts for the design-canvas widget painter.
//!
//! Split out of the `canvas_viewport_widget_tests.rs` spine (800-line
//! ceiling); the shared recorder + node fixtures come in via `use super::*`.

use super::*;

#[test]
fn tabs_matches_segmented_preview_and_stale_value_falls_back_first() {
    let rect = Rect::xywh(0.0, 0.0, 240.0, 120.0);
    let node = authored_widget_node(
        NodeKind::Frame,
        SceneWidget {
            kind: "tabs".into(),
            value_str: Some("stale".into()),
            options: vec![
                SceneWidgetOption {
                    value: "one".into(),
                    label: "One".into(),
                },
                SceneWidgetOption {
                    value: "two".into(),
                    label: "Two".into(),
                },
            ],
            ..Default::default()
        },
        rect,
        0.0,
    );
    let b = paint(&node, rect);
    assert!(b.texts.iter().any(|(t, _)| t == "One"));
    assert!(b.texts.iter().any(|(t, _)| t == "Two"));
    // Authored inactive bar + active authored segment.
    assert_eq!(b.round_rects.len(), 2);
    assert_eq!(b.round_rects[0].1, PURPLE_BORDER);
    assert!(
        b.round_rects.iter().any(|(_, c)| *c == DARK_PURPLE),
        "active tab uses authored fill"
    );
    let active = b
        .text_colors
        .iter()
        .find(|(text, _)| text == "One")
        .map(|(_, color)| *color)
        .expect("active tab color");
    let inactive = b
        .text_colors
        .iter()
        .find(|(text, _)| text == "Two")
        .map(|(_, color)| *color)
        .expect("inactive tab color");
    assert_eq!(
        (inactive.r(), inactive.g(), inactive.b()),
        (active.r(), active.g(), active.b())
    );
    assert!(inactive.a() < active.a(), "inactive tab label is muted");
}

/// Skia treats `set_stroke_width(0.0)` as a hairline, not as "draw nothing",
/// so a `thickness: 0` stroke still outlined the tabs bar. select /
/// text_field already gated on width; tabs did not.
#[test]
fn tabs_zero_width_stroke_paints_no_bar_outline() {
    let rect = Rect::xywh(0.0, 0.0, 240.0, 120.0);
    let mut node = authored_widget_node(
        NodeKind::Frame,
        SceneWidget {
            kind: "tabs".into(),
            value_str: Some("one".into()),
            options: vec![SceneWidgetOption {
                value: "one".into(),
                label: "One".into(),
            }],
            ..Default::default()
        },
        rect,
        0.0,
    );
    node.stroke = Some(SceneStroke {
        color: PURPLE_BORDER,
        width: 0.0,
        sides: None,
        align: SceneStrokeAlign::Center,
    });

    let b = paint(&node, rect);

    assert!(
        b.stroke_round_rects.is_empty(),
        "a zero-thickness stroke must not outline the bar, got {:?}",
        b.stroke_round_rects
    );

    // The gate is on zero, not on strokes in general.
    node.stroke = Some(SceneStroke {
        color: PURPLE_BORDER,
        width: 2.0,
        sides: None,
        align: SceneStrokeAlign::Center,
    });
    assert_eq!(paint(&node, rect).stroke_round_rects.len(), 1);
}
