//! Visible approximations for boolean nodes whose result geometry is absent.

use crate::common::{common_props, resolve_height, resolve_width, ConversionContext};
use crate::converters::{convert_children, convert_node};
use crate::kiwi::FigValue;
use crate::mappers::{map_figma_effects, map_figma_fills, map_figma_stroke};
use crate::mask::any_visible;
use crate::node_build::group_node;
use crate::tree::TreeNode;
use jian_ops_schema::node::container::ContainerProps;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::style::{PenFill, PenStroke};

const STROKE_STYLE_KEYS: [&str; 10] = [
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
];

/// Approximate an empty boolean result with its painted operands.
///
/// UNION can be represented reasonably by painting every operand. The exact
/// geometry of SUBTRACT, INTERSECT, and other operations is unavailable, so
/// they retain only the first operand as a visible base and emit a warning.
/// This is deliberately approximate, but it is strictly more useful than the
/// previous invisible-path fallback.
pub fn convert_empty_boolean_group(
    tree: &TreeNode,
    parent_stack_mode: Option<&str>,
    id: String,
    ctx: &mut ConversionContext<'_>,
) -> Option<PenNode> {
    if tree.figma.get_str("type") != Some("BOOLEAN_OPERATION") || tree.children.is_empty() {
        return None;
    }

    let operation = tree.figma.get_str("booleanOperation").unwrap_or("UNKNOWN");
    let mut synthetic = tree.clone();
    if operation != "UNION" {
        synthetic.children.truncate(1);
    }
    for child in &mut synthetic.children {
        // Component/frame operands are transparent wrappers, not the boolean
        // silhouette. Painting one creates a solid bounding box behind icon
        // artwork. Plain GROUP operands keep the historical empty-geometry
        // fallback because they may be the only available result silhouette.
        if operand_accepts_raw_result_paint(&child.figma) {
            inherit_result_paint(&tree.figma, &mut child.figma);
        }
    }

    let mut children = convert_children(&synthetic, ctx);
    if children.is_empty() {
        return None;
    }
    let fill = map_figma_fills(tree.figma.get_array("fillPaints"));
    let stroke = map_figma_stroke(&tree.figma);
    for child in &mut children {
        apply_result_paint_to_artwork(child, &fill, &stroke, true);
    }
    if operation != "UNION" {
        ctx.warnings.push(format!(
            "Boolean node \"{}\" approximated from first child ({operation}; empty geometry)",
            tree.figma.get_str("name").unwrap_or("")
        ));
    }

    let container = ContainerProps {
        width: Some(resolve_width(&tree.figma, parent_stack_mode, ctx)),
        height: Some(resolve_height(&tree.figma, parent_stack_mode, ctx)),
        effects: map_figma_effects(tree.figma.get_array("effects")),
        ..ContainerProps::default()
    };
    Some(group_node(
        common_props(&tree.figma, id),
        container,
        Some(children),
    ))
}

