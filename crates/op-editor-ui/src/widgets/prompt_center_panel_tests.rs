use op_editor_core::prompt_center_catalog::PromptCategory;
use op_editor_core::{CustomPrompt, EditorState, Locale, PromptFilter};

use super::test_rects;
use super::{PromptCenterHit, PromptCenterPanel};
use crate::widgets::canvas_viewport_image::{
    lock_decode_registry_for_tests, mark_decode_done, take_pending_decodes,
};
use crate::widgets::property_panel_test_support::CountingBackend;
use crate::widgets::PaintCx;
use crate::{ImageDrawMode, Point2D, Rect};

fn open_state(locale: Locale) -> EditorState {
    let mut state = EditorState::new();
    state.editor_ui.locale = locale;
    state.editor_ui.open_prompt_center(1);
    state
}

fn panel_rect() -> Rect {
    test_rects::medium()
}

fn filtered_ids(state: &EditorState) -> Vec<String> {
    PromptCenterPanel::for_editor(state)
        .expect("open panel")
        .filtered()
        .into_iter()
        .map(|card| card.id.into_owned())
        .collect()
}

fn visible_card_count(panel: &PromptCenterPanel<'_>, rect: Rect) -> usize {
    let viewport = panel.cards_viewport(rect);
    let bottom = viewport.origin.y + viewport.size.y;
    panel
        .card_rects(rect)
        .into_iter()
        .filter(|(_, card)| {
            card.origin.y + card.size.y > viewport.origin.y && card.origin.y < bottom
        })
        .count()
}

#[test]
fn travel_search_matches_both_chinese_and_english() {
    let mut state = open_state(Locale::ZhCn);
    state.editor_ui.prompt_center.search.set_text("旅行");
    assert!(filtered_ids(&state).iter().any(|id| id == "gallery-wander"));

    state.editor_ui.prompt_center.search.set_text("travel");
    assert!(filtered_ids(&state).iter().any(|id| id == "gallery-wander"));
}

#[test]
fn category_filter_only_returns_that_category() {
    let mut state = open_state(Locale::EnUs);
    // Slices rather than fixed-size arrays: the categories do not all hold
    // the same number of prompts, and the tuple list has to stay one type.
    let expected: [(PromptCategory, Option<&[&str]>); 5] = [
        (PromptCategory::Starter, None),
        (
            PromptCategory::WebPage,
            Some(&["web-orbit", "web-atelier", "web-kilnform", "web-reefwright"]),
        ),
        (
            PromptCategory::Dashboard,
            Some(&["dashboard-pulse", "dashboard-sentinel"]),
        ),
        (
            PromptCategory::Component,
            Some(&["component-data-grid", "component-form-lab"]),
        ),
        (
            PromptCategory::Modify,
            Some(&["modify-polish-current", "modify-complete-states"]),
        ),
    ];

    for (category, expected_ids) in expected {
        state.editor_ui.prompt_center.filter = PromptFilter::Category(category);
        let cards = PromptCenterPanel::for_editor(&state)
            .expect("open panel")
            .filtered();
        assert!(
            !cards.is_empty(),
            "{category:?} should have built-in prompts"
        );
        assert!(
            cards.iter().all(|card| card.category == category),
            "{category:?} filter leaked another category"
        );
        if let Some(expected_ids) = expected_ids {
            assert_eq!(
                cards
                    .iter()
                    .map(|card| card.id.as_ref())
                    .collect::<Vec<_>>(),
                expected_ids,
                "{category:?} prompt order changed"
            );
        }
    }
}

#[test]
fn unmatched_query_produces_empty_result() {
    let mut state = open_state(Locale::EnUs);
    state
        .editor_ui
        .prompt_center
        .search
        .set_text("no-such-prompt-9d79e9");
    assert!(PromptCenterPanel::for_editor(&state)
        .expect("open panel")
        .filtered()
        .is_empty());
}

/// Cards on the first row, in x order — the row is the grid's column count.
fn first_row_columns(state: &EditorState, rect: Rect) -> usize {
    let panel = PromptCenterPanel::for_editor(state).expect("open panel");
    let rects = panel.card_rects(rect);
    let top = rects[0].1.origin.y;
    rects
        .iter()
        .take_while(|(_, card)| card.origin.y == top)
        .count()
}

