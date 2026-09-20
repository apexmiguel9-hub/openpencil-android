//! Conservative Figma sibling-mask decoding.

use crate::kiwi::FigValue;
use jian_ops_schema::node::MaskType;

/// Decode the semantic operation for any Figma sibling-mask node. Kiwi omits
/// enum fields carrying their default value; Figma's documented default is
/// ALPHA, so an absent `maskType` on a marked node is not "unknown".
pub(crate) fn figma_mask_type(figma: &FigValue) -> Option<MaskType> {
    figma
        .get_f64("mask")
        .is_some_and(|value| value != 0.0)
        .then(|| match figma.get_str("maskType") {
            Some(kind) if kind.eq_ignore_ascii_case("VECTOR") => MaskType::Vector,
            Some(kind) if kind.eq_ignore_ascii_case("LUMINANCE") => MaskType::Luminance,
            _ => MaskType::Alpha,
        })
}

pub(crate) fn any_visible(paints: Option<&[FigValue]>) -> bool {
    paints
        .map(|paints| {
            paints
                .iter()
                .any(|paint| paint.get_bool("visible") != Some(false))
        })
        .unwrap_or(false)
}

/// Figma stores the sibling-mask marker as a numeric Kiwi field (`1` for
/// enabled). This first implementation deliberately accepts only masks whose
/// alpha is uniformly opaque: a solid, fully opaque fill on a fully opaque
/// node. In that case alpha masking is exactly equivalent to a geometry clip.
/// Gradient/image alpha, effects, strokes, and luminance masks require the
/// offscreen `DstIn` path and must not be claimed by this clip fast path.
pub(crate) fn figma_path_mask(figma: &FigValue) -> Option<bool> {
    if figma_mask_type(figma) != Some(MaskType::Alpha)
        || figma.get_f64("opacity").unwrap_or(1.0) < 0.999
        || any_visible(figma.get_array("strokePaints"))
        || figma
            .get_array("effects")
            .unwrap_or_default()
            .iter()
            .any(|effect| effect.get_bool("visible") != Some(false))
    {
        return None;
    }

    figma
        .get_array("fillPaints")
        .unwrap_or_default()
        .iter()
        .filter(|paint| paint.get_bool("visible") != Some(false))
        .any(|paint| {
            paint.get_str("type") == Some("SOLID")
                && paint.get_f64("opacity").unwrap_or(1.0) >= 0.999
                && paint
                    .get("color")
                    .and_then(|color| color.get_f64("a"))
                    .unwrap_or(1.0)
                    >= 0.999
        })
        .then_some(true)
}
