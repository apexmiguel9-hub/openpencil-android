//! Typed failure for the hidden `--render-shots` headless raster mode
//! (`render_cli.rs`).
//!
//! Style follows `op_orchestrator::OrchestratorError`: a plain enum plus a
//! hand-written `Display`, no `thiserror` and no new dependency. The variant
//! carries STRUCTURED fields and `Display` re-formats the sentence, so the
//! stderr line a benchmark driver greps is unchanged byte for byte.
//!
//! The mode has exactly one fallible seam that returns rather than exits: the
//! document load. Its other failure paths (`mkdir`, no active page, an empty
//! page, a node that would not render) print and `std::process::exit`
//! in place, so they never travel as a value. The enum exists anyway so the
//! next seam that needs to return joins it instead of re-introducing a
//! `Result<_, String>` here — and because the `render-shots: ` prefix is now
//! owned by `Display` rather than pasted at the one call site.
//!
//! `op_host_services::doc_io::load_editor_state` belongs to a crate this pass
//! does not own; its message is carried with `e.to_string()`.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RenderCliError {
    /// The `.op` named on argv could not be read or parsed.
    LoadDocument { file: String, message: String },
}

impl fmt::Display for RenderCliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderCliError::LoadDocument { file, message } => {
                write!(f, "render-shots: parse {file}: {message}")
            }
        }
    }
}

impl std::error::Error for RenderCliError {}
