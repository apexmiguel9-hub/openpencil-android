//! Image-fill readers + writers: mode, tile scale and the colour
//! adjustment channels.

use super::*;

/// Summary of the node's primary image fill. `None` when the first
/// fill isn't `Image`.
pub fn first_image_fill_summary(node: &PenNode) -> Option<ImageFillSummary> {
    let PenFill::Image(body) = node_fills(node)?.first()? else {
        return None;
    };
    let trimmed_url = body.url.trim();
    let transform = body
        .transform
        .as_ref()
        .map(|m| [m.m00, m.m01, m.m02, m.m10, m.m11, m.m12]);
    let mode = match body.mode.as_ref() {
        // Older `.op` documents preserve Figma's wire representation:
        // interactively cropped images were serialized as STRETCH plus an
        // affine sampling transform. Expose the actual editing semantics.
        Some(jian_ops_schema::style::ImageFillMode::Stretch)
            if crate::image_crop::image_fill_body_is_crop(body) =>
        {
            ImageFillMode::Crop
        }
        mode => ImageFillMode::from_schema(mode),
    };
    Some(ImageFillSummary {
        mode,
        has_image: !trimmed_url.is_empty(),
        image_url: (!trimmed_url.is_empty()).then(|| body.url.to_string()),
        tile_scale: Some(effective_image_tile_scale(body.tile_scale)),
        transform,
        original_size: body.original_size.as_ref().and_then(|size| {
            (size.width.is_finite()
                && size.height.is_finite()
                && size.width > 0.0
                && size.height > 0.0)
                .then_some([size.width, size.height])
        }),
        exposure: body.exposure.unwrap_or(0.0),
        contrast: body.contrast.unwrap_or(0.0),
        saturation: body.saturation.unwrap_or(0.0),
        temperature: body.temperature.unwrap_or(0.0),
        tint: body.tint.unwrap_or(0.0),
        highlights: body.highlights.unwrap_or(0.0),
        shadows: body.shadows.unwrap_or(0.0),
    })
}

fn primary_image_fill_mut(node: &mut PenNode) -> Option<&mut ImageFillBody> {
    if node_fills(node).map(|f| f.is_empty()).unwrap_or(true) {
        return None;
    }
    let fills = node_fills_mut(node)?;
    match fills.first_mut()? {
        PenFill::Image(body) => Some(body),
        _ => None,
    }
}

/// Set the primary image fill's fit mode.
pub fn set_primary_image_fill_mode(node: &mut PenNode, mode: ImageFillMode) -> bool {
    let node_width = node.width_px().map(|value| value as f32);
    let node_height = node.height_px().map(|value| value as f32);
    let Some(body) = primary_image_fill_mut(node) else {
        return false;
    };
    let schema_mode = mode.to_schema();
    let mut changed = body.mode.as_ref() != Some(&schema_mode);
    body.mode = Some(schema_mode);
    if mode == ImageFillMode::Crop {
        if body.transform.is_none() {
            let transform = node_width.zip(node_height).and_then(|(width, height)| {
                crate::image_crop::centered_crop_transform(
                    width,
                    height,
                    body.original_size.as_ref(),
                )
            });
            if transform.is_some() {
                body.transform = transform;
                changed = true;
            }
        }
    } else if body.transform.take().is_some() {
        // Affine image transforms take precedence over every non-Tile mode in
        // both renderers. Clear a crop transform when leaving Crop so Fill/Fit
        // actually changes the rendered placement.
        changed = true;
    }
    changed
}

/// Lowest and highest tile scales accepted by the inspector. Keeping the
/// range finite and bounded prevents accidental near-zero tile explosions or
/// unusably large patterns while covering normal Figma-authored values.
pub const MIN_IMAGE_TILE_SCALE: f32 = 0.01;
pub const MAX_IMAGE_TILE_SCALE: f32 = 100.0;

fn validated_image_tile_scale(value: f32) -> Option<f32> {
    if !value.is_finite() || value <= 0.0 {
        return None;
    }
    Some(value.clamp(MIN_IMAGE_TILE_SCALE, MAX_IMAGE_TILE_SCALE))
}

fn effective_image_tile_scale(value: Option<f32>) -> f32 {
    value
        .filter(|scale| scale.is_finite() && *scale > 0.0)
        .unwrap_or(1.0)
}

/// Set the primary image fill's TILE scale. Invalid/non-positive values are
/// rejected without mutating the document. New `1.0` values use the omitted
/// old-wire default; an already-authored `Some(1.0)` remains untouched because
/// its effective value did not change. Standalone Image nodes are unsupported.
pub fn set_primary_image_tile_scale(node: &mut PenNode, value: f32) -> bool {
    let Some(body) = primary_image_fill_mut(node) else {
        return false;
    };
    let Some(value) = validated_image_tile_scale(value) else {
        return false;
    };
    if effective_image_tile_scale(body.tile_scale) == value {
        return false;
    }
    body.tile_scale = (value != 1.0).then_some(value);
    true
}

/// Set one primary image-fill adjustment, clamped to the TS slider
/// range `[-100, 100]`.
pub fn set_primary_image_adjustment(
    node: &mut PenNode,
    field: ImageAdjustmentField,
    value: f32,
) -> bool {
    let Some(body) = primary_image_fill_mut(node) else {
        return false;
    };
    let value = value.clamp(-100.0, 100.0);
    match field {
        ImageAdjustmentField::Exposure => body.exposure = Some(value),
        ImageAdjustmentField::Contrast => body.contrast = Some(value),
        ImageAdjustmentField::Saturation => body.saturation = Some(value),
        ImageAdjustmentField::Temperature => body.temperature = Some(value),
        ImageAdjustmentField::Tint => body.tint = Some(value),
        ImageAdjustmentField::Highlights => body.highlights = Some(value),
        ImageAdjustmentField::Shadows => body.shadows = Some(value),
    }
    true
}

/// Reset every primary image-fill adjustment to zero.
pub fn reset_primary_image_adjustments(node: &mut PenNode) -> bool {
    let Some(body) = primary_image_fill_mut(node) else {
        return false;
    };
    body.exposure = Some(0.0);
    body.contrast = Some(0.0);
    body.saturation = Some(0.0);
    body.temperature = Some(0.0);
    body.tint = Some(0.0);
    body.highlights = Some(0.0);
    body.shadows = Some(0.0);
    true
}
