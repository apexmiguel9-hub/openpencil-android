use super::*;
use op_editor_core::scene_template_catalog::TemplateScene;

fn deck_ui() -> EditorUiState {
    EditorUiState {
        scenario: Some(TemplateScene::Slides),
        deck_html_export_supported: true,
        batch_frame_export_supported: true,
        ..EditorUiState::default()
    }
}

fn plain_ui() -> EditorUiState {
    EditorUiState {
        batch_frame_export_supported: true,
        ..EditorUiState::default()
    }
}

fn panel_for(ui: &EditorUiState) -> (ExportQuickMenu<'_>, Rect) {
    let menu = ExportQuickMenu::for_editor_ui(ui);
    let rect = Rect {
        origin: Point2D::new(400.0, 46.0),
        size: Point2D::new(MENU_WIDTH, menu.height()),
    };
    (menu, rect)
}

/// Center of row `idx` inside `panel`.
fn row_point(panel: Rect, idx: usize) -> Point2D {
    Point2D::new(
        panel.origin.x + 20.0,
        panel.origin.y + PAD_Y + ROW_HEIGHT * idx as f32 + ROW_HEIGHT / 2.0,
    )
}

#[test]
fn deck_menu_lists_five_rows_with_powerpoint_first() {
    let ui = deck_ui();
    let menu = ExportQuickMenu::for_editor_ui(&ui);
    assert_eq!(menu.rows().len(), 5);
    assert_eq!(menu.rows()[0], ExportQuickRow::Pptx);
}

#[test]
fn non_deck_menu_drops_every_deck_row() {
    let ui = plain_ui();
    let menu = ExportQuickMenu::for_editor_ui(&ui);
    assert_eq!(
        menu.rows(),
        &[ExportQuickRow::Image, ExportQuickRow::AllFrames]
    );
}

#[test]
fn menu_without_batch_capability_shows_a_single_row() {
    let ui = EditorUiState::default();
    let menu = ExportQuickMenu::for_editor_ui(&ui);
    assert_eq!(menu.rows(), &[ExportQuickRow::Image]);
    assert_eq!(menu.height(), PAD_Y * 2.0 + ROW_HEIGHT);
}

#[test]
fn height_tracks_the_row_count() {
    let deck = deck_ui();
    let menu = ExportQuickMenu::for_editor_ui(&deck);
    assert_eq!(menu.height(), PAD_Y * 2.0 + ROW_HEIGHT * 5.0);
}

#[test]
fn each_deck_row_hit_resolves_in_paint_order() {
    let ui = deck_ui();
    let (menu, panel) = panel_for(&ui);
    let expected = [
        ExportQuickRow::Pptx,
        ExportQuickRow::Pdf,
        ExportQuickRow::SlideshowHtml,
        ExportQuickRow::Image,
        ExportQuickRow::AllFrames,
    ];
    for (idx, row) in expected.into_iter().enumerate() {
        assert_eq!(
            menu.hit(panel, row_point(panel, idx)),
            ExportQuickMenuHit::Row(row),
            "row {idx}"
        );
    }
}

#[test]
fn press_on_menu_padding_is_swallowed_not_a_row() {
    let ui = deck_ui();
    let (menu, panel) = panel_for(&ui);
    let top_padding = Point2D::new(panel.origin.x + 20.0, panel.origin.y + PAD_Y / 2.0);
    assert_eq!(menu.hit(panel, top_padding), ExportQuickMenuHit::Inside);
}

#[test]
fn press_outside_the_panel_reports_outside() {
    let ui = deck_ui();
    let (menu, panel) = panel_for(&ui);
    assert_eq!(
        menu.hit(
            panel,
            Point2D::new(panel.origin.x - 4.0, panel.origin.y + 10.0)
        ),
        ExportQuickMenuHit::Outside
    );
    assert_eq!(
        menu.hit(
            panel,
            Point2D::new(panel.origin.x + 10.0, panel.origin.y + panel.size.y + 4.0)
        ),
        ExportQuickMenuHit::Outside
    );
    assert!(menu.hovered_at(panel, Point2D::new(0.0, 0.0)).is_none());
}

#[test]
fn menu_hangs_under_its_anchor_right_aligned() {
    let ui = deck_ui();
    let menu = ExportQuickMenu::for_editor_ui(&ui);
    let anchor = Rect {
        origin: Point2D::new(900.0, 8.0),
        size: Point2D::new(28.0, 28.0),
    };
    let rect = menu.rect_at(anchor, 1280.0);
    assert_eq!(rect.origin.x + rect.size.x, anchor.origin.x + anchor.size.x);
    assert_eq!(rect.origin.y, anchor.origin.y + anchor.size.y + ANCHOR_GAP);
}

#[test]
fn narrow_viewport_clamps_the_menu_inside_the_window() {
    let ui = deck_ui();
    let menu = ExportQuickMenu::for_editor_ui(&ui);
    // Anchor pinned at the right edge of a window barely wider than the menu.
    let anchor = Rect {
        origin: Point2D::new(200.0, 8.0),
        size: Point2D::new(28.0, 28.0),
    };
    let rect = menu.rect_at(anchor, 240.0);
    assert!(rect.origin.x >= 8.0, "left edge stays on screen: {rect:?}");
}

#[test]
fn hovered_row_paints_a_tint() {
    let mut ui = deck_ui();
    ui.export_quick_menu_hover = Some(ExportQuickRow::Pdf);
    let (menu, panel) = panel_for(&ui);
    let mut backend = crate::widgets::test_capture_backend::CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    menu.paint(&mut cx, panel);
    // Card + border + one row tint — the tint is the only inset round-rect
    // narrower than the menu itself.
    assert!(
        backend
            .round_fills
            .iter()
            .any(|(rect, _, _)| rect.size.x < MENU_WIDTH && rect.size.y < ROW_HEIGHT),
        "hovered row must paint a tint"
    );
}

#[test]
fn labels_are_localized_and_lose_the_menu_ellipsis() {
    let ui = deck_ui();
    let menu = ExportQuickMenu::for_editor_ui(&ui);
    let pptx = menu.label(ExportQuickRow::Pptx);
    assert!(!pptx.is_empty());
    assert!(!pptx.ends_with('…') && !pptx.ends_with('.'), "{pptx:?}");
    assert!(menu.label(ExportQuickRow::Pdf).contains("PDF"));
}