/// Rebuild a cached boolean result when an operand's component was swapped.
///
/// Figma stores baked boolean geometry on the parent. That geometry still
/// describes the original component after a nested INSTANCE_SWAP, so it must
/// not hide the newly selected component artwork. UNION operands can be
/// painted directly. A single operand is also exact for every operation;
/// multi-operand SUBTRACT/INTERSECT/EXCLUDE keep their baked result until a
/// real boolean evaluator is available.
pub fn convert_swapped_boolean_group(
    tree: &TreeNode,
    parent_stack_mode: Option<&str>,
    id: String,
    ctx: &mut ConversionContext<'_>,
) -> Option<PenNode> {
    if tree.figma.get_str("type") != Some("BOOLEAN_OPERATION")
        || tree.children.is_empty()
        || !has_real_component_swap(tree)
    {
        return None;
    }

    let operation = tree.figma.get_str("booleanOperation").unwrap_or("UNKNOWN");
    if operation != "UNION" && tree.children.len() > 1 {
        ctx.warnings.push(format!(
            "Boolean node \"{}\" kept cached geometry after component swap ({operation})",
            tree.figma.get_str("name").unwrap_or("")
        ));
        return None;
    }
    let fill = map_figma_fills(tree.figma.get_array("fillPaints"));
    let stroke = map_figma_stroke(&tree.figma);
    let child_stack_mode = tree.figma.get_str("stackMode").map(str::to_string);
    let mut children = Vec::new();
    for source_child in &tree.children {
        if source_child.figma.get_bool("visible") == Some(false)
            || source_child
                .figma
                .get_f64("opacity")
                .is_some_and(|opacity| opacity <= 0.0)
        {
            continue;
        }
        let Some(mut child) = convert_node(source_child, child_stack_mode.as_deref(), ctx) else {
            continue;
        };
        let paints_own_bounds = !expands_transparent_artwork_wrapper(source_child, ctx);
        apply_result_paint_to_artwork(&mut child, &fill, &stroke, paints_own_bounds);
        children.push(child);
    }
    if children.is_empty() {
        return None;
    }
    let container = ContainerProps {
        width: Some(resolve_width(&tree.figma, parent_stack_mode, ctx)),
        height: Some(resolve_height(&tree.figma, parent_stack_mode, ctx)),
        effects: map_figma_effects(tree.figma.get_array("effects")),
        ..ContainerProps::default()
    };
    Some(group_node(
        common_props(&tree.figma, id),
        container,
        Some(children),
    ))
}

fn has_real_component_swap(node: &TreeNode) -> bool {
    if node.figma.get_bool("visible") == Some(false) {
        return false;
    }
    let swaps_component = if node.figma.get_str("type") == Some("INSTANCE") {
        let override_guid = node
            .figma
            .get("overriddenSymbolID")
            .and_then(crate::tree::guid_to_string);
        let base_guid = node
            .figma
            .get("symbolData")
            .and_then(|data| data.get("symbolID"))
            .and_then(crate::tree::guid_to_string);
        let direct_swap = override_guid
            .as_ref()
            .is_some_and(|target| base_guid.as_ref() != Some(target));
        let nested_swap = node
            .figma
            .get("symbolData")
            .and_then(|data| data.get_array("symbolOverrides"))
            .is_some_and(|overrides| overrides.iter().any(entry_swaps_component));
        direct_swap || nested_swap || entry_has_component_assignment(&node.figma)
    } else {
        false
    };
    swaps_component || node.children.iter().any(has_real_component_swap)
}

fn entry_swaps_component(entry: &FigValue) -> bool {
    entry.get("overriddenSymbolID").is_some() || entry_has_component_assignment(entry)
}

fn entry_has_component_assignment(entry: &FigValue) -> bool {
    entry
        .get_array("componentPropAssignments")
        .is_some_and(|assignments| assignments.iter().any(assignment_targets_component))
}

fn assignment_targets_component(assignment: &FigValue) -> bool {
    assignment
        .get("value")
        .and_then(|value| value.get("guidValue"))
        .is_some()
        || assignment
            .get("varValue")
            .and_then(|value| value.get("value"))
            .and_then(|value| value.get("symbolIdValue"))
            .and_then(|value| value.get("guid"))
            .is_some()
}

