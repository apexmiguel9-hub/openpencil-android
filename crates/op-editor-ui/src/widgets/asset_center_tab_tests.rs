//! Asset Center tab row + Styles tab geometry, hit-testing, and pinning.

use super::asset_center_style_cards::style_test_support::exclusive_user_styles;
use super::scene_template_panel::test_rects::MEDIUM as PANEL;
use super::scene_template_panel::{SceneTemplatePanel, STYLE_CARD_H};
use super::SceneTemplateHit;
use crate::{Point2D, Rect};
use op_editor_core::{AssetCenterTab, EditorState};
use std::sync::MutexGuard;

fn open_state() -> EditorState {
    let mut state = EditorState::default();
    state.editor_ui.open_scene_template_center(0);
    state
}

/// The Styles tab, with the process-global imported-style registry emptied
/// and held for the duration of the test.
///
/// The guard is returned rather than dropped here on purpose: the Styles grid
/// reads a registry shared by every test in the binary, and cargo runs them in
/// parallel threads of one process. A test that imported two guides would
/// otherwise change what an unrelated test's grid contains — which it did,
/// before this existed.
fn styles_state() -> (MutexGuard<'static, ()>, EditorState) {
    let guard = exclusive_user_styles();
    let mut state = open_state();
    state.editor_ui.scene_template_center.tab = AssetCenterTab::Styles;
    (guard, state)
}

fn press(state: &mut EditorState, point: Point2D) -> Option<bool> {
    crate::widgets::scene_template_press_flow::press_scene_template_center(state, PANEL, point, 0)
}

fn centre(rect: Rect) -> Point2D {
    Point2D::new(
        rect.origin.x + rect.size.x / 2.0,
        rect.origin.y + rect.size.y / 2.0,
    )
}

#[test]
fn the_top_bar_palette_button_opens_the_panel() {
    use crate::widgets::{TopBar, TopBarHit, TOP_BAR_HEIGHT};

    let bar = TopBar::new("Untitled");
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(1000.0, TOP_BAR_HEIGHT),
    };
    let button = bar.asset_center_button_rect(rect);
    assert_eq!(
        bar.hit_test(rect, centre(button)),
        Some(TopBarHit::OpenAssetCenter)
    );

    let mut state = EditorState::default();
    assert!(!state.editor_ui.scene_template_center.open);
    crate::widgets::press_flow::apply_shared_top_bar_hit(&mut state, TopBarHit::OpenAssetCenter, 0);
    assert!(state.editor_ui.scene_template_center.open);
}

/// The panel covers the middle of the window, so any dropdown left open
/// would float over a grid the user can no longer reach past. This is the
/// same dismissal set the export button applies.
#[test]
fn opening_the_asset_center_dismisses_the_other_top_bar_menus() {
    use crate::widgets::TopBarHit;

    let mut state = EditorState::default();
    state.editor_ui.file_menu_open = true;
    state.editor_ui.export_quick_menu_open = true;
    state.editor_ui.import_menu_open = true;
    state.editor_ui.account_menu_open = true;
    state.editor_ui.locale_picker.open = true;
    state.editor_ui.prompt_center.open = true;

    crate::widgets::press_flow::apply_shared_top_bar_hit(&mut state, TopBarHit::OpenAssetCenter, 0);

    assert!(state.editor_ui.scene_template_center.open);
    assert!(!state.editor_ui.file_menu_open);
    assert!(!state.editor_ui.export_quick_menu_open);
    assert!(!state.editor_ui.import_menu_open);
    assert!(!state.editor_ui.account_menu_open);
    assert!(!state.editor_ui.locale_picker.open);
    assert!(
        !state.editor_ui.prompt_center.open,
        "two full-size centred panels cannot both be open"
    );
}

#[test]
fn the_panel_opens_on_the_templates_tab() {
    let state = open_state();
    assert_eq!(
        state.editor_ui.scene_template_center.tab,
        AssetCenterTab::Templates
    );
}

#[test]
fn every_tab_chip_is_reachable_and_none_overlap() {
    let state = open_state();
    let panel = SceneTemplatePanel::for_editor(&state).expect("open");
    let chips = panel.tab_chip_rects(PANEL);
    assert_eq!(chips.len(), AssetCenterTab::ALL.len());

    for pair in chips.windows(2) {
        let (left, _) = pair[0];
        let (right, _) = pair[1];
        assert!(
            left.origin.x + left.size.x <= right.origin.x,
            "tab chips overlap: {left:?} then {right:?}"
        );
    }
    for (rect, tab) in chips {
        assert_eq!(
            panel.hit_test(PANEL, centre(rect)),
            Some(SceneTemplateHit::SelectTab(tab))
        );
    }
}

