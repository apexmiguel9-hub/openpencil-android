//! Style and layout fields inherited by component instances.

use crate::kiwi::FigValue;
use crate::tree::guid_to_string;

/// Layout fields consumed by the canonical container and sizing mappers.
const LAYOUT_KEYS: &[&str] = &[
    "stackMode",
    "stackSpacing",
    "stackPadding",
    "stackHorizontalPadding",
    "stackVerticalPadding",
    "stackPaddingRight",
    "stackPaddingBottom",
    "stackPrimaryAlignItems",
    "stackCounterAlignItems",
    "stackPrimarySizing",
    "stackCounterSizing",
    "stackChildPrimaryGrow",
    "stackChildAlignSelf",
    "frameMaskDisabled",
];

/// Visual fields consumed by the canonical base, fill, stroke, corner, and
/// effects mappers. Geometry and instance-owned fields are intentionally absent.
const VISUAL_KEYS: &[&str] = &[
    "fillPaints",
    "backgroundPaints",
    "strokePaints",
    "strokeWeight",
    "borderStrokeWeightsIndependent",
    "borderTopWeight",
    "borderRightWeight",
    "borderBottomWeight",
    "borderLeftWeight",
    "strokeAlign",
    "strokeJoin",
    "strokeCap",
    "dashPattern",
    "cornerRadius",
    "cornerSmoothing",
    "rectangleCornerRadiiIndependent",
    "rectangleTopLeftCornerRadius",
    "rectangleTopRightCornerRadius",
    "rectangleBottomLeftCornerRadius",
    "rectangleBottomRightCornerRadius",
    "effects",
    "opacity",
    "blendMode",
];

pub(super) fn is_inherited_style_key(key: &str) -> bool {
    LAYOUT_KEYS.contains(&key) || VISUAL_KEYS.contains(&key)
}

/// Find the override entry authored for the SYMBOL root itself.
///
/// Child overrides must not leak onto the synthetic instance frame, so the
/// path has to consist of exactly the root SYMBOL's GUID.
fn root_symbol_override<'a>(instance: &'a FigValue, symbol: &FigValue) -> Option<&'a FigValue> {
    let root_guid = symbol.get("guid").and_then(guid_to_string)?;
    instance
        .get("symbolData")?
        .get_array("symbolOverrides")?
        .iter()
        .find(|entry| {
            let Some(guids) = entry
                .get("guidPath")
                .and_then(|path| path.get_array("guids"))
            else {
                return false;
            };
            guids.len() == 1
                && guids
                    .first()
                    .and_then(guid_to_string)
                    .is_some_and(|guid| guid == root_guid)
        })
}

/// Merge safe SYMBOL props onto the synthetic instance frame.
///
/// Precedence is:
/// 1. fields authored directly on the INSTANCE;
/// 2. the `symbolOverrides` entry targeting the SYMBOL root;
/// 3. defaults from the SYMBOL.
///
/// Geometry and identity fields are deliberately excluded by the two key
/// allowlists above, so the INSTANCE always keeps its own size and transform.
pub(crate) fn merge_symbol_props(instance: &FigValue, symbol: &FigValue) -> FigValue {
    let mut merged = instance.clone();
    let root_override = root_symbol_override(instance, symbol);
    for key in LAYOUT_KEYS.iter().chain(VISUAL_KEYS) {
        if merged.get(key).is_none() {
            if let Some(value) = root_override
                .and_then(|entry| entry.get(key))
                .or_else(|| symbol.get(key))
            {
                merged.set(key, value.clone());
            }
        }
    }
    merged
}
