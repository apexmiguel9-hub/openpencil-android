use super::*;
use crate::widgets::panel_control_metrics::{CHIP_H, SEGMENT_TRACK_H, SEGMENT_TRACK_PAD};
use crate::widgets::test_capture_backend::CaptureBackend;

const TRACK: Rect = Rect {
    origin: Point2D { x: 40.0, y: 20.0 },
    size: Point2D {
        x: 200.0,
        y: SEGMENT_TRACK_H,
    },
};

fn states(selected: usize, hovered: Option<usize>) -> Vec<SegmentState> {
    (0..2)
        .map(|index| SegmentState {
            selected: index == selected,
            hovered: hovered == Some(index),
            pressed: false,
        })
        .collect()
}

fn paint(states: &[SegmentState]) -> CaptureBackend {
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    paint_segmented_control(&mut cx, &Theme::dark(), TRACK, &["模板", "风格"], states);
    backend
}

/// Seam to seam inside the track, and never outside it. A gap between
/// segments is what turns a segmented control back into two buttons.
#[test]
fn segments_tile_the_track_with_no_seam_gap() {
    let rects = segment_rects(TRACK, 2);
    assert_eq!(rects.len(), 2);
    assert_eq!(rects[0].origin.x, TRACK.origin.x + SEGMENT_TRACK_PAD);
    assert_eq!(
        rects[0].origin.x + rects[0].size.x,
        rects[1].origin.x,
        "segments must touch"
    );
    assert_eq!(
        rects[1].origin.x + rects[1].size.x,
        TRACK.origin.x + TRACK.size.x - SEGMENT_TRACK_PAD
    );
    for rect in rects {
        assert_eq!(rect.size.y, CHIP_H);
        assert_eq!(rect.origin.y, TRACK.origin.y + SEGMENT_TRACK_PAD);
    }
}

/// The three states a segment can be in must be three different pictures.
/// This is the assertion the old two-lone-pills row could not have passed:
/// its selected and hovered tabs differed only in which near-identical
/// neutral token they filled with.
#[test]
fn selected_hovered_and_resting_segments_are_visually_distinct() {
    let theme = Theme::dark();

    let resting = paint(&states(0, None));
    let hovered = paint(&states(0, Some(1)));
    let selected_hovered = paint(&states(0, Some(0)));

    // Resting: the trough plus exactly one filled segment.
    assert_eq!(
        resting.round_fills.len(),
        2,
        "an unselected segment must paint no surface of its own"
    );
    let (track_rect, _, track_fill) = resting.round_fills[0];
    assert_eq!(track_rect, TRACK);
    let (_, _, selected_fill) = resting.round_fills[1];
    assert_ne!(
        selected_fill, track_fill,
        "the selection must stand off its own trough"
    );
    assert_eq!(selected_fill, theme.primary);

    // Hovering the *other* segment adds a wash without moving the fill.
    assert_eq!(hovered.round_fills.len(), 3);
    assert_eq!(hovered.round_fills[2].2, theme.button_hover);

    // Hovering the selected one lifts the fill instead of washing over it.
    assert_eq!(selected_hovered.round_fills.len(), 2);
    let lifted = selected_hovered.round_fills[1].2;
    assert_ne!(lifted, selected_fill, "a hovered selection must respond");
    assert!(lifted.r > selected_fill.r && lifted.b > selected_fill.b);
}

/// The trough has to read as inset against the surface it sits on, or the
/// control is a row of buttons with a rectangle behind it.
#[test]
fn the_track_reads_as_inset_against_the_panel_surface() {
    for theme in [Theme::dark(), Theme::light()] {
        let track = track_color(&theme);
        assert_ne!(track, theme.popover);
        assert_ne!(track, theme.muted, "a flat muted fill is not a trough");
    }
}

/// Every segment is the same width whatever its label measures, so the
/// selection travels a constant distance.
#[test]
fn segments_are_equal_width_and_sized_to_the_longest_label() {
    let short = segment_width_for(&["A", "B"]);
    let long = segment_width_for(&["A", "A considerably longer label"]);

    assert_eq!(
        short, SEGMENT_MIN_W,
        "a short group falls back to the floor"
    );
    assert!(long > short, "the longest label sets the width");
    assert_eq!(
        segment_track_width(2, long),
        long * 2.0 + SEGMENT_TRACK_PAD * 2.0
    );
}

/// A chip has to be an object at rest — the old ones were a faint fill with
/// no edge, which is why the row read as text rather than as controls.
#[test]
fn a_resting_chip_paints_a_surface_and_a_hairline() {
    let theme = Theme::dark();
    let rect = Rect::xywh(0.0, 0.0, 90.0, CHIP_H);
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    paint_panel_chip(
        &mut cx,
        &theme,
        rect,
        "教程图",
        SegmentState {
            selected: false,
            hovered: false,
            pressed: false,
        },
    );
    assert_eq!(backend.round_fills.len(), 1);
    assert_eq!(backend.round_fills[0].2, theme.secondary);
    assert_eq!(backend.round_fills[0].1, CHIP_RADIUS);

    let mut selected = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut selected,
    };
    paint_panel_chip(
        &mut cx,
        &theme,
        rect,
        "教程图",
        SegmentState {
            selected: true,
            hovered: false,
            pressed: false,
        },
    );
    assert_eq!(selected.round_fills[0].2, theme.primary);
}

/// The accent button and the segmented selection ride the same ladder — one
/// "pressable accent surface" in the panel, whatever shape it is cut to.
#[test]
fn the_accent_button_rides_the_same_pointer_ladder_as_the_selection() {
    let theme = Theme::dark();
    let rect = Rect::xywh(0.0, 0.0, 108.0, CONTROL_H_FOR_TEST);
    let fills = |hovered, pressed| {
        let mut backend = CaptureBackend::default();
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        paint_accent_button(
            &mut cx,
            &theme,
            ButtonSpec {
                rect,
                radius: 9.0,
                label: "生成",
                label_size: 13.0,
                hovered,
                pressed,
            },
        );
        backend.round_fills[0].2
    };
    let resting = fills(false, false);
    let hovered = fills(true, false);
    let pressed = fills(false, true);

    assert_eq!(resting, theme.primary);
    assert!(hovered.b > resting.b, "hover must lighten a saturated fill");
    assert!(pressed.b < resting.b, "press must darken it");
}

const CONTROL_H_FOR_TEST: f32 = crate::widgets::panel_control_metrics::CONTROL_H;
