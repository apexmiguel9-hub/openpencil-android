//! Subtree cloning + per-node application of resolved override /
//! derived data, plus the virtual-GUID walk helpers shared by the
//! Strategy-2 resolution and the foreign-session anchoring.

use super::style::is_inherited_style_key;
use super::{guid_path_key, OVERRIDE_SKIP_KEYS};
use crate::common::{round2, scale_node_fields};
use crate::figma_types::FigVec2;
use crate::kiwi::FigValue;
use crate::tree::{guid_to_string, TreeNode};
use std::collections::{HashMap, HashSet};

/// Local-id getter — falls back to 0 when `guid.localID` is absent
/// (matches TS `a.figma.guid?.localID ?? 0`).
pub(super) fn local_id(node: &TreeNode) -> u32 {
    node.figma
        .get("guid")
        .and_then(|g| g.get_f64("localID"))
        .map(|n| n as u32)
        .unwrap_or(0)
}

/// Pre-order DFS over a TreeNode (children sorted ascending by
/// localID). The starting node is included as the first entry.
pub(super) fn flatten_dfs<'a>(node: &'a TreeNode, out: &mut Vec<&'a TreeNode>) {
    out.push(node);
    let mut sorted: Vec<&TreeNode> = node.children.iter().collect();
    sorted.sort_by_key(|n| local_id(n));
    for c in sorted {
        flatten_dfs(c, out);
    }
}

/// Walk a subtree in pre-order DFS, recording the virtual GUID
/// `sessionID:firstLocalID + idx` → actual GUID for each node. Mirrors
/// the TS `walkFull` / `walkRoot` helpers.
pub(super) fn walk_virtual(
    node: &TreeNode,
    session_id: u32,
    first_local_id: u32,
    idx: &mut u32,
    out: &mut HashMap<String, String>,
) {
    if let Some(g) = node.figma.get("guid").and_then(guid_to_string) {
        out.insert(format!("{}:{}", session_id, first_local_id + *idx), g);
    }
    *idx += 1;
    let mut sorted: Vec<&TreeNode> = node.children.iter().collect();
    sorted.sort_by_key(|n| local_id(n));
    for c in sorted {
        walk_virtual(c, session_id, first_local_id, idx, out);
    }
}

/// Read `(sessionID, firstLocalID)` from the first single-segment
/// derived entry — Strategy-2's virtual-GUID base. None when either
/// field is missing.
pub(super) fn virtual_guid_base(len1_derived: &[&FigValue]) -> Option<(u32, u32)> {
    let first = len1_derived.first()?;
    let first_guid = first
        .get("guidPath")
        .and_then(|p| p.get_array("guids"))
        .and_then(|g| g.first())?;
    let sid = first_guid.get_f64("sessionID")? as u32;
    let lid = first_guid.get_f64("localID")? as u32;
    Some((sid, lid))
}

/// Build a copy of `entry` with the first `guidPath` segment dropped.
pub(super) fn strip_first_guid(entry: &FigValue) -> Option<FigValue> {
    let guids = entry.get("guidPath")?.get_array("guids")?;
    if guids.len() < 2 {
        return None;
    }
    let mut copy = entry.clone();
    let rest: Vec<FigValue> = guids[1..].to_vec();
    let mut path = FigValue::Object(Vec::new());
    path.set("guids", FigValue::Array(rest));
    copy.set("guidPath", path);
    Some(copy)
}

/// Recursively clone the subtree, applying derived data + overrides to
/// each node keyed by its guid.
/// Merge forwarded entries into a node's authored list. Callers choose
/// whether the more-specific forwarded value wins same-field collisions.
/// Authored-only fields survive, new pks append, and same-pk entries remain
/// deduplicated for Strategy 1.
fn merge_entry_lists(
    existing: Vec<FigValue>,
    forwarded: &[FigValue],
    forwarded_wins: bool,
) -> Vec<FigValue> {
    let mut out = existing;
    for f in forwarded {
        let fk = guid_path_key(f);
        let slot = fk.as_ref().and_then(|k| {
            out.iter_mut()
                .find(|e| guid_path_key(e).as_ref() == Some(k))
        });
        match slot {
            Some(e) => {
                if let (FigValue::Object(epairs), FigValue::Object(fpairs)) = (&mut *e, f) {
                    for (k, v) in fpairs {
                        if k.as_ref() == "guidPath" {
                            continue;
                        }
                        if let Some(i) = epairs.iter().position(|(ek, _)| ek == k) {
                            if forwarded_wins {
                                epairs[i].1 = v.clone();
                            }
                        } else {
                            epairs.push((k.clone(), v.clone()));
                        }
                    }
                }
            }
            None => out.push(f.clone()),
        }
    }
    out
}

