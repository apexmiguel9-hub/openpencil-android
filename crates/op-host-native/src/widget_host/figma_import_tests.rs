use super::WidgetHostNative;
use op_editor_core::{EditorState, Locale};

const VIEWPORT_W: f32 = 1200.0;
const VIEWPORT_H: f32 = 800.0;

fn page_selector_host(page_count: usize) -> WidgetHostNative {
    let mut host = WidgetHostNative::new();
    let ui = &mut host.editor_state_mut().editor_ui;
    ui.figma_import_open = true;
    ui.figma_import_pages = (0..page_count)
        .map(|index| op_editor_core::FigmaImportPage {
            name: format!("Page {index}"),
            layer_count: index,
        })
        .collect();
    ui.figma_import_page_select.open = true;
    host
}

fn point_for_hit(
    modal: &op_editor_ui::widgets::figma_import::FigmaImportModal,
    panel: op_editor_ui::Rect,
    wanted: op_editor_ui::widgets::figma_import::FigmaImportHit,
) -> op_editor_ui::Point2D {
    let mut y = panel.origin.y;
    while y <= panel.origin.y + panel.size.y {
        let mut x = panel.origin.x;
        while x <= panel.origin.x + panel.size.x {
            let point = op_editor_ui::Point2D::new(x, y);
            if modal.hit_test(panel, point) == wanted {
                return point;
            }
            x += 4.0;
        }
        y += 4.0;
    }
    panic!("hit target not found: {wanted:?}");
}

#[test]
fn install_imported_state_preserves_live_ui_and_defers_layout() {
    let mut host = WidgetHostNative::new();
    host.editor_state.editor_ui.locale = Locale::ZhCn;
    host.editor_state.editor_ui.sidebar_open = false;
    host.editor_state.editor_ui.figma_import_in_progress = true;
    host.editor_state_dirty = false;

    let mut imported = EditorState::new();
    imported.editor_ui.file_name_display = Some("Dashboard.fig".into());
    imported.editor_ui.locale = Locale::EnUs;
    imported.editor_ui.sidebar_open = true;

    host.install_imported_state(imported);

    assert_eq!(host.editor_state.editor_ui.locale, Locale::ZhCn);
    assert!(!host.editor_state.editor_ui.sidebar_open);
    assert!(!host.editor_state.editor_ui.figma_import_in_progress);
    assert_eq!(
        host.editor_state.editor_ui.file_name_display.as_deref(),
        Some("Dashboard.fig")
    );
    assert!(host.editor_state_dirty);
    assert!(host.layout_scene.pages.is_empty());
}

#[test]
fn install_imported_state_keeps_the_incoming_saved_and_dirty_markers() {
    let mut clean_host = WidgetHostNative::new();
    clean_host.editor_state.mark_saved_revision();
    let mut dirty_import = EditorState::new();
    dirty_import.mark_document_changed();
    let dirty_revision = dirty_import.document_revision();
    let dirty_saved_revision = dirty_import.saved_revision();

    clean_host.install_imported_state(dirty_import);

    assert!(clean_host.editor_state.is_dirty());
    assert!(clean_host.editor_state.editor_ui.document_dirty);
    assert_eq!(clean_host.editor_state.document_revision(), dirty_revision);
    assert_eq!(
        clean_host.editor_state.saved_revision(),
        dirty_saved_revision
    );

    let mut dirty_host = WidgetHostNative::new();
    dirty_host.editor_state.mark_document_changed();
    let mut saved_import = EditorState::new();
    saved_import.mark_document_changed();
    saved_import.mark_saved_revision();
    let saved_revision = saved_import.saved_revision();

    dirty_host.install_imported_state(saved_import);

    assert!(!dirty_host.editor_state.is_dirty());
    assert!(!dirty_host.editor_state.editor_ui.document_dirty);
    assert_eq!(dirty_host.editor_state.document_revision(), saved_revision);
    assert_eq!(dirty_host.editor_state.saved_revision(), saved_revision);
}

