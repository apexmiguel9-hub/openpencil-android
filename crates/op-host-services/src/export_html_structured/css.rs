//! Scene paint values → CSS declaration values.
//!
//! Every function here is a pure `SceneNode`-field → CSS-string map.
//! The conversions that are exact are noted as such; the two that
//! approximate (linear-gradient endpoints, `mix-blend-mode` backdrop
//! scope) say where they drift, because a reader debugging a slide
//! that looks half a shade off needs to know which side lied.

use op_editor_ui::layout_scene::{Effect, SceneGradient, SceneGradientStop, SceneNode};
use op_editor_ui::{Color, ImageBlendMode, Rect};
use std::fmt::Write as _;

/// Format a doc-px scalar. Two decimals sit well under a presented
/// pixel at any realistic projection scale, and trimming the trailing
/// zeros keeps a deck of thousands of nodes from carrying `.00` on
/// every coordinate.
pub fn num(v: f32) -> String {
    // `-0.0 == 0.0`, so this also stops a negated zero offset from
    // formatting as the CSS-legal-but-jarring `-0px`.
    if !v.is_finite() || v == 0.0 {
        return "0".to_string();
    }
    let s = format!("{v:.2}");
    let trimmed = s.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "-" {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Alpha needs more precision than lengths, and must stay in range: at
/// two decimals a 0.005 opacity step would quantise to fully
/// transparent. Only for values that ARE 0..=1 — a line-height
/// multiplier is unitless but unbounded, and goes through [`num`].
pub fn ratio(v: f32) -> String {
    let s = format!("{:.4}", v.clamp(0.0, 1.0));
    let trimmed = s.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

/// A resolved scene colour as a CSS colour. Opaque colours drop the
/// alpha channel so the common case reads as plain `rgb(...)`.
pub fn color(c: Color) -> String {
    let channel = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let (r, g, b) = (channel(c.r), channel(c.g), channel(c.b));
    if c.a >= 0.999 {
        format!("rgb({r},{g},{b})")
    } else {
        format!("rgba({r},{g},{b},{})", ratio(c.a))
    }
}

/// A resolved gradient as a CSS `<image>` value, or `None` when the
/// gradient has no CSS equivalent (mesh) and the caller must raster.
///
/// **Linear angles are exact.** The canonical `.op` convention (0° =
/// bottom→top, 90° = left→right) is literally CSS's `<angle>` for
/// `linear-gradient`, so no conversion is applied. What does drift is
/// the gradient LINE LENGTH: the scene projects endpoints onto half
/// the box extent per axis, while CSS extends the line until the
/// perpendiculars through its ends touch the box corners. The two
/// agree exactly at 0/90/180/270° and diverge by up to the corner
/// factor on a non-square box at an oblique angle — a slightly
/// stretched ramp, never a wrong direction or wrong colour.
///
/// **Radial is exact**: the scene's radius is a fraction of
/// `max(w, h)` and its centre a fraction of the box, which is what
/// `radial-gradient(circle R at X Y)` draws.
pub fn gradient(g: &SceneGradient, bounds: Rect) -> Option<String> {
    match g {
        SceneGradient::Linear {
            angle_deg,
            opacity,
            stops,
        } => Some(format!(
            "linear-gradient({}deg,{})",
            num(*angle_deg),
            stop_list(stops, *opacity)
        )),
        SceneGradient::Radial {
            cx,
            cy,
            radius,
            opacity,
            stops,
        } => {
            let r = (bounds.size.x.max(bounds.size.y) * radius.clamp(0.0, 1.0)).max(0.01);
            Some(format!(
                "radial-gradient(circle {}px at {}px {}px,{})",
                num(r),
                num(bounds.size.x * cx.clamp(0.0, 1.0)),
                num(bounds.size.y * cy.clamp(0.0, 1.0)),
                stop_list(stops, *opacity)
            ))
        }
        // A Gouraud lattice has no CSS spelling. Returning `None` sends
        // the node down the raster path rather than flattening a mesh
        // to its first stop and calling it a gradient.
        SceneGradient::Mesh { .. } => None,
    }
}

/// `SceneGradient::opacity` already carries the fill layer's cumulative
/// alpha (the canvas shader path folds it into the stops), so it is
/// multiplied in here and NOT applied again on the element.
fn stop_list(stops: &[SceneGradientStop], opacity: f32) -> String {
    let mut out = String::new();
    for (i, stop) in stops.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let faded = Color {
            a: stop.color.a * opacity.clamp(0.0, 1.0),
            ..stop.color
        };
        let _ = write!(
            out,
            "{} {}%",
            color(faded),
            num(stop.offset.clamp(0.0, 1.0) * 100.0)
        );
    }
    out
}

/// `border-radius` for a node, honouring per-corner radii. `None` when
/// the node is square-cornered.
///
/// Scene order is top-left, top-right, bottom-right, bottom-left —
/// which is exactly CSS's four-value order, so the array maps across
/// position for position.
pub fn corner_radius(n: &SceneNode) -> Option<String> {
    if let Some([tl, tr, br, bl]) = n.corner_radii {
        if [tl, tr, br, bl].iter().any(|r| *r > 0.0) {
            return Some(format!(
                "{}px {}px {}px {}px",
                num(tl.max(0.0)),
                num(tr.max(0.0)),
                num(br.max(0.0)),
                num(bl.max(0.0))
            ));
        }
        return None;
    }
    (n.corner_radius > 0.0).then(|| format!("{}px", num(n.corner_radius)))
}

/// `transform` for a node's rotation + mirroring, or `None` when the
/// node is upright and unflipped.
///
/// Rotation is clockwise about the bounds centre in both models, and
/// the caller pins `transform-origin` to the centre, so this is exact.
/// Flips are about the node's AGGREGATE bounds centre in the scene; for
/// an unbounded container whose aggregate rect differs from its own,
/// the mirrored result shifts. Bounded nodes — every node a deck board
/// contains — have the two rects equal, so the gap is theoretical here
/// and documented rather than corrected.
pub fn transform(n: &SceneNode) -> Option<String> {
    let mut parts = Vec::new();
    if n.rotation != 0.0 && n.rotation.is_finite() {
        parts.push(format!("rotate({}rad)", num(n.rotation)));
    }
    if n.flip_x {
        parts.push("scaleX(-1)".to_string());
    }
    if n.flip_y {
        parts.push("scaleY(-1)".to_string());
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// The three effect channels a node can drive, already formatted.
/// Empty strings mean "emit no declaration".
#[derive(Default)]
pub struct EffectCss {
    pub box_shadow: String,
    pub filter: String,
    pub backdrop_filter: String,
}

/// Split a node's effect list across `box-shadow` / `filter` /
/// `backdrop-filter`.
///
/// All three conversions are exact. CSS defines a `box-shadow` blur
/// radius as twice the Gaussian sigma, and the canvas backend computes
/// `sigma = blur * 0.5` from the same authored number, so the blur
/// value passes through unchanged. `filter: blur()` takes the sigma
/// directly, so a layer blur halves its authored radius — the same
/// `radius * 0.5` the painter applies.
pub fn effects(list: &[Effect]) -> EffectCss {
    let mut out = EffectCss::default();
    for effect in list {
        match effect {
            Effect::DropShadow(s) => {
                if !out.box_shadow.is_empty() {
                    out.box_shadow.push(',');
                }
                let _ = write!(
                    out.box_shadow,
                    "{}{}px {}px {}px {}",
                    if s.inner { "inset " } else { "" },
                    num(s.offset_x),
                    num(s.offset_y),
                    num(s.blur.max(0.0)),
                    color(s.color)
                );
            }
            Effect::Blur(b) if b.radius > 0.0 => {
                if !out.filter.is_empty() {
                    out.filter.push(' ');
                }
                let _ = write!(out.filter, "blur({}px)", num(b.radius * 0.5));
            }
            Effect::BackgroundBlur { radius } if *radius > 0.0 => {
                if !out.backdrop_filter.is_empty() {
                    out.backdrop_filter.push(' ');
                }
                let _ = write!(out.backdrop_filter, "blur({}px)", num(radius * 0.5));
            }
            Effect::Blur(_) | Effect::BackgroundBlur { .. } => {}
        }
    }
    out
}

/// CSS `mix-blend-mode` keyword for a scene blend mode, or `None` for
/// `Normal` (which needs no declaration).
///
/// Every scene mode has a CSS keyword, so nothing here forces a raster.
/// The drift is one of SCOPE, not of maths: Skia blends against the
/// accumulated canvas, while CSS blends against the nearest stacking
/// context's backdrop. Slides pin position with `transform` on rotated
/// nodes and `opacity` on composited ones, both of which open a
/// stacking context — so a blended node inside one of those composites
/// against its container rather than the whole slide.
pub fn blend_mode(mode: ImageBlendMode) -> Option<&'static str> {
    Some(match mode {
        ImageBlendMode::Normal => return None,
        ImageBlendMode::Darken => "darken",
        ImageBlendMode::Multiply => "multiply",
        ImageBlendMode::Screen => "screen",
        ImageBlendMode::Overlay => "overlay",
        ImageBlendMode::Lighten => "lighten",
        ImageBlendMode::Difference => "difference",
        ImageBlendMode::Hue => "hue",
        ImageBlendMode::Saturation => "saturation",
        ImageBlendMode::Color => "color",
        ImageBlendMode::Luminosity => "luminosity",
        ImageBlendMode::SoftLight => "soft-light",
        ImageBlendMode::ColorDodge => "color-dodge",
        ImageBlendMode::ColorBurn => "color-burn",
        ImageBlendMode::HardLight => "hard-light",
        ImageBlendMode::Exclusion => "exclusion",
    })
}

/// Append `prop:value;` to a style attribute under construction.
pub fn decl(style: &mut String, prop: &str, value: &str) {
    let _ = write!(style, "{prop}:{value};");
}

/// Append the four absolute-position declarations that pin a box to a
/// rect already expressed in board-local coordinates.
pub fn place(style: &mut String, rect: Rect) {
    decl(style, "left", &format!("{}px", num(rect.origin.x)));
    decl(style, "top", &format!("{}px", num(rect.origin.y)));
    decl(style, "width", &format!("{}px", num(rect.size.x)));
    decl(style, "height", &format!("{}px", num(rect.size.y)));
}
