//! Cross-role contract closure for the design-canvas widget painter.

use super::*;

fn fold_direct_scene_opacity(mut node: SceneNode, opacity: f32) -> SceneNode {
    node.opacity = opacity;
    if let Some(fill) = node.fill.as_mut() {
        fill.a *= opacity;
    }
    if let Some(stroke) = node.stroke.as_mut() {
        stroke.color.a *= opacity;
    }
    node
}

fn text_alpha(recorder: &WidgetRecorder, text: &str) -> u8 {
    recorder
        .text_colors
        .iter()
        .find(|(content, _)| content == text)
        .map(|(_, color)| color.a())
        .unwrap_or_else(|| panic!("missing text paint for {text}"))
}

#[test]
fn indeterminate_progress_ignores_value_and_paints_stable_segment() {
    let rect = Rect::xywh(10.0, 20.0, 200.0, 8.0);
    let node = authored_widget_node(
        NodeKind::Rect,
        SceneWidget {
            kind: "progress".into(),
            value_num: Some(99.0),
            max: Some(100.0),
            indeterminate: true,
            ..Default::default()
        },
        rect,
        4.0,
    );

    let recorder = paint(&node, rect);

    assert_eq!(recorder.round_rects.len(), 2, "track plus one segment");
    assert_eq!(recorder.round_rects[0].0, rect, "full inactive track");
    assert_eq!(
        recorder.round_rects[1].0,
        Rect::xywh(75.0, 20.0, 70.0, 8.0),
        "segment is x=32.5%, width=35% regardless of value"
    );
}

#[test]
fn unstyled_switch_fallback_tracks_follow_scene_opacity() {
    let rect = Rect::xywh(0.0, 0.0, 40.0, 20.0);
    for (opacity, expected_alpha) in [(0.0, 0), (0.5, 128)] {
        for checked in [false, true] {
            let mut node = widget_node(
                NodeKind::Rect,
                SceneWidget {
                    kind: "switch".into(),
                    checked: Some(checked),
                    ..Default::default()
                },
                rect,
            );
            node.opacity = opacity;
            let recorder = paint(&node, rect);
            assert!(
                recorder
                    .round_rects
                    .iter()
                    .all(|(_, color)| color.to_jian().a() == expected_alpha),
                "legacy track and derived knob must both follow opacity={opacity}"
            );
        }
    }
}

#[test]
fn styled_text_field_distinguishes_absent_and_explicit_zero_radius() {
    let rect = Rect::xywh(0.0, 0.0, 160.0, 36.0);
    let widget = SceneWidget {
        kind: "text_input".into(),
        value_str: Some("hello".into()),
        ..Default::default()
    };
    let mut absent = widget_node(NodeKind::Text, widget.clone(), rect);
    absent.fill = Some(DARK_PURPLE);
    absent.stroke = Some(SceneStroke {
        color: PURPLE_BORDER,
        width: 2.0,
        sides: None,
        align: SceneStrokeAlign::Center,
    });
    let explicit_zero = authored_widget_node(NodeKind::Text, widget, rect, 0.0);

    let absent = paint(&absent, rect);
    let explicit_zero = paint(&explicit_zero, rect);
    assert_eq!(
        (absent.round_radii[0], absent.stroke_round_radii[0]),
        (6.0, 6.0)
    );
    assert_eq!(
        (
            explicit_zero.round_radii[0],
            explicit_zero.stroke_round_radii[0]
        ),
        (0.0, 0.0)
    );
}

#[test]
fn every_derived_widget_role_applies_scene_opacity_once() {
    for (opacity, full_alpha, muted_alpha) in [(0.0, 0, 0), (0.5, 128, 83)] {
        let checkbox_rect = Rect::xywh(0.0, 0.0, 180.0, 24.0);
        let mut checkbox = authored_widget_node(
            NodeKind::Rect,
            SceneWidget {
                kind: "checkbox".into(),
                checked: Some(true),
                label: Some("Agree".into()),
                ..Default::default()
            },
            checkbox_rect,
            4.0,
        );
        checkbox.stroke.as_mut().unwrap().color = Color::rgb_u8(0xf4, 0xf4, 0xf5);
        let checkbox = paint(&fold_direct_scene_opacity(checkbox, opacity), checkbox_rect);
        assert_eq!(checkbox.round_rects[0].1.to_jian().a(), full_alpha);
        assert_eq!(checkbox.stroke_round_rects[0].1.to_jian().a(), full_alpha);
        assert!(checkbox
            .lines
            .iter()
            .all(|(_, _, color)| color.to_jian().a() == full_alpha));
        assert_eq!(text_alpha(&checkbox, "Agree"), full_alpha);

        let switch_rect = Rect::xywh(0.0, 0.0, 40.0, 20.0);
        let switch = authored_widget_node(
            NodeKind::Rect,
            SceneWidget {
                kind: "switch".into(),
                checked: Some(false),
                ..Default::default()
            },
            switch_rect,
            10.0,
        );
        let switch = paint(&fold_direct_scene_opacity(switch, opacity), switch_rect);
        assert_eq!(
            switch.round_rects[1].1.to_jian().a(),
            full_alpha,
            "inactive-derived knob"
        );

        let select_rect = Rect::xywh(0.0, 0.0, 160.0, 36.0);
        let select = authored_widget_node(
            NodeKind::Text,
            SceneWidget {
                kind: "select".into(),
                value_str: Some("night".into()),
                options: vec![SceneWidgetOption {
                    value: "night".into(),
                    label: "Night".into(),
                }],
                ..Default::default()
            },
            select_rect,
            6.0,
        );
        let select = paint(&fold_direct_scene_opacity(select, opacity), select_rect);
        assert_eq!(text_alpha(&select, "Night"), full_alpha, "field foreground");
        assert!(
            select
                .lines
                .iter()
                .all(|(_, _, color)| color.to_jian().a() == muted_alpha),
            "select chevron uses muted field foreground"
        );

        let tabs_rect = Rect::xywh(0.0, 0.0, 240.0, 120.0);
        let tabs = authored_widget_node(
            NodeKind::Frame,
            SceneWidget {
                kind: "tabs".into(),
                value_str: Some("one".into()),
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
            tabs_rect,
            0.0,
        );
        let tabs = paint(&fold_direct_scene_opacity(tabs, opacity), tabs_rect);
        assert_eq!(
            text_alpha(&tabs, "One"),
            full_alpha,
            "active external label"
        );
        assert_eq!(
            text_alpha(&tabs, "Two"),
            muted_alpha,
            "muted external label"
        );
    }
}
