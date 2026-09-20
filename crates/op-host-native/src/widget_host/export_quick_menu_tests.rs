//! TopBar export quick-menu host behaviour: opening it from the download
//! button, what each row queues, and how it coexists with the other
//! chrome dropdowns and with a running presentation.

use super::WidgetHostNative;
use op_editor_core::editor_ui_state::{ExportFormat, FileAction};
use op_editor_core::scene_template_catalog::TemplateScene;
use op_editor_core::{EditorState, ExportQuickRow};
use op_editor_ui::widgets::{ExportQuickMenu, TopBar, TOP_BAR_HEIGHT};
use op_editor_ui::{Point2D, Rect};

const VIEWPORT_W: f32 = 1400.0;
const VIEWPORT_H: f32 = 900.0;

/// Three 16:9 boards side by side — the shape a generated deck has.
const THREE_BOARD_DECK: &str = r##"{
    "version": "1.0.0",
    "children": [
        { "type": "frame", "id": "slide-1", "x": 0, "y": 0,
          "width": 1920, "height": 1080,
          "fill": [{"type":"solid","color":"#ffffff"}], "children": [] },
        { "type": "frame", "id": "slide-2", "x": 2100, "y": 0,
          "width": 1920, "height": 1080,
          "fill": [{"type":"solid","color":"#eeeeee"}], "children": [] }
    ]
}"##;

/// A host whose document is a deck and whose capability flags match the
/// desktop shell (both deck export formats + the batch frame export).
fn deck_host() -> WidgetHostNative {
    let document = jian_ops_schema::load_str(THREE_BOARD_DECK)
        .expect("parse deck fixture")
        .value;
    let mut host = WidgetHostNative::new();
    let mut state = EditorState::from_document(document);
    state.editor_ui.scenario = Some(TemplateScene::Slides);
    host.install_imported_state(state);
    // Capability flags are host-owned, so they are set on the live host
    // rather than on the document state being installed.
    let ui = &mut host.editor_state_mut().editor_ui;
    ui.scenario = Some(TemplateScene::Slides);
    ui.deck_html_export_supported = true;
    ui.batch_frame_export_supported = true;
    host
}

fn plain_host() -> WidgetHostNative {
    let mut host = WidgetHostNative::new();
    host.editor_state_mut()
        .editor_ui
        .batch_frame_export_supported = true;
    host
}

fn top_bar_rect() -> Rect {
    Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(VIEWPORT_W, TOP_BAR_HEIGHT),
    }
}

/// Center of the TopBar download button.
fn export_button_point(host: &WidgetHostNative) -> Point2D {
    let bar = TopBar::for_editor_ui(&host.editor_state.editor_ui);
    let rect = bar.export_button_rect(top_bar_rect());
    Point2D::new(
        rect.origin.x + rect.size.x / 2.0,
        rect.origin.y + rect.size.y / 2.0,
    )
}

/// Center of the open menu's row for `row`.
fn row_point(host: &WidgetHostNative, row: ExportQuickRow) -> Point2D {
    let panel = host.export_quick_menu_rect(VIEWPORT_W);
    let menu = ExportQuickMenu::for_editor_ui(&host.editor_state.editor_ui);
    let index = menu
        .rows()
        .iter()
        .position(|candidate| *candidate == row)
        .unwrap_or_else(|| panic!("{row:?} is offered by this document"));
    // Row geometry is the widget's; probe the vertical center of each row
    // by walking the menu's own hit-test instead of re-deriving constants.
    let step = (panel.size.y - 12.0) / menu.rows().len() as f32;
    Point2D::new(
        panel.origin.x + 20.0,
        panel.origin.y + 6.0 + step * index as f32 + step / 2.0,
    )
}

fn open_menu(host: &mut WidgetHostNative) {
    let point = export_button_point(host);
    assert!(host.apply_press(point.x, point.y, VIEWPORT_W, VIEWPORT_H));
    assert!(
        host.editor_state.editor_ui.export_quick_menu_open,
        "the download button opens the export menu"
    );
}

