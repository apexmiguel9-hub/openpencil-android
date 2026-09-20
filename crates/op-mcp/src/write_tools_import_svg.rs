//! The `import_svg` write tool — carved off `write_tools.rs` to keep
//! both files under the 800-line cap. Re-exported from `write_tools`
//! so the public paths stay unchanged.

use std::{collections::BTreeMap, fs};

use op_editor_core::NodeId;

use super::write_tools::{parse_opt_i32, root_or_node_id};
use super::{EditorCommand, McpTool, ToolErrorCode, ToolOutcome};

/// First-party `import_svg` tool — parse an SVG document + insert the
/// resulting nodes on the active page. `x` / `y` (optional, default 0)
/// offset the imported nodes in doc-px.
pub struct ImportSvg;

impl McpTool for ImportSvg {
    fn name(&self) -> &str {
        "import_svg"
    }
    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let svg = match args.get("svg") {
            Some(svg) => svg.clone(),
            None => {
                let Some(path) = args.get("svgPath").or_else(|| args.get("svg_path")) else {
                    return ToolOutcome::Err(
                        ToolErrorCode::MissingArgument,
                        "svg or svgPath is required".into(),
                    );
                };
                match fs::read_to_string(path) {
                    Ok(svg) => svg,
                    Err(e) => {
                        return ToolOutcome::Err(
                            ToolErrorCode::ToolFailed,
                            format!("failed to read svgPath {path:?}: {e}"),
                        );
                    }
                }
            }
        };
        if svg.trim().is_empty() {
            return ToolOutcome::Err(
                ToolErrorCode::InvalidArgument,
                "svg must not be empty".into(),
            );
        }
        // `x` / `y` are optional doc-px offsets — absent ⇒ 0, a
        // malformed value rejects so the LLM sees a typed error.
        let x = match parse_opt_i32(args, "x") {
            Ok(v) => v.unwrap_or(0),
            Err(e) => return ToolOutcome::Err(ToolErrorCode::InvalidArgument, format!("x: {e}")),
        };
        let y = match parse_opt_i32(args, "y") {
            Ok(v) => v.unwrap_or(0),
            Err(e) => return ToolOutcome::Err(ToolErrorCode::InvalidArgument, format!("y: {e}")),
        };
        let target_parent = args
            .get("parent")
            .or_else(|| args.get("parent_id"))
            .or_else(|| args.get("target_parent_id"))
            .map(|s| root_or_node_id(s))
            .unwrap_or(NodeId::NONE);
        let page_id = args
            .get("pageId")
            .or_else(|| args.get("page_id"))
            .or_else(|| args.get("page"))
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let mut out = BTreeMap::new();
        out.insert("wrote".into(), "true".into());
        ToolOutcome::OkWithCommand(
            out,
            EditorCommand::ImportSvg {
                svg,
                x,
                y,
                target_parent,
                page_id,
            },
        )
    }
}

pub fn import_svg_snapshot() -> ImportSvg {
    ImportSvg
}
