//! Shared plumbing for the HTML / web-snapshot import tools.
//!
//! `import_html`, `import_web_snapshot` (this crate), and
//! `import_html_url` (`op-host-services`) all place an imported
//! `PenNode` subtree onto the canvas with identical placement args and
//! insert-subtree outcome shape; the shared pieces live here.

use std::collections::BTreeMap;

use jian_ops_schema::node::PenNode;
use op_editor_core::NodeId;
use op_html::HtmlImportResult;

use super::write_tools::{parse_opt_i32, root_or_node_id};
use super::{EditorCommand, ToolErrorCode, ToolOutcome};

/// Placement args shared by every import tool: optional x/y offset for
/// the root frame, target parent, and target page.
pub struct ImportPlacement {
    pub x: i32,
    pub y: i32,
    pub parent_id: NodeId,
    pub page_id: Option<String>,
}

/// Parse the optional `viewportHeight` import arg (CSS pixels). Absent or
/// blank keeps the importer's aspect-derived default.
pub fn parse_viewport_height(
    args: &BTreeMap<String, String>,
) -> Result<Option<f64>, (ToolErrorCode, String)> {
    let Some(raw) = args
        .get("viewportHeight")
        .or_else(|| args.get("viewport_height"))
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    raw.parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(Some)
        .ok_or_else(|| {
            (
                ToolErrorCode::InvalidArgument,
                "viewportHeight: expected a positive number of CSS pixels".to_string(),
            )
        })
}

/// Parse the shared placement args. `Err` carries the typed tool error
/// to return to the caller.
pub fn parse_import_placement(
    args: &BTreeMap<String, String>,
) -> Result<ImportPlacement, (ToolErrorCode, String)> {
    let x = parse_opt_i32(args, "x")
        .map_err(|error| (ToolErrorCode::InvalidArgument, format!("x: {error}")))?
        .unwrap_or(0);
    let y = parse_opt_i32(args, "y")
        .map_err(|error| (ToolErrorCode::InvalidArgument, format!("y: {error}")))?
        .unwrap_or(0);
    let parent_id = args
        .get("parent")
        .or_else(|| args.get("parent_id"))
        .or_else(|| args.get("target_parent_id"))
        .map(|value| root_or_node_id(value))
        .unwrap_or(NodeId::NONE);
    let page_id = args
        .get("pageId")
        .or_else(|| args.get("page_id"))
        .or_else(|| args.get("page"))
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    Ok(ImportPlacement {
        x,
        y,
        parent_id,
        page_id,
    })
}

/// Total node count of an imported subtree (containers recurse through
/// `children`). Distinct from the `count_nodes` MCP read tool, which
/// counts the open document.
pub fn count_subtree_nodes(nodes: &[PenNode]) -> usize {
    nodes
        .iter()
        .map(|node| {
            1 + match node {
                PenNode::Frame(node) => node
                    .children
                    .as_deref()
                    .map(count_subtree_nodes)
                    .unwrap_or_default(),
                PenNode::Group(node) => node
                    .children
                    .as_deref()
                    .map(count_subtree_nodes)
                    .unwrap_or_default(),
                PenNode::Rectangle(node) => node
                    .children
                    .as_deref()
                    .map(count_subtree_nodes)
                    .unwrap_or_default(),
                _ => 0,
            }
        })
        .sum()
}

/// Turn an import result into the shared insert-subtree outcome:
/// reject empty imports, apply the x/y offset to the root frame, and
/// report `wrote` / `nodeCount` / `warnings`.
pub fn import_result_to_outcome(
    result: HtmlImportResult,
    placement: ImportPlacement,
) -> ToolOutcome {
    if result.nodes.is_empty() {
        let detail = result
            .warnings
            .first()
            .map(String::as_str)
            .unwrap_or("input produced no nodes");
        return ToolOutcome::Err(
            ToolErrorCode::InvalidArgument,
            format!("no importable content: {detail}"),
        );
    }
    let mut nodes = result.nodes;
    if placement.x != 0 || placement.y != 0 {
        if let PenNode::Frame(frame) = &mut nodes[0] {
            frame.base.x = Some(placement.x as f64);
            frame.base.y = Some(placement.y as f64);
        }
    }
    let mut output = BTreeMap::new();
    output.insert("wrote".into(), "true".into());
    output.insert("nodeCount".into(), count_subtree_nodes(&nodes).to_string());
    if !result.warnings.is_empty() {
        output.insert("warnings".into(), result.warnings.join("\n"));
    }
    ToolOutcome::OkWithCommand(
        output,
        EditorCommand::InsertSubtree {
            nodes,
            parent_id: placement.parent_id,
            page_id: placement.page_id,
        },
    )
}
