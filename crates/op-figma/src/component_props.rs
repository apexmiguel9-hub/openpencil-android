//! Normalize Figma component-property assignments into legacy symbol overrides.
//!
//! Modern `.fig` files store instance swaps as a value assignment keyed by a
//! component-property definition id. The component master separately marks the
//! nested INSTANCE whose `overriddenSymbolID` that definition controls. The
//! rest of the importer already understands explicit symbol overrides, so this
//! pass joins those two records before tree construction.

use crate::kiwi::FigValue;
use crate::tree::guid_to_string;
use std::collections::HashMap;

fn assignment_target(assignment: &FigValue) -> Option<FigValue> {
    assignment
        .get("value")
        .and_then(|value| value.get("guidValue"))
        .cloned()
        .or_else(|| {
            assignment
                .get("varValue")?
                .get("value")?
                .get("symbolIdValue")?
                .get("guid")
                .cloned()
        })
}

fn visit_references(value: &FigValue, visit: &mut impl FnMut(&FigValue)) {
    match value {
        FigValue::Array(values) => values
            .iter()
            .for_each(|value| visit_references(value, visit)),
        FigValue::Object(pairs) => {
            if value.get("defID").is_some() {
                visit(value);
            } else {
                pairs
                    .iter()
                    .for_each(|(_, value)| visit_references(value, visit));
            }
        }
        _ => {}
    }
}

fn component_swap_routes(nodes: &[FigValue]) -> HashMap<String, FigValue> {
    let mut routes = HashMap::new();
    for node in nodes {
        let Some(owner_guid) = node.get("guid").cloned() else {
            continue;
        };
        for key in ["componentPropRefs", "componentPropertyReferences"] {
            let Some(references) = node.get(key) else {
                continue;
            };
            visit_references(references, &mut |reference| {
                let field = reference
                    .get_str("componentPropNodeField")
                    .or_else(|| reference.get_str("type"));
                if !matches!(field, Some("OVERRIDDEN_SYMBOL_ID" | "INSTANCE_SWAP")) {
                    return;
                }
                if let Some(def_id) = reference.get("defID").and_then(guid_to_string) {
                    routes.entry(def_id).or_insert_with(|| owner_guid.clone());
                }
            });
        }
    }
    routes
}

fn guid_path(value: &FigValue) -> Option<Vec<FigValue>> {
    value
        .get("guidPath")?
        .get_array("guids")
        .map(<[FigValue]>::to_vec)
}

fn path_value(guids: Vec<FigValue>) -> FigValue {
    FigValue::Object(vec![("guids".into(), FigValue::Array(guids))])
}

fn same_path(entry: &FigValue, path: &[FigValue]) -> bool {
    let Some(existing) = guid_path(entry) else {
        return false;
    };
    existing.len() == path.len()
        && existing.iter().zip(path).all(|(left, right)| {
            left.get_f64("sessionID") == right.get_f64("sessionID")
                && left.get_f64("localID") == right.get_f64("localID")
        })
}

fn assignment_entries(holder: &FigValue) -> Vec<FigValue> {
    holder
        .get_array("componentPropAssignments")
        .or_else(|| {
            holder
                .get("symbolData")
                .and_then(|data| data.get_array("componentPropAssignments"))
        })
        .map(<[FigValue]>::to_vec)
        .unwrap_or_default()
}

fn resolved_assignments(
    holder: &FigValue,
    routes: &HashMap<String, FigValue>,
) -> Vec<(FigValue, FigValue)> {
    assignment_entries(holder)
        .iter()
        .filter_map(|assignment| {
            let def_id = assignment.get("defID").and_then(guid_to_string)?;
            Some((routes.get(&def_id)?.clone(), assignment_target(assignment)?))
        })
        .collect()
}

fn upsert_swap_override(overrides: &mut Vec<FigValue>, path: Vec<FigValue>, target: FigValue) {
    if let Some(existing) = overrides.iter_mut().find(|entry| same_path(entry, &path)) {
        // Legacy direct overrides are authoritative when both encodings exist.
        if existing.get("overriddenSymbolID").is_none() {
            existing.set("overriddenSymbolID", target);
        }
        return;
    }
    overrides.push(FigValue::Object(vec![
        ("guidPath".into(), path_value(path)),
        ("overriddenSymbolID".into(), target),
    ]));
}

