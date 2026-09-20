//! Page-first-paint identity regression tests.

use super::*;
use jian_ops_schema::page::PenPage;

fn page(id: &str, name: &str) -> PenPage {
    PenPage {
        id: id.into(),
        name: name.into(),
        children: Vec::new(),
        background_color: None,
        state: None,
        lifecycle: None,
    }
}

#[test]
fn index_reuse_after_page_removal_changes_identity() {
    let mut app = DesktopApp::new(None);
    app.host.editor_state_mut().doc.pages =
        Some(vec![page("page-a", "Page A"), page("page-b", "Page B")]);
    app.host.editor_state_mut().ui.active_page_index = 0;

    let before = app.active_page_paint_identity();
    app.host
        .editor_state_mut()
        .doc
        .pages
        .as_mut()
        .expect("pages")
        .remove(0);
    let after = app.active_page_paint_identity();

    assert_eq!(before.document_epoch, after.document_epoch);
    assert_eq!(before.page_id, "page-a");
    assert_eq!(after.page_id, "page-b");
    assert_ne!(before, after, "an index must not identify a logical page");
}

#[test]
fn whole_document_replacement_changes_identity_for_same_page_id() {
    let mut app = DesktopApp::new(None);
    app.host.editor_state_mut().doc.pages = Some(vec![page("page-a", "Old Page")]);
    let before = app.active_page_paint_identity();

    let mut replacement = op_editor_core::EditorState::starter();
    replacement.doc.pages = Some(vec![page("page-a", "Replacement Page")]);
    app.host.replace_editor_state(replacement);
    let after = app.active_page_paint_identity();

    assert_eq!(before.page_id, after.page_id);
    assert_eq!(after.document_epoch, before.document_epoch + 1);
    assert_ne!(
        before, after,
        "the epoch must distinguish replaced documents"
    );
}

#[test]
fn duplicate_legacy_page_ids_fall_back_to_index() {
    let mut app = DesktopApp::new(None);
    app.host.editor_state_mut().doc.pages = Some(vec![
        page("duplicate", "Page A"),
        page("duplicate", "Page B"),
    ]);
    app.host.editor_state_mut().ui.active_page_index = 0;
    let first = app.active_page_paint_identity();

    app.host.editor_state_mut().ui.active_page_index = 1;
    let second = app.active_page_paint_identity();

    assert_eq!(first.page_id, second.page_id);
    assert_eq!(first.duplicate_index, Some(0));
    assert_eq!(second.duplicate_index, Some(1));
    assert_ne!(first, second);
}