#[test]
fn grid_lays_cards_out_in_rows_and_columns() {
    let state = open_state(Locale::EnUs);
    let panel = PromptCenterPanel::for_editor(&state).expect("open panel");
    let rect = panel_rect();
    let rects = panel.card_rects(rect);
    let columns = first_row_columns(&state, rect);
    assert!(rects.len() > columns, "need more than one row to test rows");

    assert_eq!(rects[0].1.origin.y, rects[1].1.origin.y);
    assert!(rects[1].1.origin.x > rects[0].1.origin.x);
    assert_eq!(rects[0].1.origin.x, rects[columns].1.origin.x);
    assert!(rects[columns].1.origin.y > rects[0].1.origin.y);

    let preview = PromptCenterPanel::card_preview_rect(rects[0].1);
    assert!(rects[0].1.contains(preview.origin));
    assert!(
        (preview.size.x / preview.size.y - 16.0 / 10.0).abs() < 0.001,
        "preview must remain 16:10"
    );
    assert!(preview.origin.y + preview.size.y < rects[0].1.origin.y + rects[0].1.size.y);
}

/// The whole point of the resize: a wider panel buys more cards per row, and
/// the card height tracks the width so the preview keeps its aspect.
///
/// This is what a fixed `CARD_COLS = 2` / `CARD_H = 262` pair cannot do — put
/// either back and the first assertion fails on the very first pair.
#[test]
fn grid_columns_and_card_height_follow_panel_width() {
    let state = open_state(Locale::EnUs);
    let mut seen: Vec<(usize, f32, f32)> = Vec::new();
    for rect in [
        test_rects::narrow(),
        test_rects::medium(),
        test_rects::wide(),
    ] {
        let panel = PromptCenterPanel::for_editor(&state).expect("open panel");
        let card = panel.card_rects(rect)[0].1;
        seen.push((first_row_columns(&state, rect), card.size.x, card.size.y));
    }
    assert_eq!(
        seen.iter().map(|entry| entry.0).collect::<Vec<_>>(),
        vec![2, 3, 4],
        "column count must step up with the viewport"
    );
    for entry in &seen {
        let expected_h = 10.0 + (entry.1 - 20.0) / (16.0 / 10.0) + 54.0;
        assert!(
            (entry.2 - expected_h).abs() < 0.01,
            "card height must derive from its width, got {entry:?}"
        );
    }
}

/// A wider panel must not sprout an ever-wider content column: past the cap
/// the extra width becomes margin, exactly as in the Asset Center.
#[test]
fn content_column_stops_widening_at_the_shared_cap() {
    let wide = test_rects::wide();
    let content = PromptCenterPanel::content_rect(wide);
    assert!(
        content.size.x < wide.size.x,
        "the cap must leave margin on a 2400 px viewport"
    );
    assert_eq!(
        content.size.x,
        crate::widgets::SCENE_TEMPLATE_CONTENT_MAX_W,
        "the Prompt Center shares the Asset Center content cap"
    );
    let centre = wide.origin.x + wide.size.x / 2.0;
    assert!(
        (content.origin.x + content.size.x / 2.0 - centre).abs() < 0.01,
        "the content column must stay centred in the panel"
    );
}

/// Chrome rows have to move with the shell — a search field or a close button
/// still pinned to a 720 px-wide box would sit in the panel's top-left corner.
#[test]
fn chrome_rows_span_the_content_column() {
    let state = open_state(Locale::EnUs);
    let panel = PromptCenterPanel::for_editor(&state).expect("open panel");
    let rect = test_rects::wide();
    let content = PromptCenterPanel::content_rect(rect);

    let search = PromptCenterPanel::search_rect(rect);
    assert_eq!(search.origin.x, content.origin.x);
    assert_eq!(search.size.x, content.size.x);

    let close = PromptCenterPanel::close_rect(rect);
    assert_eq!(
        close.origin.x + close.size.x,
        content.origin.x + content.size.x
    );

    let chips = panel.filter_chip_rects(rect);
    assert_eq!(chips[0].0.origin.x, content.origin.x);
    assert!(
        chips.last().expect("filters").0.origin.x > content.origin.x,
        "the filter row must run along the content column"
    );
    assert!(
        panel.cards_viewport(rect).origin.y > chips[0].0.origin.y + chips[0].0.size.y,
        "cards start below the filter row"
    );
}

#[test]
fn body_language_follows_cjk_locale_boundary() {
    let zh = open_state(Locale::ZhCn);
    let zh_body = PromptCenterPanel::for_editor(&zh)
        .expect("open panel")
        .filtered()
        .into_iter()
        .find(|card| card.id == "gallery-wander")
        .expect("wander")
        .body
        .to_owned();
    assert!(zh_body.contains("旅行"));

    let en = open_state(Locale::EnUs);
    let en_body = PromptCenterPanel::for_editor(&en)
        .expect("open panel")
        .filtered()
        .into_iter()
        .find(|card| card.id == "gallery-wander")
        .expect("wander")
        .body
        .to_owned();
    assert!(en_body.contains("travel itinerary"));
    assert!(!en_body.contains("旅行"));

    let ja = open_state(Locale::Ja);
    let ja_body = PromptCenterPanel::for_editor(&ja)
        .expect("open panel")
        .filtered()
        .into_iter()
        .find(|card| card.id == "gallery-wander")
        .expect("wander")
        .body;
    assert!(ja_body.contains("旅行"));
}

