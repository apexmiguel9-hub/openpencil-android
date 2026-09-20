//! Small pure conversion helpers for the CanvasKit backend.
//!
//! Split out of `canvaskit.rs`: DPR resolution, gradient-stop flattening, and
//! the enum → wire-code mappings the flat `OpCk` FFI takes as scalars.

use op_editor_ui::{Color, ImageBlendMode, ImageDrawMode};

/// Minimum backing-store scale used by the web host.
///
/// Some embedded browsers report a DPR of 1 even on a HiDPI display. Text in
/// the web host is rasterized through a browser canvas before CanvasKit draws
/// it, so a 1x backing store leaves small glyphs visibly softer than native.
const MIN_WEB_RENDER_DPR: f32 = 2.0;

/// Use the browser's full device-pixel ratio for the CanvasKit backing store,
/// with a 2x quality floor for browsers and webviews that report DPR 1.
///
/// Capping the surface by viewport area made large HiDPI windows render below
/// their native resolution and left CSS to upscale the result. That saved GPU
/// memory, but it also softened every glyph and one-pixel chrome edge. Native
/// hosts render at the display scale, so the web host must do the same.
pub(super) fn display_dpr(native_dpr: f32) -> f32 {
    (if native_dpr.is_finite() {
        native_dpr
    } else {
        MIN_WEB_RENDER_DPR
    })
    .max(MIN_WEB_RENDER_DPR)
}

pub(super) fn flatten_gradient_stops(stops: &[(f32, Color)]) -> Vec<f32> {
    let mut flat = Vec::with_capacity(stops.len() * 5);
    for (offset, color) in stops {
        flat.extend([*offset, color.r, color.g, color.b, color.a]);
    }
    flat
}

pub(super) fn flatten_gradient_colors(colors: &[Color]) -> Vec<f32> {
    let mut flat = Vec::with_capacity(colors.len() * 4);
    for color in colors {
        flat.extend([color.r, color.g, color.b, color.a]);
    }
    flat
}

pub(super) fn image_draw_mode_code(mode: ImageDrawMode) -> u8 {
    match mode {
        ImageDrawMode::Fill => 0,
        ImageDrawMode::Fit => 1,
        ImageDrawMode::Crop => 2,
        ImageDrawMode::Tile => 3,
        ImageDrawMode::Stretch => 4,
    }
}

pub(super) fn normalized_tile_scale(scale: f32) -> f32 {
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

pub(super) fn valid_original_size(size: Option<[f32; 2]>) -> Option<[f32; 2]> {
    size.filter(|[width, height]| {
        width.is_finite() && height.is_finite() && *width > 0.0 && *height > 0.0
    })
}

pub(super) fn image_blend_mode_code(mode: ImageBlendMode) -> u8 {
    match mode {
        ImageBlendMode::Normal => 0,
        ImageBlendMode::Darken => 1,
        ImageBlendMode::Multiply => 2,
        ImageBlendMode::Screen => 3,
        ImageBlendMode::Overlay => 4,
        ImageBlendMode::Lighten => 5,
        ImageBlendMode::Difference => 6,
        ImageBlendMode::Hue => 7,
        ImageBlendMode::Saturation => 8,
        ImageBlendMode::Color => 9,
        ImageBlendMode::Luminosity => 10,
        ImageBlendMode::SoftLight => 11,
        ImageBlendMode::ColorDodge => 12,
        ImageBlendMode::ColorBurn => 13,
        ImageBlendMode::HardLight => 14,
        ImageBlendMode::Exclusion => 15,
    }
}

pub(super) fn svg_path_even_odd(d: &str) -> bool {
    d.matches(['Z', 'z']).count() > 1
}
