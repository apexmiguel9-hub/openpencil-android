//! Tests for the Scene Template Center's prompt-to-deck row.
//!
//! Kept beside the panel's other tests rather than inside them: the row is a
//! separate entry point with its own visibility rules, and a reader looking
//! for "why does the generate button not paint" should land in one file.

use super::*;
use crate::widgets::press_flow::press_scene_template_center;
use op_editor_core::scene_template_catalog::TemplateScene;
use op_editor_core::EditorState;

use super::test_rects::MEDIUM as PANEL;

/// A host that can run the whole chain, which is what the row needs.
fn capable_state() -> EditorState {
    let mut state = EditorState::default();
    state.editor_ui.scene_template_generate_supported = true;
    state.editor_ui.open_scene_template_center(0);
    state
}

fn centre(rect: Rect) -> Point2D {
    Point2D::new(
        rect.origin.x + rect.size.x / 2.0,
        rect.origin.y + rect.size.y / 2.0,
    )
}

#[test]
fn the_row_paints_only_where_a_deck_is_what_the_user_asked_for() {
    let mut state = capable_state();
    for (filter, expected) in [
        (SceneFilter::All, true),
        (SceneFilter::Scene(TemplateScene::Slides), true),
        (SceneFilter::Scene(TemplateScene::Card), false),
        (SceneFilter::Scene(TemplateScene::Tutorial), false),
        (SceneFilter::Scene(TemplateScene::Carousel), false),
        (SceneFilter::Scene(TemplateScene::Comparison), false),
    ] {
        state.editor_ui.scene_template_center.filter = filter;
        let panel = SceneTemplatePanel::for_editor(&state).expect("open");
        assert_eq!(
            panel.generate_row_visible(),
            expected,
            "{filter:?} should {}show the row",
            if expected { "" } else { "not " }
        );
        assert_eq!(panel.generate_input_rect(PANEL).is_some(), expected);
        assert_eq!(panel.generate_button_rect(PANEL).is_some(), expected);
    }
}

/// A host that cannot both replace the document and launch a turn gets no
/// row — the capability bit, not the filter, is what decides that.
#[test]
fn a_host_without_the_chain_gets_no_row_at_all() {
    let mut state = EditorState::default();
    state.editor_ui.open_scene_template_center(0);
    let panel = SceneTemplatePanel::for_editor(&state).expect("open");
    assert!(!panel.generate_row_visible());
    assert_eq!(panel.generate_input_rect(PANEL), None);

    // And the grid keeps the space: an unsupported host's panel is laid out
    // exactly as it was before the row existed.
    let with_row = SceneTemplatePanel::for_editor(&capable_state())
        .expect("open")
        .cards_viewport(PANEL);
    let without_row = panel.cards_viewport(PANEL);
    assert!(without_row.size.y > with_row.size.y);
    assert_eq!(without_row.size.y - with_row.size.y, GENERATE_ROW_H);
}

#[test]
fn the_row_sits_inside_the_panel_and_above_the_grid() {
    let state = capable_state();
    let panel = SceneTemplatePanel::for_editor(&state).expect("open");
    let input = panel.generate_input_rect(PANEL).expect("visible");
    let button = panel.generate_button_rect(PANEL).expect("visible");
    let cards = panel.cards_viewport(PANEL);
    let content = SceneTemplatePanel::content_rect(PANEL);

    assert!(input.origin.x >= content.origin.x - 0.01);
    assert!(
        button.origin.x >= input.origin.x + input.size.x,
        "the button overlaps the field"
    );
    assert!(
        button.origin.x + button.size.x <= content.origin.x + content.size.x + 0.01,
        "the button runs past the content column"
    );
    assert_eq!(input.origin.y, button.origin.y);
    assert!(
        input.origin.y + input.size.y < cards.origin.y,
        "the row overlaps the card grid"
    );
    let (_, _, card_h) = panel.grid_metrics(PANEL);
    assert!(
        cards.size.y > card_h,
        "the grid must still show a full card row"
    );
}

#[test]
fn pressing_the_field_and_the_button_resolve_to_their_own_hits() {
    let state = capable_state();
    let panel = SceneTemplatePanel::for_editor(&state).expect("open");
    let input = panel.generate_input_rect(PANEL).expect("visible");
    let button = panel.generate_button_rect(PANEL).expect("visible");

    assert_eq!(
        panel.hit_test(PANEL, centre(input)),
        Some(SceneTemplateHit::FocusGenerate(0)),
        "an empty field puts the caret at 0 wherever it is clicked"
    );
    assert_eq!(
        panel.hit_test(PANEL, centre(button)),
        Some(SceneTemplateHit::Generate)
    );
    assert_eq!(
        panel.hover_at(PANEL, centre(button)),
        Some(SCENE_TEMPLATE_GENERATE_HOVER)
    );
    // The gap between the two controls is panel chrome, not a card.
    let gap = Point2D::new(
        input.origin.x + input.size.x + GENERATE_GAP / 2.0,
        input.origin.y + 2.0,
    );
    assert_eq!(panel.hit_test(PANEL, gap), Some(SceneTemplateHit::Inside));
}

