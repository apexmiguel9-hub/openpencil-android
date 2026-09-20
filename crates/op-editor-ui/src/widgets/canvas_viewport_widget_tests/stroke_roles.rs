//! Stroke-role fallbacks and the zero-width stroke gate for the
//! design-canvas widget painter.
//!
//! Split out of the `canvas_viewport_widget_tests.rs` spine (800-line
//! ceiling); the shared recorder + node fixtures come in via `use super::*`.

use super::*;

/// The paint-side half of the "no fabricated black stroke" contract: with the
/// loader now dropping an unpainted widget stroke (see op-pen-loader's
/// `is_unpainted_widget_stroke`), the painter must land on the resolver's role
/// defaults — the legacy `#D1D5DB` off-track and a borderless select — rather
/// than on the opaque black it used to be handed.
#[test]
fn strokeless_widgets_use_role_defaults_not_black() {
    const TRACK_OFF: Color = Color::rgb_u8(0xD1, 0xD5, 0xDB);

    let switch_rect = Rect::xywh(0.0, 0.0, 40.0, 20.0);
    let switch = widget_node(
        NodeKind::Rect,
        SceneWidget {
            kind: "switch".into(),
            checked: Some(false),
            ..Default::default()
        },
        switch_rect,
    );
    let b = paint(&switch, switch_rect);
    assert_eq!(
        b.round_rects[0].1, TRACK_OFF,
        "off-track falls back to the legacy inactive token"
    );

    let select_rect = Rect::xywh(0.0, 0.0, 160.0, 36.0);
    let select = widget_node(
        NodeKind::Text,
        SceneWidget {
            kind: "select".into(),
            placeholder: Some("Pick one".into()),
            ..Default::default()
        },
        select_rect,
    );
    let b = paint(&select, select_rect);
    assert!(
        b.stroke_round_rects.is_empty(),
        "an unstyled select stays borderless, got {:?}",
        b.stroke_round_rects
    );
}
