use super::*;

#[test]
fn cancelling_an_awaiting_session_marks_token_and_clears_selector() {
    let mut host = WidgetHostNative::new();
    let ui = &mut host.editor_state_mut().editor_ui;
    ui.figma_import_open = true;
    ui.figma_import_pages = vec![op_editor_core::FigmaImportPage {
        name: "Page".into(),
        layer_count: 1,
    }];
    ui.figma_import_page_select.open = true;
    let cancellation = CancellationToken::default();
    let mut session = Some(FigmaImportSession {
        path: PathBuf::from("fixture.fig"),
        stage: SessionStage::AwaitingSelection(None),
        cancellation: cancellation.clone(),
        output_mode: ImportOutputMode::CreateFixed,
    });

    assert!(finish_selection(
        &mut host,
        &mut session,
        op_editor_core::FigmaImportSelection::Cancel
    ));

    assert!(session.is_none());
    assert!(cancellation.is_cancelled());
    let ui = &host.editor_state().editor_ui;
    assert!(!ui.figma_import_open);
    assert!(ui.figma_import_pages.is_empty());
    assert!(!ui.figma_import_page_select.open);
}
