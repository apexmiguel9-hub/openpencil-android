//! Binding / alias / slash-path resolution helpers for the `batch_design`
//! DSL executor, plus the small id, page and forest utilities the
//! executor leans on.
//!
//! Split out of `batch_program.rs` to stay under the 800-line cap.

use std::collections::BTreeMap;

use jian_ops_schema::node::PenNode;
use op_editor_core::{EditorState, NodeId, PenNodeExt};

use super::batch_program::Result;
use super::batch_program_error::ProgramError;
use super::EditorCommand;

pub(crate) fn resolve_kit_component_id(raw: &str, state: &EditorState) -> Result<(String, String)> {
    let Some((kit_part, component_part)) = raw.split_once('/') else {
        return Err(ProgramError::Syntax(
            "K() kitComponentId must be starter/<id>, shadcn/<id>, or <kit-id>/<component-id>"
                .into(),
        ));
    };
    let kit_id = match kit_part {
        "starter" => "openpencil-starter".to_string(),
        "shadcn" => "shadcn-ui".to_string(),
        other => other.to_string(),
    };
    let component_id = if kit_id == "shadcn-ui" && !component_part.starts_with("shadcn-") {
        format!("shadcn-{component_part}")
    } else {
        component_part.to_string()
    };
    let Some(kit) = state.ui_kits.iter().find(|kit| kit.id == kit_id) else {
        return Err(ProgramError::NotFound(format!(
            "K() kit not found: {kit_part}"
        )));
    };
    if !kit
        .components
        .iter()
        .any(|component| component.id == component_id)
    {
        return Err(ProgramError::NotFound(format!(
            "K() component not found: {raw} (resolved to {}/{})",
            kit.id, component_id
        )));
    }
    Ok((kit.id.clone(), component_id))
}

// --- Reference + path resolution --------------------------------------

/// Resolve an operation parent while honoring the document-root spellings
/// advertised by the design-agent protocol. Bare `document` / `root` are
/// sentinels only when no same-named binding exists; quoted values remain
/// ordinary node ids.
pub(crate) fn resolve_parent_ref(raw: &str, bindings: &BTreeMap<String, String>) -> Option<String> {
    let trimmed = raw.trim();
    if matches!(trimmed, "null" | "undefined")
        || (matches!(trimmed, "document" | "root") && !bindings.contains_key(trimmed))
    {
        None
    } else {
        Some(resolve_ref(trimmed, bindings))
    }
}

/// TS `resolveRef` — strip one leading + one trailing double quote,
/// then look the cleaned token up in the binding table.
pub(crate) fn resolve_ref(raw: &str, bindings: &BTreeMap<String, String>) -> String {
    let cleaned = strip_outer_quotes(raw);
    bindings.get(cleaned.as_str()).cloned().unwrap_or(cleaned)
}

pub(crate) fn strip_outer_quotes(raw: &str) -> String {
    let s = raw.strip_prefix('"').unwrap_or(raw);
    let s = s.strip_suffix('"').unwrap_or(s);
    s.to_string()
}

/// TS `resolvePathExpr` — `binding+"/child"` concatenation, else a
/// plain (possibly quoted) ref.
pub(crate) fn resolve_path_expr(raw: &str, bindings: &BTreeMap<String, String>) -> String {
    if raw.contains('+') {
        return raw
            .split('+')
            .map(|part| {
                let t = part.trim();
                if t.starts_with('"') || t.starts_with('\'') {
                    // TS `slice(1, -1)` — drop the delimiters.
                    let mut cs = t.chars();
                    cs.next();
                    cs.next_back();
                    cs.as_str().to_string()
                } else {
                    bindings.get(t).cloned().unwrap_or_else(|| t.to_string())
                }
            })
            .collect::<Vec<_>>()
            .join("");
    }
    resolve_ref(raw, bindings)
}

/// One path segment → live sim id, translating authored ids through
/// the alias table when the segment doesn't resolve directly.
pub(crate) fn lookup_id(id: &str, alias: &BTreeMap<String, String>) -> String {
    alias.get(id).cloned().unwrap_or_else(|| id.to_string())
}

/// TS `findNodeByPath` — first segment is a deep `findNodeInTree`,
/// every later segment must be a DIRECT child id. Segments written
/// against authored (pre-remap) ids fall back through the alias table.
pub(crate) fn find_node_by_path<'a>(
    children: &'a [PenNode],
    path: &str,
    alias: &BTreeMap<String, String>,
) -> Option<&'a PenNode> {
    let mut parts = path.split('/');
    let first = parts.next()?;
    let first_id = lookup_id(first, alias);
    let mut current = op_editor_core::walkers::find_node(children, &NodeId::new(&first_id))?;
    for part in parts {
        let part_id = lookup_id(part, alias);
        let kids = current.children()?;
        current = kids.iter().find(|c| c.id_str() == part_id)?;
    }
    Some(current)
}

// --- Small helpers ------------------------------------------------------

pub(crate) fn parent_node_id(parent: Option<&str>) -> NodeId {
    match parent {
        None => NodeId::NONE,
        Some(raw) if raw.trim().is_empty() || raw.trim() == "0" => NodeId::NONE,
        Some(raw) => NodeId::new(raw.trim()),
    }
}

/// Stamp the program-level pageId onto a U() command built by the
/// shared direct-op builder (which emits `page_id: None`).
pub(crate) fn with_page_id(cmd: EditorCommand, page_id: Option<String>) -> EditorCommand {
    match cmd {
        EditorCommand::PatchNodeData {
            node_id,
            patch_json,
            ..
        } => EditorCommand::PatchNodeData {
            node_id,
            patch_json,
            page_id,
        },
        EditorCommand::UpdateNode {
            node_id,
            x,
            y,
            width,
            height,
            name,
            fill_hex,
            ..
        } => EditorCommand::UpdateNode {
            node_id,
            x,
            y,
            width,
            height,
            name,
            fill_hex,
            page_id,
        },
        other => other,
    }
}

/// Mirror of `command_apply::command_page_index`'s explicit-page arm:
/// page id match first, then a legacy numeric index.
pub(crate) fn resolve_page_index(state: &EditorState, raw: &str) -> Option<usize> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    match state.doc.pages.as_ref() {
        Some(pages) if !pages.is_empty() => pages
            .iter()
            .position(|page| page.id == raw)
            .or_else(|| raw.parse::<usize>().ok().filter(|idx| *idx < pages.len())),
        _ => raw.parse::<usize>().ok().filter(|idx| *idx == 0),
    }
}

/// TS error-line preview: 200 chars + `...`.
pub(crate) fn line_preview(line: &str) -> String {
    let mut preview: String = line.chars().take(200).collect();
    if line.chars().count() > 200 {
        preview.push_str("...");
    }
    preview
}

/// TS `countNodes`.
pub(crate) fn count_forest(nodes: &[PenNode]) -> usize {
    nodes
        .iter()
        .map(|node| {
            1 + node
                .children()
                .map(|children| count_forest(children))
                .unwrap_or(0)
        })
        .sum()
}