/// Newer Figma component properties carry a text override on the
/// nested INSTANCE rather than on its TEXT leaf. Recover one
/// unambiguous TEXT_DATA assignment so it can be forwarded to that
/// instance's sole text override entry.
fn component_text_assignment(entry: Option<&FigValue>) -> Option<FigValue> {
    let assignments = entry?.get_array("componentPropAssignments")?;
    let mut found = None;
    for assignment in assignments {
        let text = assignment
            .get("value")
            .and_then(|value| value.get("textValue"))
            .or_else(|| {
                assignment
                    .get("varValue")
                    .and_then(|value| value.get("value"))
                    .and_then(|value| value.get("textDataValue"))
            });
        let Some(text) = text.filter(|text| text.get_str("characters").is_some()) else {
            continue;
        };
        if found.is_some() {
            return None;
        }
        found = Some(text.clone());
    }
    found
}

fn apply_component_text_assignment(entries: &mut [FigValue], text_data: FigValue) {
    let candidates: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter_map(|(i, entry)| {
            (entry.get("textData").is_some() || entry.get("styleIdForText").is_some()).then_some(i)
        })
        .collect();
    if candidates.len() == 1 {
        entries[candidates[0]].set("textData", text_data);
    }
}

/// A component swap starts from the resolved fields of the old component.
/// Drop inherited layout and visual values that the swap override did not
/// explicitly author so the swapped-in SYMBOL can provide its own defaults.
fn clear_stale_swap_props(figma: &mut FigValue, override_entry: &FigValue) {
    let Some(target_guid) = override_entry
        .get("overriddenSymbolID")
        .and_then(guid_to_string)
    else {
        return;
    };
    let base_guid = figma
        .get("symbolData")
        .and_then(|data| data.get("symbolID"))
        .and_then(guid_to_string);
    if base_guid.as_ref() == Some(&target_guid) {
        return;
    }
    let FigValue::Object(pairs) = figma else {
        return;
    };
    pairs.retain(|(key, _)| !is_inherited_style_key(key) || override_entry.get(key).is_some());
}

