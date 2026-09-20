//! Scene scalars → OOXML units.
//!
//! DrawingML measures nothing in pixels. Every function here is a pure
//! map from the doc-px / radian / 0..1 world the scene speaks into the
//! integer unit the XML expects, and every one of them returns an
//! INTEGER: OOXML attributes are xsd integer types, and a value that
//! formatted as `1.8288e7` would be rejected by the schema before
//! PowerPoint ever saw the slide.

use op_editor_ui::Color;

/// English Metric Units per doc pixel.
///
/// EMU is defined as 914400 per inch, and the canonical `.op` document
/// is authored at the CSS reference resolution of 96 px per inch, so
/// one pixel is exactly 9525 EMU with no rounding in the constant
/// itself. A 1920×1080 board therefore lands as 18288000×10287000 EMU
/// — a real 20×11.25 inch slide, 16:9 to the last unit.
pub const EMU_PER_PX: f64 = 9525.0;

/// The largest coordinate OOXML accepts (`ST_Coordinate` tops out at
/// 2^31-1 EMU). Clamping instead of wrapping keeps a corrupt bound from
/// producing a negative offset that PowerPoint reads as garbage.
const MAX_EMU: i64 = 2_147_483_647;

/// Doc pixels → EMU, rounded to the nearest unit.
pub fn emu(px: f32) -> i64 {
    if !px.is_finite() {
        return 0;
    }
    let v = (px as f64 * EMU_PER_PX).round();
    (v as i64).clamp(-MAX_EMU, MAX_EMU)
}

/// Doc pixels → EMU for a LENGTH, which may not be negative or zero:
/// a shape with `cx="0"` is legal XML that PowerPoint draws as nothing,
/// so a degenerate rect is floored to one EMU rather than vanishing.
pub fn emu_extent(px: f32) -> i64 {
    emu(px).max(1)
}

/// Doc pixels → hundredths of a point, the unit of `sz` on a run and of
/// `spcPts` on a paragraph.
///
/// CSS pixels are 1/96 inch and points are 1/72, so a pixel is exactly
/// 0.75 pt: a 32 px heading is 24 pt, the size PowerPoint's own font
/// box will show. Clamped to the `ST_TextFontSize` range (1 pt..4000
/// pt) that the schema enforces.
pub fn font_hundredths_pt(px: f32) -> i64 {
    if !px.is_finite() {
        return 1200;
    }
    ((px as f64 * 75.0).round() as i64).clamp(100, 400_000)
}

/// Doc pixels → hundredths of a point WITHOUT the font-size clamp, for
/// letter spacing (which is legally negative) and line spacing.
pub fn hundredths_pt(px: f32) -> i64 {
    if !px.is_finite() {
        return 0;
    }
    ((px as f64 * 75.0).round() as i64).clamp(-400_000, 400_000)
}

/// Radians clockwise → the 1/60000-degree clockwise integer that every
/// DrawingML rotation attribute uses, normalized into `[0, 360)`.
///
/// Both models turn clockwise about the shape's centre, so this is a
/// unit change and nothing else.
pub fn rot_60k(radians: f32) -> i64 {
    if !radians.is_finite() || radians == 0.0 {
        return 0;
    }
    degrees_60k(radians.to_degrees())
}

/// Degrees clockwise → 1/60000-degree, normalized into `[0, 360)`.
pub fn degrees_60k(degrees: f32) -> i64 {
    if !degrees.is_finite() {
        return 0;
    }
    let normalized = degrees.rem_euclid(360.0);
    ((normalized as f64 * 60_000.0).round() as i64).rem_euclid(21_600_000)
}

/// A `0.0..=1.0` fraction → the 1/1000-percent integer DrawingML uses
/// for alpha, gradient stop positions and crop insets.
pub fn pct_1000(fraction: f32) -> i64 {
    if !fraction.is_finite() {
        return 0;
    }
    ((fraction.clamp(0.0, 1.0) as f64) * 100_000.0).round() as i64
}