/// A swapped icon component often uses an explicitly unpainted SYMBOL root as
/// a sizing/clipping wrapper around one geometry child. Kiwi can still cache a
/// visible fill on the INSTANCE override. That cache must not turn the wrapper
/// into artwork when a parent boolean result recolours the actual geometry.
///
/// An absent/empty root paint is transparent too — this is the common shape
/// of Figma icon components. Visible root paint keeps components with a
/// genuinely painted background in the boolean silhouette.
fn expands_transparent_artwork_wrapper(node: &TreeNode, ctx: &ConversionContext<'_>) -> bool {
    if node.figma.get_str("type") != Some("INSTANCE") {
        return false;
    }
    let override_guid = node
        .figma
        .get("overriddenSymbolID")
        .and_then(crate::tree::guid_to_string);
    let base_guid = node
        .figma
        .get("symbolData")
        .and_then(|data| data.get("symbolID"))
        .and_then(crate::tree::guid_to_string);
    let Some(target_guid) = override_guid.filter(|target| base_guid.as_ref() != Some(target))
    else {
        return false;
    };
    let Some(symbol) = ctx.symbol_tree.get(&target_guid).copied() else {
        return false;
    };

    if any_visible(symbol.figma.get_array("fillPaints"))
        || any_visible(symbol.figma.get_array("backgroundPaints"))
        || any_visible(symbol.figma.get_array("strokePaints"))
    {
        return false;
    }

    let mut visible_children = symbol.children.iter().filter(|child| {
        child.figma.get_bool("visible") != Some(false)
            && child.figma.get_f64("opacity").unwrap_or(1.0) > 0.0
    });
    let Some(artwork) = visible_children.next() else {
        return false;
    };
    visible_children.next().is_none()
        && matches!(
            artwork.figma.get_str("type"),
            Some(
                "VECTOR"
                    | "BOOLEAN_OPERATION"
                    | "RECTANGLE"
                    | "ROUNDED_RECTANGLE"
                    | "ELLIPSE"
                    | "LINE"
                    | "STAR"
                    | "REGULAR_POLYGON"
            )
        )
}

fn apply_result_paint_to_artwork(
    node: &mut PenNode,
    fill: &Option<Vec<PenFill>>,
    stroke: &Option<PenStroke>,
    paint_self: bool,
) {
    match node {
        PenNode::Frame(frame) => {
            if paint_self {
                apply_shape_result_paint(
                    &mut frame.container.fill,
                    &mut frame.container.stroke,
                    fill,
                    stroke,
                );
            } else {
                frame.container.fill = None;
                frame.container.stroke = None;
            }
            if let Some(children) = &mut frame.children {
                for child in children {
                    apply_result_paint_to_artwork(child, fill, stroke, true);
                }
            }
        }
        PenNode::Group(group) => {
            if paint_self {
                // Empty boolean GROUP operands have no canonical geometry or
                // authored container paint. Their bounds are the historical
                // fallback silhouette, so apply the parent result directly.
                group.container.fill = fill.clone();
                group.container.stroke = stroke.clone();
            } else {
                group.container.fill = None;
                group.container.stroke = None;
            }
            if let Some(children) = &mut group.children {
                for child in children {
                    apply_result_paint_to_artwork(child, fill, stroke, true);
                }
            }
        }
        PenNode::Ref(reference) => {
            if let Some(children) = &mut reference.children {
                for child in children {
                    apply_result_paint_to_artwork(child, fill, stroke, true);
                }
            }
        }
        PenNode::Tabs(tabs) => {
            apply_shape_result_paint(&mut tabs.fill, &mut tabs.stroke, fill, stroke);
            if let Some(children) = &mut tabs.children {
                for child in children {
                    apply_result_paint_to_artwork(child, fill, stroke, true);
                }
            }
        }
        PenNode::Rectangle(rectangle) => {
            apply_shape_result_paint(
                &mut rectangle.container.fill,
                &mut rectangle.container.stroke,
                fill,
                stroke,
            );
            if let Some(children) = &mut rectangle.children {
                for child in children {
                    apply_result_paint_to_artwork(child, fill, stroke, true);
                }
            }
        }
        PenNode::Ellipse(ellipse) => {
            apply_shape_result_paint(&mut ellipse.fill, &mut ellipse.stroke, fill, stroke);
        }
        PenNode::Polygon(polygon) => {
            apply_shape_result_paint(&mut polygon.fill, &mut polygon.stroke, fill, stroke);
        }
        PenNode::Path(path) => {
            apply_shape_result_paint(&mut path.fill, &mut path.stroke, fill, stroke);
        }
        PenNode::Text(text) => {
            if text.fill.is_some() {
                text.fill = fill
                    .clone()
                    .or_else(|| stroke.as_ref().and_then(|stroke| stroke.fill.clone()));
            }
        }
        PenNode::Line(line) => {
            let mut no_fill = None;
            apply_shape_result_paint(&mut no_fill, &mut line.stroke, fill, stroke);
        }
        PenNode::IconFont(icon) => {
            apply_shape_result_paint(&mut icon.fill, &mut icon.stroke, fill, stroke);
        }
        _ => {}
    }
}

