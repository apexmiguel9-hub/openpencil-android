//! Family-aware text measurement for editor chrome.
//!
//! **Chrome code must never call `RenderBackend::measure_text` directly.**
//! That call is family-BLIND: it resolves whatever the backend's default
//! typeface is (the bundled Roboto on native, the CanvasKit default in the
//! browser), while every chrome string is *drawn* as a named run —
//! [`CHROME_FONT_FAMILY`] — which native resolves through the system
//! `FontMgr` (`.AppleSystemUIFont` / SF Pro on macOS) and the browser
//! resolves through CSS font matching. jian says as much on the trait
//! itself (`jian_widgets::painter::Painter::measure_text_family`).
//!
//! SF Pro is wider than Roboto at the same point size, so a family-blind
//! measurement *under-reports* the painted width. The failure is silent by
//! construction:
//!
//! - an ellipsizer believes a string fits, emits no `…`, and the content
//!   clip shears the last glyph in half at the column edge;
//! - a `(container - measured) / 2` centring lands the run visibly left of
//!   centre;
//! - a container sized as `measured + padding` (a tooltip bubble, a pill,
//!   a chip) is born too narrow to hold its own label;
//! - a caret drawn at `measure(&text[..pos])` drifts away from the glyph
//!   the user is editing, further with every character.
//!
//! None of those raise an error, and none of them reproduce under the test
//! backends (whose blind and family-aware measurements agree), which is why
//! this class of bug is only ever found by eye on a real machine.
//!
//! Everything here routes through `measure_text_family` with the family the
//! run is actually painted in. Reach for [`measure_chrome`] where you used
//! to reach for `measure_text`, [`fit_chrome`] to ellipsize, and
//! [`centered_text_x`] to centre. `tools/check-text-measure.sh` fails the
//! build when a widget module goes back to the blind call.

use crate::{Rect, RenderBackend};

/// The font family every chrome string is DRAWN with — the `family`
/// argument of the `TextLayout::single_run` calls all over `widgets/`.
/// Measurement has to name the same one.
pub const CHROME_FONT_FAMILY: &str = "system-ui";

/// Painted width of `text` at `font_size` in the chrome font family.
///
/// The drop-in replacement for `backend.measure_text(text, font_size)`.
pub fn measure_chrome(backend: &mut dyn RenderBackend, text: &str, font_size: f32) -> f32 {
    backend.measure_text_family(text, font_size, CHROME_FONT_FAMILY)
}

/// Painted width of `text` at `font_size` and `weight` in the chrome font
/// family — for runs that carry a `.with_font_weight(…)`, where the bold
/// face's advances differ from the regular one.
pub fn measure_chrome_weighted(
    backend: &mut dyn RenderBackend,
    text: &str,
    font_size: f32,
    weight: u16,
) -> f32 {
    backend.measure_text_family_styled(text, font_size, CHROME_FONT_FAMILY, weight, false)
}

/// Painted width of `text` in an explicitly named `family` — for the few
/// chrome runs that are not [`CHROME_FONT_FAMILY`] (monospace readouts,
/// the font-picker's preview rows, which paint each entry in its own face).
pub fn measure_in_family(
    backend: &mut dyn RenderBackend,
    text: &str,
    font_size: f32,
    family: &str,
) -> f32 {
    backend.measure_text_family(text, font_size, family)
}

/// Ellipsize `text` with a trailing `…` until it fits `max_w`, measured in
/// the family it will be drawn in.
pub fn fit_chrome(
    backend: &mut dyn RenderBackend,
    text: &str,
    max_w: f32,
    font_size: f32,
) -> String {
    crate::util::ellipsize_to_width(text, max_w, |s| measure_chrome(backend, s, font_size))
}

/// Ellipsize `text` to `max_w` measured in an explicitly named `family` —
/// the [`fit_chrome`] twin for runs a jian component paints in its own face.
pub fn fit_in_family(
    backend: &mut dyn RenderBackend,
    text: &str,
    max_w: f32,
    font_size: f32,
    family: &str,
) -> String {
    crate::util::ellipsize_to_width(text, max_w, |s| {
        measure_in_family(backend, s, font_size, family)
    })
}

/// Family jian's `SelectTrigger` paints its value in, and the horizontal
/// space its own chrome takes: `PAD_X`(8) either side plus the 14px chevron
/// and its 4px gutter. Mirrored from
/// `jian_widgets::components::select_trigger` — a vendored component we may
/// not edit, and one that **clips** its value rather than ellipsizing it.
const SELECT_TRIGGER_FAMILY: &str = "Inter";
const SELECT_TRIGGER_INSET: f32 = 8.0 + 8.0 + 14.0 + 4.0;

/// Fit a value for a jian `SelectTrigger` occupying `rect`.
///
/// The component clips its value to the box, so an over-long localized value
/// is cut mid-glyph with nothing to signal it. Fitting here turns that into
/// an ellipsis — and it must measure in the component's own family, not the
/// chrome one.
pub fn fit_select_trigger_label(
    backend: &mut dyn RenderBackend,
    label: &str,
    rect: Rect,
    font_size: f32,
) -> String {
    fit_in_family(
        backend,
        label,
        (rect.size.x - SELECT_TRIGGER_INSET).max(0.0),
        font_size,
        SELECT_TRIGGER_FAMILY,
    )
}

/// Left edge that centres `text` horizontally inside `rect`.
///
/// Centring on a family-blind width biases the run left by half the
/// measurement error, which is what makes a "centred" button label read as
/// slightly off on a real machine but perfectly centred in every test.
pub fn centered_text_x(
    backend: &mut dyn RenderBackend,
    text: &str,
    font_size: f32,
    rect: Rect,
) -> f32 {
    rect.origin.x + (rect.size.x - measure_chrome(backend, text, font_size)) / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::test_family_gap_backend::FamilyGapBackend;

    #[test]
    fn measure_chrome_reports_the_named_family_not_the_blind_default() {
        let mut backend = FamilyGapBackend::default();
        let blind = backend.measure_text("Doubao", 13.0);

        let painted = measure_chrome(&mut backend, "Doubao", 13.0);

        assert!(
            painted > blind,
            "chrome measurement must resolve {CHROME_FONT_FAMILY}, not the backend default"
        );
    }

    #[test]
    fn fit_chrome_ellipsizes_against_the_painted_width() {
        let mut backend = FamilyGapBackend::default();
        let text = "A very long provider name that cannot fit";
        let max_w = 60.0;

        let fitted = fit_chrome(&mut backend, text, max_w, 13.0);

        assert!(fitted.ends_with('…'), "expected truncation: {fitted:?}");
        assert!(
            measure_chrome(&mut backend, &fitted, 13.0) <= max_w,
            "fitted text must fit the column when measured in its paint family"
        );
    }

    #[test]
    fn centered_text_x_centres_on_the_painted_width() {
        let mut backend = FamilyGapBackend::default();
        let rect = Rect::xywh(10.0, 0.0, 200.0, 24.0);

        let x = centered_text_x(&mut backend, "Connect", 12.0, rect);

        let w = measure_chrome(&mut backend, "Connect", 12.0);
        let left_gap = x - rect.origin.x;
        let right_gap = rect.origin.x + rect.size.x - (x + w);
        assert!(
            (left_gap - right_gap).abs() < 0.01,
            "label should sit centred: {left_gap} vs {right_gap}"
        );
    }
}