pub(super) fn apply_to_node(
    node: TreeNode,
    node_override: &HashMap<String, FigValue>,
    node_derived: &HashMap<String, FigValue>,
    nested_override: &HashMap<String, Vec<FigValue>>,
    nested_derived: &HashMap<String, Vec<FigValue>>,
    derived_branch: &HashSet<String>,
    scale: (f64, f64),
) -> TreeNode {
    let key = node
        .figma
        .get("guid")
        .and_then(guid_to_string)
        .unwrap_or_default();
    let d = node_derived.get(&key);
    let ov = node_override.get(&key);
    let nested_ov = nested_override.get(&key);
    let nested_d = nested_derived.get(&key);
    let TreeNode {
        mut figma,
        children,
    } = node;

    // Instance rescale, branch by branch. Figma records resolved
    // geometry ("derived") for the instance children it re-laid out
    // and omits it entirely for branches that just came along for the
    // ride. A branch that carries derived data anywhere inside it is
    // already in instance space and must be left alone; only a branch
    // with no derived data at all is still component-space and needs
    // the instance/symbol ratio (that is the icon symbol whose artwork
    // sat at (60,60) inside a 40x40 box). Scaling a derived branch
    // instead pulled hidden price rows into view under text that kept
    // its own resolved size.
    let scales_here = !derived_branch.contains(&key);
    let authored_size = figma.get("size").and_then(FigVec2::from_value);
    if scales_here {
        scale_node_fields(&mut figma, scale.0, scale.1);
    }
    // A node Figma resized itself re-bases its children on its own
    // ratio. Inside an instance-space branch the ratio is spent, so
    // children stay put — scaling them anyway left a 102 px row
    // holding 6 px labels.
    let child_scale = match (
        authored_size,
        d.and_then(|d| d.get("size")).and_then(FigVec2::from_value),
    ) {
        (Some(a), Some(dv)) if a.x.abs() > 0.001 && a.y.abs() > 0.001 => (dv.x / a.x, dv.y / a.y),
        _ if !scales_here => (1.0, 1.0),
        _ => scale,
    };

    if d.is_none() && ov.is_none() && nested_ov.is_none() && nested_d.is_none() {
        return TreeNode {
            figma,
            children: children
                .into_iter()
                .map(|child| {
                    apply_to_node(
                        child,
                        node_override,
                        node_derived,
                        nested_override,
                        nested_derived,
                        derived_branch,
                        child_scale,
                    )
                })
                .collect(),
        };
    }

    // Derived data — scale stroke weight before overwriting size.
    if let Some(d) = d {
        if let (Some(dsize), Some(nsize)) = (
            d.get("size").and_then(FigVec2::from_value),
            figma.get("size").and_then(FigVec2::from_value),
        ) {
            if let Some(sw) = figma.get_f64("strokeWeight") {
                if nsize.x != 0.0 && nsize.y != 0.0 {
                    let scale = (dsize.x / nsize.x).min(dsize.y / nsize.y);
                    if scale < 0.99 {
                        figma.set("strokeWeight", FigValue::Float(round2(sw * scale) as f32));
                    }
                }
            }
        }
        if let Some(size) = d.get("size") {
            figma.set("size", size.clone());
        }
        if let Some(t) = d.get("transform") {
            figma.set("transform", t.clone());
        }
        if let Some(fs) = d.get("fontSize") {
            figma.set("fontSize", fs.clone());
        }
        if let Some(dtd) = d.get("derivedTextData") {
            if dtd.get("characters").is_some() {
                figma.set("textData", dtd.clone());
            }
        }
    }

    // Override props — copy every non-blacklisted key. Explicit
    // `Null` is preserved (TS `if (value !== undefined)`: only
    // `undefined` is skipped, `null` is copied as an intentional
    // reset).
    if let Some(override_entry) = ov {
        clear_stale_swap_props(&mut figma, override_entry);
    }
    if let Some(FigValue::Object(pairs)) = ov {
        for (k, v) in pairs {
            if !OVERRIDE_SKIP_KEYS.contains(&k.as_ref()) {
                figma.set(k, v.clone());
            }
        }
    }

    // Forward nested entries into nested INSTANCE nodes. Forwarded
    // entries MERGE with the node's own authored lists — replacing
    // them would strip a nested icon's scale / fill targets, and
    // appending duplicates would inflate the entry count into a false
    // Strategy-1 (index-mapping) match downstream. Per pk, forwarded
    // override fields win while authored-only fields survive.
    let is_instance =
        figma.get_str("type") == Some("INSTANCE") || figma.get("symbolData").is_some();
    if is_instance {
        let component_text = component_text_assignment(ov);
        if nested_ov.is_some() || component_text.is_some() {
            let existing: Vec<FigValue> = figma
                .get("symbolData")
                .and_then(|s| s.get_array("symbolOverrides"))
                .map(|a| a.to_vec())
                .unwrap_or_default();
            let mut merged = merge_entry_lists(
                existing,
                nested_ov.map(Vec::as_slice).unwrap_or_default(),
                true,
            );
            if let Some(text_data) = component_text {
                apply_component_text_assignment(&mut merged, text_data);
            }
            let mut symbol_data = figma
                .get("symbolData")
                .cloned()
                .unwrap_or(FigValue::Object(Vec::new()));
            symbol_data.set("symbolOverrides", FigValue::Array(merged));
            figma.set("symbolData", symbol_data);
        }
        if let Some(nested) = nested_d {
            let existing: Vec<FigValue> = figma
                .get_array("derivedSymbolData")
                .map(|a| a.to_vec())
                .unwrap_or_default();
            let merged = merge_entry_lists(existing, nested, true);
            figma.set("derivedSymbolData", FigValue::Array(merged));
        }
    }

    TreeNode {
        figma,
        children: children
            .into_iter()
            .map(|child| {
                apply_to_node(
                    child,
                    node_override,
                    node_derived,
                    nested_override,
                    nested_derived,
                    derived_branch,
                    child_scale,
                )
            })
            .collect(),
    }
}

/// Mark geometry-derived nodes and their ancestors so the outer instance
/// ratio is not applied a second time to an already-resolved branch.
pub(super) fn mark_derived_branches(
    node: &TreeNode,
    node_derived: &HashMap<String, FigValue>,
    out: &mut HashSet<String>,
) -> bool {
    let key = node.figma.get("guid").and_then(guid_to_string);
    let mut has = key
        .as_ref()
        .and_then(|key| node_derived.get(key.as_str()))
        .is_some_and(|derived| derived.get("size").is_some() || derived.get("transform").is_some());
    for child in &node.children {
        has |= mark_derived_branches(child, node_derived, out);
    }
    if has {
        if let Some(key) = key {
            out.insert(key);
        }
    }
    has
}

/// Strip the resolved head from multi-segment entries and group them by the
/// nested instance that will receive the remaining path.
pub(super) fn collect_nested_entries(
    order: &[String],
    entries: &HashMap<String, &FigValue>,
    pk_to_node_guid: &HashMap<String, String>,
) -> HashMap<String, Vec<FigValue>> {
    let mut nested = HashMap::new();
    for pk in order {
        if !pk.contains('/') {
            continue;
        }
        let head = pk.split('/').next().unwrap_or("");
        let instance_guid = pk_to_node_guid
            .get(head)
            .cloned()
            .unwrap_or_else(|| head.to_string());
        if let Some(entry) = entries.get(pk) {
            if let Some(rest) = strip_first_guid(entry) {
                nested
                    .entry(instance_guid)
                    .or_insert_with(Vec::new)
                    .push(rest);
            }
        }
    }
    nested
}
