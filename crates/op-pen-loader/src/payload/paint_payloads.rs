//! Leaf paint payload structs shared by the adapter, the scene builder and
//! the serialized `DocPayload` wire format.
//!
//! Pure code motion out of the `payload.rs` spine (800-line ceiling). These
//! are plain resolved-value carriers — no behaviour lives here; `payload.rs`
//! re-exports every name so `crate::payload::StrokePayload` and friends keep
//! resolving.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct StrokePayload {
    /// The stroke's resolved solid paint, or `None` when the author declared
    /// a stroke with no resolvable colour at all (`"stroke": {"thickness": 1}`
    /// with no `fill` key, `"fill": []`, or a gradient with no usable stops).
    /// `None` is NOT the same as an explicitly transparent paint — that
    /// parses to `Some([_, _, _, 0.0])`. Consumers pick their own fallback:
    /// ordinary shapes keep painting the historical opaque black, while
    /// widget nodes drop the stroke so the widget-visual resolver falls back
    /// to its role defaults instead of a black track / border.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[f32; 4]>,
    pub width: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sides: Option<[f32; 4]>,
    /// Stroke placement: -1 inside, 0 center (default), +1 outside.
    #[serde(default, skip_serializing_if = "is_zero_i8")]
    pub align: i8,
}

fn is_zero_i8(v: &i8) -> bool {
    *v == 0
}

/// One styled text segment, flattened in document order. Sentinels
/// mirror the node-level fields: `0.0` font size / `0` weight / `None`
/// fill = inherit the node's value. `italic` / `underline` /
/// `strikethrough` are RESOLVED against the node level already (a
/// segment without an override inherits the node's flag).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextRunPayload {
    pub text: String,
    #[serde(default)]
    pub font_size: f32,
    #[serde(default)]
    pub font_weight: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<[f32; 4]>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub underline: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub strikethrough: bool,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ImageAdjustmentPayload {
    #[serde(default)]
    pub exposure: f32,
    #[serde(default)]
    pub contrast: f32,
    #[serde(default)]
    pub saturation: f32,
    #[serde(default)]
    pub temperature: f32,
    #[serde(default)]
    pub tint: f32,
    #[serde(default)]
    pub highlights: f32,
    #[serde(default)]
    pub shadows: f32,
}

/// One resolved gradient stop — offset 0.0..=1.0 + RGBA colour.
/// Mirrors `jian_ops_schema::GradientStop` but with the colour
/// hex pre-parsed into the same `[r,g,b,a]` array `NodePayload.fill`
/// uses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GradientStopPayload {
    pub offset: f32,
    pub color: [f32; 4],
}

/// Layout-resolved gradient body for `NodePayload.gradient`.
/// Pre-parsed (colour hex → RGBA, opacity baked into the variant)
/// so the scene builder + canvas painter never re-walk the
/// canonical schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GradientPayload {
    Linear {
        /// Gradient angle in degrees, canonical `.op` convention:
        /// 0° = bottom→top, 90° = left→right, 180° = top→bottom
        /// (matches CSS `to-top`). The renderer subtracts 90° before
        /// projecting endpoints, so storing as authored keeps the
        /// scene wire-format equal to the file's `angle`.
        angle_deg: f32,
        opacity: f32,
        stops: Vec<GradientStopPayload>,
    },
    Radial {
        /// Centre x as a 0.0..=1.0 fraction of bounds width.
        cx: f32,
        /// Centre y as a 0.0..=1.0 fraction of bounds height.
        cy: f32,
        /// Outer radius as a 0.0..=1.0 fraction of `max(w, h)` —
        /// matches the TS renderer, so the same `.op` file paints at
        /// the same radial size on native + web + export.
        radius: f32,
        opacity: f32,
        stops: Vec<GradientStopPayload>,
    },
    /// Uniform-grid mesh gradient (v1). `colors` is a row-major
    /// `rows`×`cols` lattice of pre-resolved RGBA values (length ==
    /// `rows * cols`); vertex `(r, c)` lives at `colors[r * cols + c]`.
    /// Opacity is carried separately and folded by the painter (parity
    /// with how the Linear / Radial variants thread `opacity`).
    Mesh {
        rows: u32,
        cols: u32,
        colors: Vec<[f32; 4]>,
        opacity: f32,
    },
}

/// One resolved SkSL shader uniform — name plus a concrete float vector
/// (length 1 = float, 2/3/4 = vec*). A `color` uniform is pre-expanded
/// into a 4-float premultiplied-RGBA `vec4` here so the scene builder +
/// painter never re-walk the canonical schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderUniformPayload {
    pub name: String,
    pub values: Vec<f32>,
}

/// Layout-resolved native SkSL shader body for `NodePayload.shader`.
/// `sksl` is the RAW (untrusted) source; uniforms are pre-resolved.
/// `fallback` is the `[r,g,b,a]` solid colour painted when a host can't
/// compile the program (first `color` uniform, else mid-gray) — kept
/// alongside `NodePayload.fill` so the degradation path always has a
/// visible colour.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderPayload {
    pub sksl: String,
    #[serde(default)]
    pub uniforms: Vec<ShaderUniformPayload>,
    pub opacity: f32,
    pub fallback: [f32; 4],
}

/// One path bezier anchor in absolute doc coords. `handle_in` /
/// `handle_out` are absolute control-point positions (already
/// resolved from the schema's anchor-relative deltas); `point_type`
/// is `0` corner / `1` mirrored / `2` independent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnchorPayload {
    pub x: f32,
    pub y: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handle_in: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handle_out: Option<[f32; 2]>,
    #[serde(default)]
    pub point_type: u8,
}