#[test]
fn install_imported_state_rebuilds_even_when_the_import_matches_the_cache() {
    let mut host = WidgetHostNative::new();
    // Prime the scene-build cache from the current document.
    host.editor_state_dirty = true;
    let _ = host.layout_scene();
    assert!(
        !host.layout_scene.pages.is_empty(),
        "precondition: a scene is built"
    );

    // Import a state whose scene inputs are identical to the cached build (same
    // document + same authored-geometry latch). `install_imported_state` takes
    // (empties) the scene and defers the rebuild; without invalidating the cache
    // the matching-input refresh would skip and leave the canvas blank.
    let mut imported = EditorState::from_document(host.editor_state.doc.clone());
    imported.editor_ui.preserve_authored_geometry =
        host.editor_state.editor_ui.preserve_authored_geometry;
    host.install_imported_state(imported);
    assert!(
        host.layout_scene.pages.is_empty(),
        "import defers (empties) the scene"
    );

    let _ = host.layout_scene();
    assert!(
        !host.layout_scene.pages.is_empty(),
        "import must rebuild the scene even when its inputs match the cache"
    );
}

#[test]
fn page_row_and_import_all_route_distinct_file_actions() {
    let mut page_host = page_selector_host(12);
    let modal =
        op_editor_ui::widgets::figma_import::FigmaImportModal::for_editor(page_host.editor_state());
    let panel = modal.rect(VIEWPORT_W, VIEWPORT_H);
    let list = modal.page_list_rect(panel).expect("page list");
    assert!(page_host.apply_press(
        list.origin.x + 20.0,
        list.origin.y + 45.0,
        VIEWPORT_W,
        VIEWPORT_H
    ));
    assert_eq!(
        page_host.editor_state().editor_ui.pending_file_action,
        Some(op_editor_core::FileAction::FinishFigmaImport(
            op_editor_core::FigmaImportSelection::Page(1)
        ))
    );

    let mut all_host = page_selector_host(12);
    let modal =
        op_editor_ui::widgets::figma_import::FigmaImportModal::for_editor(all_host.editor_state());
    let panel = modal.rect(VIEWPORT_W, VIEWPORT_H);
    let point = point_for_hit(
        &modal,
        panel,
        op_editor_ui::widgets::figma_import::FigmaImportHit::ImportAll,
    );
    assert!(all_host.apply_press(point.x, point.y, VIEWPORT_W, VIEWPORT_H));
    assert_eq!(
        all_host.editor_state().editor_ui.pending_file_action,
        Some(op_editor_core::FileAction::FinishFigmaImport(
            op_editor_core::FigmaImportSelection::All
        ))
    );
}

#[test]
fn closing_page_selector_routes_cancel() {
    let mut host = page_selector_host(12);
    let modal =
        op_editor_ui::widgets::figma_import::FigmaImportModal::for_editor(host.editor_state());
    let panel = modal.rect(VIEWPORT_W, VIEWPORT_H);
    let point = point_for_hit(
        &modal,
        panel,
        op_editor_ui::widgets::figma_import::FigmaImportHit::Close,
    );

    assert!(host.apply_press(point.x, point.y, VIEWPORT_W, VIEWPORT_H));
    assert_eq!(
        host.editor_state().editor_ui.pending_file_action,
        Some(op_editor_core::FileAction::FinishFigmaImport(
            op_editor_core::FigmaImportSelection::Cancel
        ))
    );
    assert!(!host.editor_state().editor_ui.figma_import_open);
}

#[test]
fn page_selector_wheel_scrolls_without_zooming_canvas() {
    let mut host = page_selector_host(70);
    host.editor_state_mut()
        .editor_ui
        .figma_import_page_select
        .hover = Some(0);
    let modal =
        op_editor_ui::widgets::figma_import::FigmaImportModal::for_editor(host.editor_state());
    let panel = modal.rect(VIEWPORT_W, VIEWPORT_H);
    let list = modal.page_list_rect(panel).expect("page list");
    let zoom = host.editor_state().viewport.zoom;

    assert!(host.apply_wheel(
        list.origin.x + 20.0,
        list.origin.y + 20.0,
        -90.0,
        VIEWPORT_W,
        VIEWPORT_H
    ));

    let ui = &host.editor_state().editor_ui;
    assert_eq!(ui.figma_import_page_select.scroll.offset, 90.0);
    assert_eq!(ui.figma_import_page_select.hover, None);
    assert_eq!(host.editor_state().viewport.zoom, zoom);
}
