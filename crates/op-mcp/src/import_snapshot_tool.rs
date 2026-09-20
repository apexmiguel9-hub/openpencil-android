use std::{collections::BTreeMap, fs};

use op_html::{import_snapshot, HtmlImportOptions};

use super::import_common::{import_result_to_outcome, parse_import_placement};
use super::{McpTool, ToolErrorCode, ToolOutcome};

pub struct ImportWebSnapshot;

impl McpTool for ImportWebSnapshot {
    fn name(&self) -> &str {
        "import_web_snapshot"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let snapshot = match args.get("snapshot") {
            Some(snapshot) => snapshot.clone(),
            None => {
                let Some(path) = args
                    .get("snapshotPath")
                    .or_else(|| args.get("snapshot_path"))
                else {
                    return ToolOutcome::Err(
                        ToolErrorCode::MissingArgument,
                        "snapshot or snapshotPath is required".into(),
                    );
                };
                match fs::read_to_string(path) {
                    Ok(snapshot) => snapshot,
                    Err(error) => {
                        return ToolOutcome::Err(
                            ToolErrorCode::ToolFailed,
                            format!("failed to read snapshotPath {path:?}: {error}"),
                        );
                    }
                }
            }
        };
        if snapshot.trim().is_empty() {
            return ToolOutcome::Err(
                ToolErrorCode::InvalidArgument,
                "snapshot must not be empty".into(),
            );
        }
        let placement = match parse_import_placement(args) {
            Ok(placement) => placement,
            Err((code, message)) => return ToolOutcome::Err(code, message),
        };

        let result = import_snapshot(&snapshot, &HtmlImportOptions::default());
        import_result_to_outcome(result, placement)
    }
}

pub fn import_web_snapshot_tool() -> ImportWebSnapshot {
    ImportWebSnapshot
}