/// Paint a reconstructed boolean silhouette without turning transparent
/// helper layers into artwork. When the source uses strokes to describe an
/// icon but the boolean result is fill-only, keep the source stroke geometry
/// and recolour that stroke from the result fill instead of erasing it.
fn apply_shape_result_paint(
    node_fill: &mut Option<Vec<PenFill>>,
    node_stroke: &mut Option<PenStroke>,
    result_fill: &Option<Vec<PenFill>>,
    result_stroke: &Option<PenStroke>,
) {
    let had_fill = node_fill.is_some();
    let authored_stroke = node_stroke.clone();
    if !had_fill && authored_stroke.is_none() {
        return;
    }

    match (result_fill, result_stroke) {
        (Some(fill), Some(stroke)) => {
            *node_fill = Some(fill.clone());
            *node_stroke = Some(stroke.clone());
        }
        (Some(fill), None) if had_fill => {
            *node_fill = Some(fill.clone());
            *node_stroke = None;
        }
        (Some(fill), None) => {
            let mut stroke = authored_stroke.expect("painted stroke checked above");
            stroke.fill = Some(fill.clone());
            *node_fill = None;
            *node_stroke = Some(stroke);
        }
        (None, Some(stroke)) => {
            *node_fill = None;
            *node_stroke = Some(stroke.clone());
        }
        (None, None) => {
            *node_fill = None;
            *node_stroke = None;
        }
    }
}

fn inherit_result_paint(parent: &FigValue, child: &mut FigValue) {
    for key in ["fillPaints", "strokePaints"] {
        let paints = parent.get_array(key).unwrap_or_default().to_vec();
        child.set(key, FigValue::Array(paints));
    }
    child.set("backgroundPaints", FigValue::Array(Vec::new()));
    for key in STROKE_STYLE_KEYS {
        if let Some(value) = parent.get(key) {
            child.set(key, value.clone());
        }
    }
}

fn operand_accepts_raw_result_paint(figma: &FigValue) -> bool {
    !matches!(
        figma.get_str("type"),
        Some("FRAME" | "SECTION" | "SYMBOL" | "INSTANCE" | "COMPONENT_SET")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(pairs: Vec<(&str, FigValue)>) -> FigValue {
        FigValue::Object(
            pairs
                .into_iter()
                .map(|(key, value)| (key.into(), value))
                .collect(),
        )
    }

    fn guid(session_id: u32, local_id: u32) -> FigValue {
        obj(vec![
            ("sessionID", FigValue::Uint(session_id)),
            ("localID", FigValue::Uint(local_id)),
        ])
    }

    fn instance(fields: Vec<(&str, FigValue)>) -> TreeNode {
        let mut pairs = vec![("type", FigValue::Str("INSTANCE".into()))];
        pairs.extend(fields);
        TreeNode {
            figma: obj(pairs),
            children: Vec::new(),
        }
    }

    #[test]
    fn component_swap_detection_accepts_legacy_and_nested_encodings() {
        let legacy = instance(vec![("overriddenSymbolID", guid(2, 20))]);
        assert!(has_real_component_swap(&legacy));

        let nested = instance(vec![(
            "symbolData",
            obj(vec![
                ("symbolID", guid(1, 10)),
                (
                    "symbolOverrides",
                    FigValue::Array(vec![obj(vec![("overriddenSymbolID", guid(2, 20))])]),
                ),
            ]),
        )]);
        assert!(has_real_component_swap(&nested));

        let modern = instance(vec![(
            "componentPropAssignments",
            FigValue::Array(vec![obj(vec![(
                "value",
                obj(vec![("guidValue", guid(3, 30))]),
            )])]),
        )]);
        assert!(has_real_component_swap(&modern));

        let unchanged = instance(vec![
            ("overriddenSymbolID", guid(1, 10)),
            ("symbolData", obj(vec![("symbolID", guid(1, 10))])),
        ]);
        assert!(!has_real_component_swap(&unchanged));
    }
}
