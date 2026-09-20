//! Snapshot → `.op` document conversion.
//!
//! The offline download path no longer hands the user the raw extractor
//! snapshot (which then had to be converted with `op import:snapshot`). It
//! produces a `.op` document — OpenPencil's canonical `PenDocument` format —
//! that opens directly by double-click or drag-into-app, still fully offline.
//!
//! This mirrors `op-cli`'s `run_import_snapshot` (`html_cli.rs`) step for step:
//! `import_snapshot_document` → refuse an empty document → serialize the
//! resulting `PenDocument` to JSON. The serialized string IS the `.op` file.
//! The page title flows through `HtmlImportOptions::document_name` so the
//! produced document carries a name.

use jian_ops_schema::node::PenNode;
use op_html::HtmlImportOptions;

/// Outcome of converting one snapshot into a `.op` document.
pub enum OpExport {
    /// The snapshot produced at least one node. `op` is the serialized
    /// `PenDocument` JSON — the exact bytes to write to a `.op` file.
    Ready {
        op: String,
        node_count: usize,
        warnings: Vec<String>,
    },
    /// The snapshot produced no importable content (empty capture, unsupported
    /// version, malformed JSON). `error` mirrors the CLI's message.
    Failed { error: String },
}

/// Convert an extractor snapshot into a `.op` document.
///
/// `title`, when non-empty, becomes the document's name via
/// [`HtmlImportOptions::document_name`], so the `.op` file opens with a
/// meaningful title rather than an anonymous fallback.
pub fn snapshot_to_op(snapshot_json: &str, title: Option<&str>) -> OpExport {
    let options = HtmlImportOptions {
        document_name: title
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(str::to_owned),
        ..HtmlImportOptions::default()
    };
    let imported = op_html::import_snapshot_document(snapshot_json, &options);
    if imported.document.children.is_empty() {
        // Byte-identical to the CLI's "no importable content" refusal so the
        // two paths report an empty capture the same way.
        let detail = imported
            .warnings
            .first()
            .map(String::as_str)
            .unwrap_or("input produced no nodes");
        return OpExport::Failed {
            error: format!("no importable content: {detail}"),
        };
    }
    let op = match serde_json::to_value(&imported.document) {
        Ok(value) => value.to_string(),
        Err(error) => {
            return OpExport::Failed {
                error: format!("serialize .op document: {error}"),
            };
        }
    };
    OpExport::Ready {
        op,
        node_count: count_nodes(&imported.document.children),
        warnings: imported.warnings,
    }
}

/// Total node count across the tree, mirroring `op-cli`'s `count_nodes`. The
/// three container variants that carry `children` recurse; every other variant
/// is a leaf.
fn count_nodes(nodes: &[PenNode]) -> usize {
    nodes
        .iter()
        .map(|node| {
            1 + match node {
                PenNode::Frame(node) => node
                    .children
                    .as_deref()
                    .map(count_nodes)
                    .unwrap_or_default(),
                PenNode::Group(node) => node
                    .children
                    .as_deref()
                    .map(count_nodes)
                    .unwrap_or_default(),
                PenNode::Rectangle(node) => node
                    .children
                    .as_deref()
                    .map(count_nodes)
                    .unwrap_or_default(),
                _ => 0,
            }
        })
        .sum()
}