/// One line, no wrap: the row has to fit in the locale with the longest
/// labels, not just the default one.
#[test]
fn the_tab_row_fits_the_panel_in_every_locale() {
    for locale in op_editor_core::Locale::ALL {
        let mut state = open_state();
        state.editor_ui.locale = locale;
        let panel = SceneTemplatePanel::for_editor(&state).expect("open");
        let (last, _) = panel.tab_chip_rects(PANEL).last().copied().expect("chips");
        assert!(
            last.origin.x + last.size.x <= PANEL.origin.x + PANEL.size.x,
            "{locale:?}: the tab row runs past the panel edge"
        );
    }
}

#[test]
fn the_tab_row_sits_above_the_search_field_without_overlapping_it() {
    let state = open_state();
    let panel = SceneTemplatePanel::for_editor(&state).expect("open");
    let (chip, _) = panel.tab_chip_rects(PANEL)[0];
    let search = SceneTemplatePanel::search_rect(PANEL);
    assert!(
        chip.origin.y + chip.size.y <= search.origin.y,
        "tab chip {chip:?} overlaps the search field {search:?}"
    );
}

#[test]
fn switching_tabs_swaps_the_grid_contents() {
    let templates = open_state();
    let template_panel = SceneTemplatePanel::for_editor(&templates).expect("open");
    let template_cards = template_panel.card_rects(PANEL).len();

    let (_guard, styles) = styles_state();
    let style_panel = SceneTemplatePanel::for_editor(&styles).expect("open");
    let style_cards = style_panel.card_rects(PANEL).len();

    assert_eq!(template_cards, template_panel.filtered().len());
    assert_eq!(style_cards, style_panel.style_cards().len());
    assert_ne!(
        template_cards, style_cards,
        "the two catalogues are different sizes; equal counts means one tab is showing the other's grid"
    );
}

#[test]
fn the_scene_filter_row_belongs_to_the_templates_tab_only() {
    let templates = open_state();
    assert!(!SceneTemplatePanel::for_editor(&templates)
        .expect("open")
        .filter_chip_rects(PANEL)
        .is_empty());

    let (_guard, styles) = styles_state();
    let panel = SceneTemplatePanel::for_editor(&styles).expect("open");
    assert!(
        panel.filter_chip_rects(PANEL).is_empty(),
        "the style catalogue has tags, not scenes"
    );
    // The vacated row must be reclaimed, not left as a gap.
    let templates_top = SceneTemplatePanel::for_editor(&templates)
        .expect("open")
        .cards_viewport(PANEL)
        .origin
        .y;
    assert!(
        panel.cards_viewport(PANEL).origin.y < templates_top,
        "the style grid must start where the filter row would have been"
    );
}

/// Style cards share the template grid's column count — a card is a card, and
/// two tabs disagreeing about how wide one is would jump the eye on a switch.
/// Their height does not follow, because they hold no picture to scale.
#[test]
fn style_cards_share_the_grid_columns_and_keep_their_own_height() {
    let (_guard, state) = styles_state();
    let panel = SceneTemplatePanel::for_editor(&state).expect("open");
    let viewport = panel.cards_viewport(PANEL);
    let (columns, card_w, _) = panel.grid_metrics(PANEL);
    let rects = panel.card_rects(PANEL);
    assert!(
        rects.len() > 20,
        "the shipped style corpus is not this small"
    );

    let templates_state = open_state();
    let templates = SceneTemplatePanel::for_editor(&templates_state).expect("open");
    let (template_columns, template_card_w, _) = templates.grid_metrics(PANEL);
    assert_eq!(
        (columns, card_w),
        (template_columns, template_card_w),
        "the two tabs must lay cards out on the same column grid"
    );

    let (_, first) = rects[0];
    let (_, second) = rects[1];
    assert_eq!(first.origin.y, second.origin.y, "first two share a row");
    assert!(second.origin.x > first.origin.x);
    assert_eq!(first.size.y, STYLE_CARD_H);
    assert!(
        rects[columns].1.origin.y > first.origin.y,
        "the card past the last column wraps to the next row"
    );

    for (_, rect) in &rects {
        assert!(rect.origin.x >= viewport.origin.x - 0.5);
        assert!(
            rect.origin.x + rect.size.x <= viewport.origin.x + viewport.size.x + 0.5,
            "style card {rect:?} runs past the viewport"
        );
    }
}

