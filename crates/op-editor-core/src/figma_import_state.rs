//! State-layer mirror of the Figma-import modal's interactive targets.
//!
//! `op_editor_ui::widgets::figma_import::FigmaImportHit` lives in the
//! widget crate, so this `Copy` mirror captures the two hoverable
//! targets — the close `✕` and the browse drop-zone — for the hover
//! state on `EditorUiState.figma_import_hover`. Same wasm32-clean
//! discipline as the other `*_state` mirrors.

/// What the import modal is currently importing. Both sources share
/// one modal — same geometry, same interactions, different copy — so
/// the two entry points can never drift into different dialogs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImportSource {
    #[default]
    Figma,
    Html,
}

/// Lightweight page metadata exposed after a binary `.fig` has been
/// prepared but before its nodes are converted. The prepared document
/// tree stays desktop-host-side; UI state carries only these labels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FigmaImportPage {
    pub name: String,
    pub layer_count: usize,
}

/// User decision made in the multi-page import modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FigmaImportSelection {
    Page(usize),
    All,
    Cancel,
}

/// Which Figma-import-modal target the cursor is over. `None` =
/// no hover wash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FigmaImportButton {
    /// The close `✕` in the modal header.
    Close,
    /// The dashed drop-zone (click to browse for a `.fig` file).
    DropZone,
    /// One row in the prepared file's page list.
    Page(usize),
    /// Convert every page in the prepared file.
    ImportAll,
}