fn normalize_node(node: &mut FigValue, routes: &HashMap<String, FigValue>) {
    let direct = resolved_assignments(node, routes);
    let node_guid = node.get("guid").and_then(guid_to_string);
    let mut additions = Vec::new();
    for (owner, target) in direct {
        if node_guid.as_deref()
            == owner
                .get("sessionID")
                .and_then(|_| guid_to_string(&owner))
                .as_deref()
        {
            if node.get("overriddenSymbolID").is_none() {
                node.set("overriddenSymbolID", target);
            }
        } else {
            additions.push((vec![owner], target));
        }
    }

    let Some(symbol_data) = node.get_mut("symbolData") else {
        return;
    };
    let mut overrides = symbol_data
        .get_array("symbolOverrides")
        .map(<[FigValue]>::to_vec)
        .unwrap_or_default();
    let authored = overrides.clone();
    for entry in &authored {
        let prefix = guid_path(entry).unwrap_or_default();
        for (owner, target) in resolved_assignments(entry, routes) {
            let mut path = prefix.clone();
            path.push(owner);
            additions.push((path, target));
        }
    }
    for (path, target) in additions {
        upsert_swap_override(&mut overrides, path, target);
    }
    symbol_data.set("symbolOverrides", FigValue::Array(overrides));
}

pub(crate) fn resolve_component_property_swaps(nodes: &mut [FigValue]) {
    let routes = component_swap_routes(nodes);
    if routes.is_empty() {
        return;
    }
    nodes
        .iter_mut()
        .for_each(|node| normalize_node(node, &routes));
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

    fn reference(owner: (u32, u32), def: (u32, u32)) -> FigValue {
        obj(vec![
            ("guid", guid(owner.0, owner.1)),
            (
                "componentPropRefs",
                FigValue::Array(vec![obj(vec![
                    ("defID", guid(def.0, def.1)),
                    (
                        "componentPropNodeField",
                        FigValue::Str("OVERRIDDEN_SYMBOL_ID".into()),
                    ),
                ])]),
            ),
        ])
    }

    #[test]
    fn turns_nested_component_assignment_into_explicit_swap_override() {
        let mut nodes = vec![
            reference((18, 15), (90, 1)),
            obj(vec![
                ("guid", guid(3, 30)),
                (
                    "symbolData",
                    obj(vec![(
                        "symbolOverrides",
                        FigValue::Array(vec![obj(vec![
                            ("guidPath", path_value(vec![guid(7, 70)])),
                            (
                                "componentPropAssignments",
                                FigValue::Array(vec![obj(vec![
                                    ("defID", guid(90, 1)),
                                    ("value", obj(vec![("guidValue", guid(44, 440))])),
                                ])]),
                            ),
                        ])]),
                    )]),
                ),
            ]),
        ];

        resolve_component_property_swaps(&mut nodes);

        let overrides = nodes[1]
            .get("symbolData")
            .and_then(|data| data.get_array("symbolOverrides"))
            .expect("normalized overrides");
        let swap = overrides
            .iter()
            .find(|entry| same_path(entry, &[guid(7, 70), guid(18, 15)]))
            .expect("nested swap route");
        assert_eq!(
            swap.get("overriddenSymbolID").and_then(guid_to_string),
            Some("44:440".into())
        );
    }

    #[test]
    fn reads_variable_symbol_value_and_preserves_direct_legacy_swap() {
        let mut nodes = vec![
            reference((18, 15), (90, 1)),
            obj(vec![
                ("guid", guid(18, 15)),
                ("overriddenSymbolID", guid(1, 10)),
                (
                    "componentPropAssignments",
                    FigValue::Array(vec![obj(vec![
                        ("defID", guid(90, 1)),
                        (
                            "varValue",
                            obj(vec![(
                                "value",
                                obj(vec![("symbolIdValue", obj(vec![("guid", guid(2, 20))]))]),
                            )]),
                        ),
                    ])]),
                ),
            ]),
        ];

        resolve_component_property_swaps(&mut nodes);

        assert_eq!(
            nodes[1].get("overriddenSymbolID").and_then(guid_to_string),
            Some("1:10".into())
        );
    }
}