#[test]
fn download_button_opens_and_a_second_press_closes_the_menu() {
    let mut host = plain_host();
    open_menu(&mut host);

    // The button sits above the open menu, so the same point is an
    // outside press for the menu — it must close, not re-toggle open.
    let point = export_button_point(&host);
    assert!(host.apply_press(point.x, point.y, VIEWPORT_W, VIEWPORT_H));
    assert!(!host.editor_state.editor_ui.export_quick_menu_open);
}

#[test]
fn a_deck_offers_five_rows_led_by_powerpoint() {
    let mut host = deck_host();
    open_menu(&mut host);
    let menu = ExportQuickMenu::for_editor_ui(&host.editor_state.editor_ui);
    assert_eq!(menu.rows().len(), 5);
    assert_eq!(menu.rows()[0], ExportQuickRow::Pptx);
}

#[test]
fn a_non_deck_document_offers_no_deck_rows() {
    let mut host = plain_host();
    open_menu(&mut host);
    let menu = ExportQuickMenu::for_editor_ui(&host.editor_state.editor_ui);
    assert_eq!(
        menu.rows(),
        &[ExportQuickRow::Image, ExportQuickRow::AllFrames]
    );
}

#[test]
fn powerpoint_row_queues_the_pptx_file_action_and_closes_the_menu() {
    let mut host = deck_host();
    open_menu(&mut host);
    let point = row_point(&host, ExportQuickRow::Pptx);
    assert!(host.apply_press(point.x, point.y, VIEWPORT_W, VIEWPORT_H));
    assert_eq!(
        host.editor_state.editor_ui.pending_file_action,
        Some(FileAction::ExportPptx)
    );
    assert!(!host.editor_state.editor_ui.export_quick_menu_open);
}

#[test]
fn pdf_row_sets_the_format_and_commits_straight_to_the_save_picker() {
    let mut host = deck_host();
    assert_ne!(host.editor_state.editor_ui.export_format, ExportFormat::Pdf);
    open_menu(&mut host);
    let point = row_point(&host, ExportQuickRow::Pdf);
    assert!(host.apply_press(point.x, point.y, VIEWPORT_W, VIEWPORT_H));
    assert_eq!(host.editor_state.editor_ui.export_format, ExportFormat::Pdf);
    assert_eq!(
        host.editor_state.editor_ui.pending_file_action,
        Some(FileAction::ExportImageConfirm),
        "PDF skips the format dialog — picking it IS that choice"
    );
    assert!(!host.editor_state.editor_ui.export_dialog_open);
}

#[test]
fn slideshow_and_frame_rows_queue_their_own_actions() {
    let mut host = deck_host();
    open_menu(&mut host);
    let point = row_point(&host, ExportQuickRow::SlideshowHtml);
    assert!(host.apply_press(point.x, point.y, VIEWPORT_W, VIEWPORT_H));
    assert_eq!(
        host.editor_state.editor_ui.pending_file_action,
        Some(FileAction::ExportSlideshowHtml)
    );

    open_menu(&mut host);
    let point = row_point(&host, ExportQuickRow::AllFrames);
    assert!(host.apply_press(point.x, point.y, VIEWPORT_W, VIEWPORT_H));
    assert_eq!(
        host.editor_state.editor_ui.pending_file_action,
        Some(FileAction::ExportAllFrames)
    );
}

#[test]
fn image_row_routes_through_the_existing_export_dialog_action() {
    let mut host = plain_host();
    open_menu(&mut host);
    let point = row_point(&host, ExportQuickRow::Image);
    assert!(host.apply_press(point.x, point.y, VIEWPORT_W, VIEWPORT_H));
    assert_eq!(
        host.editor_state.editor_ui.pending_file_action,
        Some(FileAction::ExportImage)
    );
}

#[test]
fn a_press_outside_the_menu_dismisses_it_without_queueing_anything() {
    let mut host = deck_host();
    open_menu(&mut host);
    let panel = host.export_quick_menu_rect(VIEWPORT_W);
    assert!(host.apply_press(
        panel.origin.x - 40.0,
        panel.origin.y + panel.size.y / 2.0,
        VIEWPORT_W,
        VIEWPORT_H,
    ));
    assert!(!host.editor_state.editor_ui.export_quick_menu_open);
    assert_eq!(host.editor_state.editor_ui.pending_file_action, None);
}