#[test]
fn outside_inside_and_card_hits_are_distinct() {
    let state = open_state(Locale::ZhCn);
    let panel = PromptCenterPanel::for_editor(&state).expect("open panel");
    let rect = panel_rect();
    assert_eq!(
        panel.hit_test(rect, Point2D::new(rect.origin.x - 1.0, rect.origin.y)),
        None
    );
    assert_eq!(
        panel.hit_test(
            rect,
            Point2D::new(rect.origin.x + rect.size.x / 2.0, rect.origin.y + 10.0)
        ),
        Some(PromptCenterHit::Inside)
    );

    let first = panel.card_rects(rect)[0].1;
    let hit = panel
        .hit_test(
            rect,
            Point2D::new(first.origin.x + 12.0, first.origin.y + 12.0),
        )
        .expect("card hit");
    match hit {
        PromptCenterHit::SelectPrompt { id, body } => {
            assert_eq!(id, "gallery-wander");
            assert!(body.contains("旅行"));
        }
        other => panic!("unexpected hit: {other:?}"),
    }
}

#[test]
fn scrolled_grid_hits_the_card_visible_at_the_viewport_top() {
    let mut state = open_state(Locale::EnUs);
    let rect = panel_rect();
    let row_step = {
        let panel = PromptCenterPanel::for_editor(&state).expect("open panel");
        let cards = panel.card_rects(rect);
        cards[2].1.origin.y - cards[0].1.origin.y
    };
    state.editor_ui.prompt_center.scroll.offset = row_step;

    let panel = PromptCenterPanel::for_editor(&state).expect("open panel");
    let cards = panel.card_rects(rect);
    let viewport = panel.cards_viewport(rect);
    assert_eq!(cards[2].1.origin.y, viewport.origin.y);
    let expected_id = panel.filtered()[2].id.to_string();
    assert!(matches!(
        panel.hit_test(
            rect,
            Point2D::new(cards[2].1.origin.x + 12.0, cards[2].1.origin.y + 12.0),
        ),
        Some(PromptCenterHit::SelectPrompt { id, .. }) if id == expected_id
    ));
}

#[test]
fn custom_delete_has_its_own_hit_target() {
    let mut state = open_state(Locale::EnUs);
    state.editor_ui.prompt_center.install_custom_prompts(
        vec![CustomPrompt {
            id: "custom-1".to_owned(),
            title: "Reusable".to_owned(),
            body: "Reusable prompt body".to_owned(),
            category: PromptCategory::Modify,
            created_at: 1,
        }],
        true,
    );
    state.editor_ui.prompt_center.filter = PromptFilter::Custom;
    let panel = PromptCenterPanel::for_editor(&state).expect("open panel");
    let rect = panel_rect();
    let card = panel.card_rects(rect)[0].1;
    let delete = PromptCenterPanel::delete_rect(card);
    assert_eq!(
        panel.hit_test(
            rect,
            Point2D::new(
                delete.origin.x + delete.size.x / 2.0,
                delete.origin.y + delete.size.y / 2.0,
            ),
        ),
        Some(PromptCenterHit::DeleteCustom("custom-1".to_owned()))
    );
}

#[test]
fn read_only_custom_card_does_not_expose_delete() {
    let mut state = open_state(Locale::EnUs);
    state.editor_ui.prompt_center.install_custom_prompts(
        vec![CustomPrompt {
            id: "custom-1".to_owned(),
            title: "Reusable".to_owned(),
            body: "Reusable prompt body".to_owned(),
            category: PromptCategory::Modify,
            created_at: 1,
        }],
        false,
    );
    state.editor_ui.prompt_center.filter = PromptFilter::Custom;
    let panel = PromptCenterPanel::for_editor(&state).expect("open panel");
    let rect = panel_rect();
    let card = panel.card_rects(rect)[0].1;
    let delete = PromptCenterPanel::delete_rect(card);
    assert!(matches!(
        panel.hit_test(
            rect,
            Point2D::new(
                delete.origin.x + delete.size.x / 2.0,
                delete.origin.y + delete.size.y / 2.0,
            ),
        ),
        Some(PromptCenterHit::SelectPrompt { .. })
    ));
}

