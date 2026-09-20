//! MCP tool `export_deck` — writes the active page's boards as a
//! presentation deck in PowerPoint, self-contained HTML, or PDF.
//!
//! Input: `{ "format": "pptx"|"html"|"pdf", "outputPath": String }`
//!
//! Output: `{ "format": String, "path": String, "slides": Number,
//!            "structuredNodes"?: Number, "rasterFallbacks"?: Number }`
//!
//! The deck exporters have been in the desktop app since `v0.8.3` but were
//! reachable only from its File menu, so an agent driving the editor over
//! MCP could build a deck and then had no way to hand it to anyone.
//!
//! **The output argument is `outputPath`, not `filePath`.** `filePath` is
//! reserved workspace-wide for "the `.op` document this call targets" and is
//! intercepted before dispatch (`mcp_serve/file_path.rs`); naming the
//! destination `filePath` would route the call at a document that does not
//! exist instead of writing a deck.
//!
//! Unlike `export_item` / `export_nodes` this tool always writes a file
//! rather than returning base64. A deck is a multi-megabyte artifact and the
//! caller's goal is a shareable file; base64 stays the node-scoped answer.

use std::collections::BTreeMap;
use std::path::PathBuf;

use op_editor_core::EditorState;
use op_mcp::{McpTool, ToolErrorCode, ToolOutcome};

use crate::export_html::export_deck_html;
use crate::export_pdf::export_deck_pdf;
use crate::export_pptx::export_deck_pptx;

pub struct ExportDeck {
    state: EditorState,
}

impl McpTool for ExportDeck {
    fn name(&self) -> &str {
        "export_deck"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let format = match args.get("format").map(|value| value.trim()) {
            Some(format) if !format.is_empty() => format,
            _ => {
                return ToolOutcome::Err(
                    ToolErrorCode::MissingArgument,
                    "format is required (pptx|html|pdf)".into(),
                );
            }
        };
        let output_path = match args.get("outputPath").map(|value| value.trim()) {
            Some(path) if !path.is_empty() => PathBuf::from(path),
            _ => {
                return ToolOutcome::Err(
                    ToolErrorCode::MissingArgument,
                    "outputPath is required — export_deck writes a file".into(),
                );
            }
        };

        match format {
            "pptx" => match export_deck_pptx(&self.state, &output_path) {
                Ok(summary) => ToolOutcome::OkJson(deck_result(
                    "pptx",
                    &output_path,
                    summary.slides,
                    Some((summary.structured_nodes, summary.raster_fallbacks)),
                )),
                Err(error) => export_failed(error),
            },
            "html" => match export_deck_html(&self.state, &output_path) {
                Ok(summary) => ToolOutcome::OkJson(deck_result(
                    "html",
                    &output_path,
                    summary.slides,
                    Some((summary.structured_nodes, summary.raster_fallbacks)),
                )),
                Err(error) => export_failed(error),
            },
            // The PDF writer reports success by returning, not by counting:
            // its slide total is the board count it just walked.
            "pdf" => match export_deck_pdf(&self.state, &output_path) {
                Ok(()) => {
                    let slides =
                        op_editor_core::preview_slideshow::active_page_boards(&self.state).len();
                    ToolOutcome::OkJson(deck_result("pdf", &output_path, slides, None))
                }
                Err(error) => export_failed(error),
            },
            other => ToolOutcome::Err(
                ToolErrorCode::InvalidArgument,
                format!("unknown format {other:?}: must be one of pptx, html, pdf"),
            ),
        }
    }
}

/// Snapshot constructor. Registration is per-call and gated on the requested
/// tool (`should_register`), so this clone happens only when `export_deck` is
/// the tool being invoked — never on unrelated calls.
pub fn export_deck_snapshot(state: &EditorState) -> ExportDeck {
    ExportDeck {
        state: state.clone(),
    }
}

fn deck_result(
    format: &str,
    path: &std::path::Path,
    slides: usize,
    structured: Option<(usize, usize)>,
) -> String {
    let mut result = serde_json::json!({
        "format": format,
        "path": path.display().to_string(),
        "slides": slides,
    });
    // The PDF writer reports no structural split, so those keys are absent
    // rather than zero — a zero would claim every node rasterized.
    if let Some((structured_nodes, raster_fallbacks)) = structured {
        result["structuredNodes"] = structured_nodes.into();
        result["rasterFallbacks"] = raster_fallbacks.into();
    }
    result.to_string()
}

fn export_failed(error: crate::export::ExportError) -> ToolOutcome {
    ToolOutcome::Err(ToolErrorCode::Internal, error.to_string())
}