#[test]
fn hovering_a_row_records_it_and_leaving_the_menu_clears_it() {
    let mut host = deck_host();
    // `apply_cursor_move` reads the viewport the last paint measured.
    host.last_viewport_w = VIEWPORT_W;
    host.last_viewport_h = VIEWPORT_H;
    open_menu(&mut host);
    let point = row_point(&host, ExportQuickRow::Pdf);
    host.apply_cursor_move(point.x, point.y);
    assert_eq!(
        host.editor_state.editor_ui.export_quick_menu_hover,
        Some(ExportQuickRow::Pdf)
    );
    let panel = host.export_quick_menu_rect(VIEWPORT_W);
    host.apply_cursor_move(panel.origin.x - 40.0, panel.origin.y + panel.size.y / 2.0);
    assert_eq!(host.editor_state.editor_ui.export_quick_menu_hover, None);
}

#[test]
fn opening_another_top_bar_dropdown_closes_the_export_menu() {
    let mut host = plain_host();
    // Locale picker first, then the export button: the export menu must
    // take the tier over, not stack under an already-open picker.
    op_editor_core::host_press_transitions::toggle_locale_picker(&mut host.editor_state.editor_ui);
    assert!(host.editor_state.editor_ui.locale_picker.open);
    op_editor_core::host_press_transitions::toggle_export_quick_menu(
        &mut host.editor_state.editor_ui,
    );
    assert!(host.editor_state.editor_ui.export_quick_menu_open);
    assert!(!host.editor_state.editor_ui.locale_picker.open);

    // And the other way round.
    op_editor_core::host_press_transitions::toggle_locale_picker(&mut host.editor_state.editor_ui);
    assert!(host.editor_state.editor_ui.locale_picker.open);
    assert!(!host.editor_state.editor_ui.export_quick_menu_open);

    // The file menu shares the same exclusion.
    op_editor_core::host_press_transitions::toggle_export_quick_menu(
        &mut host.editor_state.editor_ui,
    );
    op_editor_core::host_press_transitions::toggle_file_menu(&mut host.editor_state.editor_ui);
    assert!(host.editor_state.editor_ui.file_menu_open);
    assert!(!host.editor_state.editor_ui.export_quick_menu_open);
}

#[test]
fn escape_closes_the_export_menu_one_layer_at_a_time() {
    let mut host = plain_host();
    open_menu(&mut host);
    assert!(host.apply_escape());
    assert!(!host.editor_state.editor_ui.export_quick_menu_open);
    assert_eq!(host.editor_state.editor_ui.export_quick_menu_hover, None);
}

/// The TopBar stays painted while a deck presents (it carries the exit
/// control), so its buttons must keep working — including this one, which
/// is how a presenter exports the deck they are showing.
///
/// Windows-gated for the same reason the other preview host tests are:
/// entering preview solves layout through `jian_skia::SkiaMeasure`, which
/// aborts the process under Windows CI's DirectWrite.
#[cfg(not(target_os = "windows"))]
#[test]
fn presenting_leaves_the_export_button_reachable() {
    let mut host = deck_host();
    assert!(host.enter_preview((VIEWPORT_W, VIEWPORT_H)));
    assert!(host.preview_slideshow_active());

    let point = export_button_point(&host);
    assert!(host.apply_press(point.x, point.y, VIEWPORT_W, VIEWPORT_H));
    assert!(host.editor_state.editor_ui.export_quick_menu_open);
    assert!(
        host.preview_slideshow_active(),
        "opening the menu must not end the presentation"
    );

    let point = row_point(&host, ExportQuickRow::Pptx);
    assert!(host.apply_press(point.x, point.y, VIEWPORT_W, VIEWPORT_H));
    assert_eq!(
        host.editor_state.editor_ui.pending_file_action,
        Some(FileAction::ExportPptx)
    );
    assert!(host.preview_slideshow_active());
}