#[test]
fn the_swatch_band_stays_inside_its_card() {
    let (_guard, state) = styles_state();
    let panel = SceneTemplatePanel::for_editor(&state).expect("open");
    let (_, card) = panel.card_rects(PANEL)[0];
    let band = SceneTemplatePanel::swatch_band_rect(card);
    assert!(band.origin.x >= card.origin.x);
    assert!(band.origin.y >= card.origin.y);
    assert!(band.origin.x + band.size.x <= card.origin.x + card.size.x);
    assert!(band.origin.y + band.size.y <= card.origin.y + card.size.y);
}

#[test]
fn a_press_on_a_style_card_resolves_to_that_guide() {
    let (_guard, state) = styles_state();
    let panel = SceneTemplatePanel::for_editor(&state).expect("open");
    let cards = panel.style_cards();
    let (index, rect) = panel.card_rects(PANEL).into_iter().next().expect("a card");
    assert_eq!(
        panel.hit_test(PANEL, centre(rect)),
        Some(SceneTemplateHit::ToggleStyleGuide(cards[index].id.clone()))
    );
}

#[test]
fn pressing_a_style_card_pins_it_and_pressing_it_again_unpins() {
    let (_guard, mut state) = styles_state();
    let name = {
        let panel = SceneTemplatePanel::for_editor(&state).expect("open");
        panel.style_cards()[0].name.to_string()
    };
    let card = {
        let panel = SceneTemplatePanel::for_editor(&state).expect("open");
        panel.card_rects(PANEL)[0].1
    };

    assert_eq!(state.editor_ui.pinned_style_guide, None);
    press(&mut state, centre(card));
    assert_eq!(
        state.editor_ui.pinned_style_guide.as_deref(),
        Some(name.as_str())
    );
    press(&mut state, centre(card));
    assert_eq!(state.editor_ui.pinned_style_guide, None);
}

#[test]
fn pinning_a_second_guide_replaces_the_first() {
    // At most one pin at a time: two pinned guides would leave the generation
    // path arbitrating between them, and the panel showing two selected cards.
    let (_guard, mut state) = styles_state();
    let (first_card, second_card, first_name, second_name) = {
        let panel = SceneTemplatePanel::for_editor(&state).expect("open");
        let rects = panel.card_rects(PANEL);
        let cards = panel.style_cards();
        (
            rects[0].1,
            rects[1].1,
            cards[0].name.to_string(),
            cards[1].name.to_string(),
        )
    };

    press(&mut state, centre(first_card));
    assert_eq!(
        state.editor_ui.pinned_style_guide.as_deref(),
        Some(first_name.as_str())
    );
    press(&mut state, centre(second_card));
    assert_eq!(
        state.editor_ui.pinned_style_guide.as_deref(),
        Some(second_name.as_str())
    );

    let panel = SceneTemplatePanel::for_editor(&state).expect("open");
    let pinned: Vec<String> = panel
        .style_cards()
        .iter()
        .filter(|card| card.is_pinned(panel.pinned_style_guide()))
        .map(|card| card.name.clone())
        .collect();
    assert_eq!(pinned, vec![second_name.as_str()]);
}

#[test]
fn pressing_a_tab_chip_switches_tabs_and_drops_stale_scroll_and_hover() {
    let (_guard, mut state) = styles_state();
    state.editor_ui.scene_template_center.scroll.offset = 120.0;
    state.editor_ui.scene_template_center.hover = Some(7);

    let templates_chip = {
        let panel = SceneTemplatePanel::for_editor(&state).expect("open");
        panel
            .tab_chip_rects(PANEL)
            .into_iter()
            .find(|(_, tab)| *tab == AssetCenterTab::Templates)
            .expect("a templates chip")
            .0
    };
    press(&mut state, centre(templates_chip));

    let center = &state.editor_ui.scene_template_center;
    assert_eq!(center.tab, AssetCenterTab::Templates);
    assert_eq!(center.scroll.offset, 0.0);
    // A hover index points into the grid that just went away; the press also
    // sets its own hover, so what matters is that it is not the stale 7.
    assert_ne!(center.hover, Some(7));
}

