//! The gallery panels' control scale — one named ladder every control in
//! the Asset Center (and the Prompt Center chrome it shares) measures
//! against.
//!
//! Written for the same reason `agent_settings_metrics` was: before this
//! module the Asset Center carried a 28 px chip, a 30 px card button, a
//! 32 px close button, and two 38 px text fields, each declared next to the
//! code that painted it. Nothing was wrong in isolation. Together they read
//! as a panel assembled from four different kits, because that is what it
//! was — and the seventh height was always one feature away.
//!
//! Two heights, not one. A chip and a text field are genuinely different
//! controls: a chip is a tap target for a word, a field is a place to put a
//! caret, and forcing them to the same box makes one of them wrong. Every
//! *other* number here is shared.
//!
//! A height or radius literal in a sibling module is a bug unless it is the
//! intrinsic size of one specific thing (a 16 px glyph, a 10 px palette
//! band), which is that thing's business rather than the layout's.

use crate::Color;

/// Text fields and the buttons that sit on their row.
///
/// The search box, the topic field, and the generate button are one line of
/// the panel and read as one control each way they are wrong: different
/// heights make the row look broken, different radii make the button look
/// pasted on.
pub(crate) const CONTROL_H: f32 = 36.0;

/// Corner radius of everything [`CONTROL_H`] tall.
///
/// Not a pill. A field this wide rounded to its half-height reads as a
/// search *pill* — fine for a lone omnibox, wrong beside a rectangular
/// grid, and wrong against the 12 px card corners it sits above.
pub(crate) const CONTROL_RADIUS: f32 = 9.0;

/// Chips: filter pills, the segmented control's segments, the basis chip,
/// the Styles tab's import action, and a card's own action buttons.
///
/// 30 px on purpose — it is the settings modal's `TAB_HEIGHT`, so a tab in
/// the gallery and a tab in the settings dialog are the same object at the
/// same size rather than two 2 px-apart approximations of one.
pub(crate) const CHIP_H: f32 = 30.0;

/// Chips are pills: their radius is always half their height, so a chip
/// that changes height cannot forget to change shape.
pub(crate) const CHIP_RADIUS: f32 = CHIP_H / 2.0;

/// A text field must not be a pill. Checked at compile time rather than in a
/// test, because the failure is someone raising [`CONTROL_RADIUS`] to "match
/// the chips" and quietly turning the search box into an omnibox.
const _: () = assert!(CONTROL_RADIUS * 2.0 < CONTROL_H);

/// Horizontal padding either side of a chip's label.
pub(crate) const CHIP_PAD_X: f32 = 13.0;

/// Space between adjacent chips in a row.
pub(crate) const CHIP_GAP: f32 = 8.0;

/// Label size shared by every chip and segment, so the filter row and the
/// tab row set at the same size.
pub(crate) const CHIP_LABEL_SIZE: f32 = 12.0;

/// The inset between a segmented control's track and its segments.
///
/// What makes a segmented control a segmented control rather than a row of
/// buttons: the segments sit *inside* a shared trough, seam to seam, so the
/// group reads as one control with a moving selection instead of as two
/// things that happen to be adjacent.
pub(crate) const SEGMENT_TRACK_PAD: f32 = 3.0;

/// Height of a segmented control's track, segments included.
pub(crate) const SEGMENT_TRACK_H: f32 = CHIP_H + SEGMENT_TRACK_PAD * 2.0;

/// Narrowest a segment gets, whatever its label measures.
///
/// Segments are equal-width and sized to the longest label in the group, so
/// the selection travels a constant distance and a locale with a short word
/// ("模板") does not produce a segment too small to aim at.
pub(crate) const SEGMENT_MIN_W: f32 = 88.0;

/// How far a filled surface moves toward white on hover, and toward black
/// on press, as a fraction of the distance to that end.
///
/// A *ladder*, not a wash. The generic `button_hover` overlay is 6% ink,
/// which is the right amount on a neutral chip and invisible on a saturated
/// primary button — the one control in the panel that most needs to answer
/// the pointer. Working in the base colour's own lightness gives the same
/// perceived step on both.
pub(crate) const HOVER_LIFT: f32 = 0.12;
pub(crate) const PRESS_DROP: f32 = 0.10;

/// `base` shaded for the pointer state it is in.
///
/// Press wins over hover: the pointer is always over a control it is
/// pressing, and a press that read as a hover would make the button feel
/// like it had not registered.
pub(crate) fn control_fill(base: Color, hovered: bool, pressed: bool) -> Color {
    if pressed {
        mix(base, Color::BLACK, PRESS_DROP)
    } else if hovered {
        mix(base, Color::WHITE, HOVER_LIFT)
    } else {
        base
    }
}

/// `from` moved `amount` of the way to `to`, alpha untouched.
///
/// Alpha is deliberately left alone: these colours are opaque surfaces, and
/// blending it would make a hover on a translucent token also make it more
/// opaque, which reads as the control growing rather than lighting up.
pub(crate) fn mix(from: Color, to: Color, amount: f32) -> Color {
    let amount = amount.clamp(0.0, 1.0);
    Color {
        r: from.r + (to.r - from.r) * amount,
        g: from.g + (to.g - from.g) * amount,
        b: from.b + (to.b - from.b) * amount,
        a: from.a,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ladder has to be visible in both directions and monotonic, or
    /// hover and press are decoration.
    #[test]
    fn the_pointer_ladder_steps_up_on_hover_and_down_on_press() {
        let base = Color::rgb_u8(0x25, 0x63, 0xEB);
        let hovered = control_fill(base, true, false);
        let pressed = control_fill(base, false, true);

        assert!(hovered.r > base.r && hovered.g > base.g && hovered.b > base.b);
        assert!(pressed.r < base.r && pressed.g < base.g && pressed.b < base.b);
        // A step smaller than a couple of levels out of 255 is a step
        // nobody sees on a real display.
        assert!((hovered.b - base.b).abs() * 255.0 > 2.0);
        assert!((base.b - pressed.b).abs() * 255.0 > 2.0);
    }

    #[test]
    fn pressing_beats_hovering_because_the_pointer_is_over_both() {
        let base = Color::rgb_u8(0x25, 0x63, 0xEB);
        assert_eq!(
            control_fill(base, true, true),
            control_fill(base, false, true)
        );
    }

    #[test]
    fn the_ladder_leaves_alpha_alone() {
        let translucent = Color::rgb_u8(0x20, 0x20, 0x20).with_alpha(0.4);
        assert_eq!(control_fill(translucent, true, false).a, 0.4);
        assert_eq!(control_fill(translucent, false, true).a, 0.4);
    }

    /// A chip is a pill by construction, so changing its height cannot
    /// leave the shape behind.
    #[test]
    fn a_chip_radius_always_follows_its_height() {
        assert_eq!(CHIP_RADIUS * 2.0, CHIP_H);
    }
}
