//! State-layer row identity for the TopBar export quick menu.
//!
//! `ExportQuickRow` mirrors the rows `op_editor_ui::widgets::ExportQuickMenu`
//! paints so the host can record hover on `EditorUiState` without
//! `op-editor-core` depending on the widget crate (same discipline as
//! `topbar_state::TopBarButton` and `account_state::AccountMenuRow`).
//!
//! Which rows a given document actually offers is decided in the widget
//! layer (`widgets::export_menu_rows`), shared with the File menu's export
//! section so the two can never disagree about what is available.

/// One row of the TopBar export dropdown. The order here is the order a
/// deck shows them in; a non-deck document offers only the last two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportQuickRow {
    /// Editable PowerPoint deck (`FileAction::ExportPptx`).
    Pptx,
    /// Slide-per-page PDF — sets `export_format` and commits straight to
    /// the save picker, skipping the format/scale dialog.
    Pdf,
    /// Self-contained slideshow page (`FileAction::ExportSlideshowHtml`).
    SlideshowHtml,
    /// Opens the format/scale export dialog (`FileAction::ExportImage`).
    Image,
    /// One PNG per top-level frame (`FileAction::ExportAllFrames`).
    AllFrames,
}

impl ExportQuickRow {
    /// Every row in deck order — the row map filters this by availability.
    pub const ALL: [ExportQuickRow; 5] = [
        ExportQuickRow::Pptx,
        ExportQuickRow::Pdf,
        ExportQuickRow::SlideshowHtml,
        ExportQuickRow::Image,
        ExportQuickRow::AllFrames,
    ];
}