/// The caret lands where the glyphs are, not where the field starts — paint
/// and hit-test share one inset so a click between two characters splits
/// them there.
#[test]
fn a_press_inside_a_typed_topic_lands_on_a_character_boundary() {
    let mut state = capable_state();
    state
        .editor_ui
        .scene_template_center
        .generate
        .set_text("abcdef");
    let panel = SceneTemplatePanel::for_editor(&state).expect("open");
    let input = panel.generate_input_rect(PANEL).expect("visible");

    let at_start = Point2D::new(input.origin.x + 2.0, input.origin.y + 4.0);
    assert_eq!(
        panel.hit_test(PANEL, at_start),
        Some(SceneTemplateHit::FocusGenerate(0))
    );
    let past_the_text = Point2D::new(input.origin.x + input.size.x - 2.0, input.origin.y + 4.0);
    assert_eq!(
        panel.hit_test(PANEL, past_the_text),
        Some(SceneTemplateHit::FocusGenerate(6)),
        "a click past the last glyph parks the caret at the end"
    );
}

/// When the row is hidden its coordinates belong to the grid again, so the
/// same point must not still resolve to a control nobody can see.
#[test]
fn a_hidden_rows_coordinates_go_back_to_the_grid() {
    let capable = capable_state();
    let button = SceneTemplatePanel::for_editor(&capable)
        .expect("open")
        .generate_button_rect(PANEL)
        .expect("visible");

    let mut state = capable_state();
    state.editor_ui.scene_template_center.filter = SceneFilter::Scene(TemplateScene::Card);
    let panel = SceneTemplatePanel::for_editor(&state).expect("open");
    assert_ne!(
        panel.hit_test(PANEL, centre(button)),
        Some(SceneTemplateHit::Generate)
    );
    assert_ne!(
        panel.hover_at(PANEL, centre(button)),
        Some(SCENE_TEMPLATE_GENERATE_HOVER),
        "the button's old coordinates must not still light it up"
    );
}

#[test]
fn focus_follows_the_field_that_was_pressed() {
    let mut state = capable_state();
    let panel_rect = PANEL;
    let (input, button) = {
        let panel = SceneTemplatePanel::for_editor(&state).expect("open");
        (
            panel.generate_input_rect(panel_rect).expect("visible"),
            panel.generate_button_rect(panel_rect).expect("visible"),
        )
    };

    assert_eq!(
        state.editor_ui.scene_template_center.focus,
        SceneTemplateFocus::Search,
        "the panel opens with the search field focused"
    );
    press_scene_template_center(&mut state, panel_rect, centre(input), 1).expect("inside");
    assert_eq!(
        state.editor_ui.scene_template_center.focus,
        SceneTemplateFocus::Generate
    );

    let search = SceneTemplatePanel::search_rect(panel_rect);
    press_scene_template_center(&mut state, panel_rect, centre(search), 2).expect("inside");
    assert_eq!(
        state.editor_ui.scene_template_center.focus,
        SceneTemplateFocus::Search
    );

    // Pressing the button does not steal the caret into a field the press
    // was not aimed at.
    press_scene_template_center(&mut state, panel_rect, centre(button), 3).expect("inside");
    assert_eq!(
        state.editor_ui.scene_template_center.focus,
        SceneTemplateFocus::Search
    );
}

#[test]
fn an_empty_topic_submits_nothing_and_keeps_the_panel_open() {
    let mut state = capable_state();
    let button = SceneTemplatePanel::for_editor(&state)
        .expect("open")
        .generate_button_rect(PANEL)
        .expect("visible");
    // Whitespace is not a topic either.
    state
        .editor_ui
        .scene_template_center
        .generate
        .set_text("   ");

    let changed =
        press_scene_template_center(&mut state, PANEL, centre(button), 1).expect("inside");

    assert_eq!(state.editor_ui.scene_template_center.pending_generate, None);
    assert!(
        state.editor_ui.scene_template_center.open,
        "an empty submit must not dismiss the panel"
    );
    // The press still registered as pressed-button feedback, so the panel
    // repaints — but nothing else moved.
    assert!(changed);
    assert_eq!(state.editor_ui.scene_template_center.generate.text(), "   ");
}

#[test]
fn submitting_a_topic_raises_a_request_and_closes_the_panel() {
    let mut state = capable_state();
    let button = SceneTemplatePanel::for_editor(&state)
        .expect("open")
        .generate_button_rect(PANEL)
        .expect("visible");
    state
        .editor_ui
        .scene_template_center
        .generate
        .set_text("  Q3 复盘  ");

    assert!(press_scene_template_center(&mut state, PANEL, centre(button), 1).expect("inside"));

    let center = &mut state.editor_ui.scene_template_center;
    assert!(!center.open, "submitting dismisses the panel");
    assert_eq!(
        center.take_pending_generate().as_deref(),
        Some("Q3 复盘"),
        "the raw topic is handed over trimmed"
    );
    assert!(
        center.generate.text().is_empty(),
        "the field is cleared so reopening does not re-offer a spent topic"
    );
    assert_eq!(center.focus, SceneTemplateFocus::Search);
}

/// Changing the filter can hide a focused topic field. The caret has to go
/// somewhere visible, or the panel paints with no focus at all and the next
/// keystroke lands in an input the user cannot see.
#[test]
fn hiding_the_row_moves_the_painted_caret_back_to_search() {
    let mut state = capable_state();
    state.editor_ui.scene_template_center.focus = SceneTemplateFocus::Generate;
    let panel = SceneTemplatePanel::for_editor(&state).expect("open");
    assert!(panel.field_focused(SceneTemplateFocus::Generate));
    assert!(!panel.field_focused(SceneTemplateFocus::Search));

    state.editor_ui.scene_template_center.filter = SceneFilter::Scene(TemplateScene::Card);
    let panel = SceneTemplatePanel::for_editor(&state).expect("open");
    assert!(!panel.field_focused(SceneTemplateFocus::Generate));
    assert!(panel.field_focused(SceneTemplateFocus::Search));
}