#[test]
fn clipped_custom_delete_does_not_steal_hover_from_panel_chrome() {
    let mut state = open_state(Locale::EnUs);
    state.editor_ui.prompt_center.install_custom_prompts(
        (0..12)
            .map(|index| CustomPrompt {
                id: format!("custom-{index}"),
                title: format!("Reusable {index}"),
                body: "Reusable prompt body".to_owned(),
                category: PromptCategory::Modify,
                created_at: index,
            })
            .collect(),
        true,
    );
    state.editor_ui.prompt_center.filter = PromptFilter::Custom;
    // The narrow fixture: the grid has to overflow its viewport for a card to
    // be scrollable up under the chrome at all.
    let rect = test_rects::narrow();
    let panel = PromptCenterPanel::for_editor(&state).expect("open panel");
    let delete = PromptCenterPanel::delete_rect(panel.card_rects(rect)[0].1);
    let search = PromptCenterPanel::search_rect(rect);
    state.editor_ui.prompt_center.scroll.offset =
        delete.origin.y + delete.size.y / 2.0 - (search.origin.y + search.size.y / 2.0);

    let panel = PromptCenterPanel::for_editor(&state).expect("open panel");
    let hidden_delete = PromptCenterPanel::delete_rect(panel.card_rects(rect)[0].1);
    let point = Point2D::new(
        hidden_delete.origin.x + hidden_delete.size.x / 2.0,
        hidden_delete.origin.y + hidden_delete.size.y / 2.0,
    );
    assert!(search.contains(point));
    assert_eq!(panel.hover_at(rect, point), None);
}

#[test]
fn max_scroll_tracks_filtered_grid_height() {
    let mut state = open_state(Locale::EnUs);
    let rect = panel_rect();
    let panel = PromptCenterPanel::for_editor(&state).expect("open panel");
    let viewport = panel.cards_viewport(rect);
    let last = panel.card_rects(rect).last().expect("catalogue card").1;
    assert_eq!(
        panel.max_scroll(rect),
        last.origin.y + last.size.y - (viewport.origin.y + viewport.size.y)
    );

    state.editor_ui.prompt_center.filter = PromptFilter::Category(PromptCategory::Starter);
    assert!(
        PromptCenterPanel::for_editor(&state)
            .expect("open panel")
            .max_scroll(rect)
            > 0.0,
        "two thumbnail rows must overflow the card viewport"
    );

    state.editor_ui.prompt_center.install_custom_prompts(
        vec![CustomPrompt {
            id: "custom-only".to_owned(),
            title: "Reusable".to_owned(),
            body: "Reusable prompt body".to_owned(),
            category: PromptCategory::Modify,
            created_at: 1,
        }],
        true,
    );
    state.editor_ui.prompt_center.filter = PromptFilter::Custom;
    assert_eq!(
        PromptCenterPanel::for_editor(&state)
            .expect("open panel")
            .max_scroll(rect),
        0.0,
        "a single thumbnail card must not scroll"
    );
}

#[test]
fn paint_decodes_only_generated_previews_visible_in_the_viewport() {
    let state = open_state(Locale::EnUs);
    let panel = PromptCenterPanel::for_editor(&state).expect("open panel");
    let expected = visible_card_count(&panel, panel_rect());
    let mut backend = CountingBackend::default();
    panel.paint(
        &mut PaintCx {
            backend: &mut backend,
        },
        panel_rect(),
    );

    assert_eq!(backend.images.len(), expected);
    assert_eq!(backend.image_modes.len(), backend.images.len());
    assert!(backend
        .image_modes
        .iter()
        .all(|mode| *mode == ImageDrawMode::Fill));
}

#[test]
fn first_paint_queues_only_visible_previews_and_uses_fallbacks() {
    let _guard = lock_decode_registry_for_tests();
    let state = open_state(Locale::EnUs);
    let panel = PromptCenterPanel::for_editor(&state).expect("open panel");
    let expected = visible_card_count(&panel, panel_rect());
    let mut backend = CountingBackend {
        image_decode_ready: Some(false),
        image_resident_ready: Some(false),
        ..Default::default()
    };
    panel.paint(
        &mut PaintCx {
            backend: &mut backend,
        },
        panel_rect(),
    );

    assert!(backend.images.is_empty());
    assert!(backend.linear_gradients >= 2);
    let pending = take_pending_decodes(usize::MAX);
    assert_eq!(pending.len(), expected);
    for entry in pending {
        mark_decode_done(entry.id);
    }
}

#[test]
fn starter_quick_actions_use_generated_previews() {
    let mut state = open_state(Locale::EnUs);
    state.editor_ui.prompt_center.filter = PromptFilter::Category(PromptCategory::Starter);
    let panel = PromptCenterPanel::for_editor(&state).expect("open panel");
    let mut backend = CountingBackend::default();
    panel.paint(
        &mut PaintCx {
            backend: &mut backend,
        },
        panel_rect(),
    );

    assert_eq!(backend.images.len(), 4);
    assert_eq!(backend.image_decode_edges.len(), 4);
    assert_eq!(backend.image_modes.len(), 4);
    assert!(backend
        .image_modes
        .iter()
        .all(|mode| *mode == ImageDrawMode::Fill));
    assert_eq!(backend.linear_gradients, 0);
}