#[test]
fn re_pressing_the_active_tab_leaves_the_grid_where_it_was() {
    let (_guard, mut state) = styles_state();
    state.editor_ui.scene_template_center.scroll.offset = 90.0;
    let styles_chip = {
        let panel = SceneTemplatePanel::for_editor(&state).expect("open");
        panel
            .tab_chip_rects(PANEL)
            .into_iter()
            .find(|(_, tab)| *tab == AssetCenterTab::Styles)
            .expect("a styles chip")
            .0
    };
    press(&mut state, centre(styles_chip));

    assert_eq!(
        state.editor_ui.scene_template_center.scroll.offset, 90.0,
        "re-selecting the tab you are on must not scroll you to the top"
    );
}

#[test]
fn the_generate_row_is_offered_on_the_styles_tab() {
    // Picking a style and typing a topic is the tab's whole point; without
    // the row the user has to close the panel to use what they just pinned.
    let (_guard, mut state) = styles_state();
    state.editor_ui.scene_template_generate_supported = true;
    let panel = SceneTemplatePanel::for_editor(&state).expect("open");
    assert!(panel.generate_row_visible());
    assert!(panel.generate_input_rect(PANEL).is_some());

    // Still honest about host capability. Built inline rather than through
    // `styles_state` — the registry lock is already held by this test and is
    // not reentrant.
    let mut unsupported = open_state();
    unsupported.editor_ui.scene_template_center.tab = AssetCenterTab::Styles;
    unsupported.editor_ui.scene_template_generate_supported = false;
    assert!(!SceneTemplatePanel::for_editor(&unsupported)
        .expect("open")
        .generate_row_visible());
}

/// The colour band is the only thing distinguishing two style cards until
/// previews are baked, so it has to be painted with the guide's own colours
/// at the geometry the card claims — not a placeholder, and not off-card.
#[test]
fn the_swatch_band_paints_the_guides_own_colours() {
    use crate::widgets::test_capture_backend::CaptureBackend;
    use crate::widgets::PaintCx;

    let (_guard, state) = styles_state();
    let panel = SceneTemplatePanel::for_editor(&state).expect("open");
    let first = panel.style_cards().remove(0);
    let (_, card) = panel.card_rects(PANEL)[0];
    let band = SceneTemplatePanel::swatch_band_rect(card);

    let mut backend = CaptureBackend::default();
    panel.paint(
        &mut PaintCx {
            backend: &mut backend,
        },
        PANEL,
    );

    let swatches: Vec<_> = backend
        .rect_fills
        .iter()
        // Both cards in the row share a band y, so bound by the first
        // card's columns too.
        .filter(|(rect, _)| {
            (rect.origin.y - band.origin.y).abs() < 0.5
                && rect.origin.x >= band.origin.x - 0.5
                && rect.origin.x + rect.size.x <= band.origin.x + band.size.x + 0.5
        })
        .collect();
    assert_eq!(
        swatches.len(),
        first.swatches.len(),
        "one painted swatch per parsed palette colour"
    );
    for (index, (rect, color)) in swatches.iter().enumerate() {
        assert_eq!(*color, first.swatches[index], "swatch {index} colour");
        assert_eq!(rect.size.y, band.size.y);
        assert!(rect.origin.x >= band.origin.x - 0.5);
        assert!(rect.origin.x + rect.size.x <= band.origin.x + band.size.x + 0.5);
    }
}

#[test]
fn search_narrows_the_style_grid() {
    let (_guard, mut state) = styles_state();
    state
        .editor_ui
        .scene_template_center
        .search
        .set_text("terminal");
    let panel = SceneTemplatePanel::for_editor(&state).expect("open");
    let hits = panel.style_cards();
    assert!(!hits.is_empty());
    assert!(hits.len() < super::asset_center_style_cards::style_guide_cards().len());

    state
        .editor_ui
        .scene_template_center
        .search
        .set_text("no-such-style-anywhere");
    let panel = SceneTemplatePanel::for_editor(&state).expect("open");
    assert!(panel.style_cards().is_empty());
    assert_eq!(panel.max_scroll(PANEL), 0.0, "an empty grid cannot scroll");
}

// ─── Importing a DESIGN.md ─────────────────────────────────────────────
