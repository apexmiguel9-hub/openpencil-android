//! Fill / stroke / effect read-write helpers for `PenNode`.
//!
//! The canonical model stores rich `Vec<PenFill>` payloads; the
//! property panel mostly edits a primary colour/fill/stroke/effect
//! surface. These helpers preserve non-target fills and gradient/image
//! bodies instead of flattening the node into shell-core's old scalar
//! colour model.
//!
//! ### Module layout
//!
//! This file is the public spine: the [`ImageFillSummary`] type plus
//! the shared re-exports. The helpers themselves live in sibling
//! submodules (per the 800-line-per-file ceiling) and are glob
//! re-exported here, so every existing `fills::*` import path still
//! resolves:
//!
//! - [`access`] — fill / stroke / effect read accessors
//! - [`image`] — image-fill mode, tile scale and colour adjustments
//! - [`primary`] — primary-fill writers (opacity, hex, gradient stops)
//! - [`kinds`] — fill-type classification, defaults and conversion
//! - [`edits`] — indexed fill list edits, stroke hex, effect appenders

pub mod access;
pub mod edits;
pub mod image;
pub mod kinds;
pub mod primary;
#[cfg(test)]
mod tests;

pub use access::*;
pub use edits::*;
pub use image::*;
pub use kinds::*;
pub use primary::*;

use crate::editor_ui_state::{FillType, ImageAdjustmentField, ImageFillMode};
use crate::PenNodeExt;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::style::{
    GradientStop, ImageFillBody, LinearGradientBody, MeshGradientBody, MeshVertexStop, PenEffect,
    PenFill, PenStroke, RadialGradientBody, ShaderFillBody, ShadowBody, SolidFillBody,
    StrokeThickness,
};

/// Display/edit summary of the primary image fill.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageFillSummary {
    pub mode: ImageFillMode,
    pub has_image: bool,
    /// Current primary image URL. Native picked images are stored as
    /// inline `data:image/...;base64,...` URLs so the property popover
    /// can decode and preview them without another host-side fetch.
    pub image_url: Option<String>,
    /// Effective Figma TILE scale for an image fill. `Some(1.0)` is the
    /// backward-compatible default when `tileScale` is absent or invalid;
    /// standalone Image nodes report `None` because they do not support it.
    pub tile_scale: Option<f32>,
    /// Figma affine image transform, mapping normalized node coordinates to
    /// normalized image UV coordinates. Standalone Image nodes do not carry
    /// this fill-only crop payload.
    pub transform: Option<[f32; 6]>,
    /// Authored source dimensions used by crop initialization and TILE
    /// rendering. This remains independent from the decoded preview raster's
    /// possibly-downsampled dimensions.
    pub original_size: Option<[f32; 2]>,
    pub exposure: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub temperature: f32,
    pub tint: f32,
    pub highlights: f32,
    pub shadows: f32,
}

impl ImageFillSummary {
    pub fn adjustment(&self, field: ImageAdjustmentField) -> f32 {
        match field {
            ImageAdjustmentField::Exposure => self.exposure,
            ImageAdjustmentField::Contrast => self.contrast,
            ImageAdjustmentField::Saturation => self.saturation,
            ImageAdjustmentField::Temperature => self.temperature,
            ImageAdjustmentField::Tint => self.tint,
            ImageAdjustmentField::Highlights => self.highlights,
            ImageAdjustmentField::Shadows => self.shadows,
        }
    }

    pub fn has_adjustments(&self) -> bool {
        ImageAdjustmentField::ALL
            .iter()
            .any(|field| self.adjustment(*field).abs() > f32::EPSILON)
    }
}