/// Same scale as [`pct_1000`] but for a value that is legally negative
/// (a `fillRect` inset expands the fill area when negative).
pub fn signed_pct_1000(fraction: f32) -> i64 {
    if !fraction.is_finite() {
        return 0;
    }
    ((fraction.clamp(-10.0, 10.0) as f64) * 100_000.0).round() as i64
}

/// A resolved scene colour as the six upper-case hex digits `srgbClr`
/// wants. The alpha channel is NOT part of it — see [`alpha_child`].
pub fn srgb(c: Color) -> String {
    let channel = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!(
        "{:02X}{:02X}{:02X}",
        channel(c.r),
        channel(c.g),
        channel(c.b)
    )
}

/// The `<a:alpha/>` child of a colour element, or an empty string when
/// the colour is opaque.
///
/// `scale` folds in the inherited composite opacity of every ancestor.
/// DrawingML has no per-shape opacity property at all: a translucent
/// group in the scene can only reach the slide by multiplying into the
/// alpha of each colour its subtree paints, which is what the walk does
/// by threading a scale factor down and passing it here.
pub fn alpha_child(c: Color, scale: f32) -> String {
    let a = (c.a * scale.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    if a >= 0.999 {
        return String::new();
    }
    format!("<a:alpha val=\"{}\"/>", pct_1000(a))
}

/// A complete `<a:srgbClr>` element for `c` at the inherited `scale`.
pub fn color_element(c: Color, scale: f32) -> String {
    let alpha = alpha_child(c, scale);
    if alpha.is_empty() {
        return format!("<a:srgbClr val=\"{}\"/>", srgb(c));
    }
    format!("<a:srgbClr val=\"{}\">{alpha}</a:srgbClr>", srgb(c))
}

/// A complete `<a:solidFill>` element for `c` at the inherited `scale`.
pub fn solid_fill(c: Color, scale: f32) -> String {
    format!("<a:solidFill>{}</a:solidFill>", color_element(c, scale))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_hd_board_is_exactly_sixteen_by_nine_in_emu() {
        assert_eq!(emu(1920.0), 18_288_000);
        assert_eq!(emu(1080.0), 10_287_000);
    }

    #[test]
    fn every_unit_formats_as_a_plain_integer() {
        // The bug this guards: an f32 formatted with `{}` reaches
        // scientific notation around 1e7, which is inside the EMU range
        // a normal slide uses.
        for px in [0.0, 1.5, 1920.0, 12_345.678] {
            let s = emu(px).to_string();
            assert!(!s.contains('e'), "{s}");
            assert!(!s.contains('.'), "{s}");
        }
    }

    #[test]
    fn pixels_convert_to_points_at_three_quarters() {
        assert_eq!(font_hundredths_pt(32.0), 2400);
        assert_eq!(font_hundredths_pt(16.0), 1200);
    }

    #[test]
    fn a_degenerate_extent_is_floored_rather_than_dropped() {
        assert_eq!(emu_extent(0.0), 1);
        assert_eq!(emu_extent(-3.0), 1);
    }

    #[test]
    fn rotation_normalizes_into_one_turn() {
        assert_eq!(rot_60k(0.0), 0);
        assert_eq!(rot_60k(std::f32::consts::FRAC_PI_2), 5_400_000);
        // -90 degrees is 270, not a negative attribute value.
        assert_eq!(rot_60k(-std::f32::consts::FRAC_PI_2), 16_200_000);
    }

    #[test]
    fn an_opaque_colour_writes_no_alpha_child() {
        let opaque = Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        assert_eq!(alpha_child(opaque, 1.0), "");
        assert_eq!(srgb(opaque), "FF0000");
        // An opaque colour under a half-transparent ancestor is not.
        assert_eq!(alpha_child(opaque, 0.5), "<a:alpha val=\"50000\"/>");
    }
}
