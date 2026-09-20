//! Density contract for the settings modal.
//!
//! The traversal audit in `agent_settings_row_layout_tests` answers "does
//! any line escape its box". These answer the complaint that produced
//! this pass: the modal was legible and still read as too big. Three
//! things have to hold at once, and each has an upper bound so a future
//! "let me just add one more line here" trips instead of shipping.
//!
//! 1. The block above the first setting is a heading, not a hero.
//! 2. A two-line row is tight but its two lines' INK never touches —
//!    the failure mode of every past squeeze.
//! 3. The scroll range ends below the last row, not at it.

use crate::widgets::agent_settings_metrics::{
    CONTENT_TAIL_PAD, ROW_H_ONE_LINE, ROW_H_TWO_LINE, SECTION_GAP,
};
use crate::widgets::agent_settings_panel::{
    content_viewport, AgentSettingsPanel, AGENTS_HERO_HEIGHT,
};
use crate::widgets::agent_settings_panel_geometry::agent_card_rect_in;
use crate::widgets::agent_settings_rows::{
    row_height, tab_intro_height, RowLines, ASCENT_RATIO, DESCENT_RATIO, ROW_DESC_FONT,
    ROW_LABEL_BASELINE, ROW_LABEL_FONT, ROW_SECOND_LINE_BASELINE,
};
use op_editor_core::agent_settings::{AgentProvider, AgentSettings, AgentSettingsTab};
use op_editor_core::EditorState;

/// Ceiling for a tab's opening block. The shipped hero was 96 px — a
/// title plus two muted lines, roughly a quarter of the modal's fold
/// spent before the first setting. A title plus one muted line is the
/// most this may ever be again.
const TAB_INTRO_CEILING: f32 = 52.0;

/// Ceilings for the two row shapes. Named separately from the metrics
/// themselves so tightening the scale is a deliberate edit here too, and
/// so "Pen is about 48-50 for a single row" is written down somewhere the
/// next person reads.
const ONE_LINE_ROW_CEILING: f32 = 48.0;
const TWO_LINE_ROW_CEILING: f32 = 56.0;

#[test]
fn a_tab_opens_with_a_heading_not_a_hero() {
    // Bound through locals: these are compile-time constants, and an
    // `assert!` over a const expression is a lint, not a test.
    let intro = AGENTS_HERO_HEIGHT;
    let ceiling = TAB_INTRO_CEILING;
    assert!(
        intro <= ceiling,
        "the Agents tab spends {AGENTS_HERO_HEIGHT}px before its first section — \
         over the {TAB_INTRO_CEILING}px a title plus one muted line costs, which \
         means a headline or a second blurb line crept back in"
    );
    assert!(tab_intro_height(false) < tab_intro_height(true));
    assert!(tab_intro_height(true) <= TAB_INTRO_CEILING);
}

#[test]
fn list_rows_stay_inside_the_density_ceilings() {
    assert!(
        row_height(RowLines::One) <= ONE_LINE_ROW_CEILING,
        "a one-line row is {}px",
        row_height(RowLines::One)
    );
    assert!(
        row_height(RowLines::Two) <= TWO_LINE_ROW_CEILING,
        "a two-line row is {}px",
        row_height(RowLines::Two)
    );
    // Compact is not the same as cramped: a row still has to be a
    // comfortable pointer target.
    assert!(row_height(RowLines::One) >= 40.0);
    assert!(row_height(RowLines::Two) >= 50.0);
    assert_eq!(row_height(RowLines::One), ROW_H_ONE_LINE);
    assert_eq!(row_height(RowLines::Two), ROW_H_TWO_LINE);
}

#[test]
fn the_two_lines_of_a_two_line_row_never_share_ink() {
    // Computed the way the row box itself is: baseline ± the nominal
    // ascent/descent ratios the geometry is built from. Injecting an old
    // baseline pair here turns this red, which is the point — the box got
    // 8 px shorter in this pass and the baselines had to move with it.
    let label_ink_top = ROW_LABEL_BASELINE - ROW_LABEL_FONT * ASCENT_RATIO;
    let label_ink_bottom = ROW_LABEL_BASELINE + ROW_LABEL_FONT * DESCENT_RATIO;
    let desc_ink_top = ROW_SECOND_LINE_BASELINE - ROW_DESC_FONT * ASCENT_RATIO;
    let desc_ink_bottom = ROW_SECOND_LINE_BASELINE + ROW_DESC_FONT * DESCENT_RATIO;
    assert!(
        desc_ink_top > label_ink_bottom,
        "the label's descender reaches {label_ink_bottom} and the description's \
         ascender starts at {desc_ink_top} — they overlap, so the row reads as one blob"
    );
    // The whole stack lives inside the box, top ascender to bottom
    // descender.
    assert!(
        label_ink_top >= 0.0,
        "the label's ascender rises out of the row box"
    );
    assert!(
        desc_ink_bottom <= row_height(RowLines::Two),
        "the description's descender falls out of the row box"
    );
}

#[test]
fn the_last_provider_row_keeps_a_bottom_margin_inside_the_scroll_range() {
    // At full scroll the last row must not sit flush against the modal's
    // bottom edge. Equivalent statement, and the one that does not need a
    // scroll simulation: the reported content height reaches past the last
    // row by at least one tail pad.
    for connected in [false, true] {
        let mut state = EditorState::default();
        state.editor_ui.agent_settings.tab = AgentSettingsTab::Agents;
        if connected {
            // The Claude row grows a hint line when it is connected, and
            // the height walk has to step over that too.
            state.editor_ui.agent_settings.connected[0] = true;
            state.editor_ui.agent_settings.provider_connection[0].phase =
                op_editor_core::agent_settings::ProviderConnectPhase::Connected;
        }
        let panel = AgentSettingsPanel::for_editor(&state);
        let rect = panel.rect(1400.0, 1000.0);
        let content = content_viewport(rect);
        let settings: AgentSettings = state.editor_ui.agent_settings.clone();

        let last = agent_card_rect_in(rect, AgentProvider::ALL.len() - 1, &settings);
        let used = last.origin.y + last.size.y - content.origin.y;
        let total = panel.content_total_height();

        assert!(
            total >= used + CONTENT_TAIL_PAD - 0.01,
            "connected={connected}: the scroll range ends {}px past the last provider row, \
             under the {CONTENT_TAIL_PAD}px bottom margin every tab reserves",
            total - used
        );
        // …and it must not overshoot either, or the tab scrolls into
        // empty space below its own content.
        assert!(
            total <= used + CONTENT_TAIL_PAD + SECTION_GAP,
            "connected={connected}: the scroll range runs {}px past the last row — \
             the height walk and the row ladder have drifted apart",
            total - used
        );
    }
}

#[test]
fn the_provider_row_ladder_agrees_with_what_paint_walks() {
    // Paint steps: ACP section, one section gap, one section header, then
    // rows. The hit-test ladder used to step 32 px for that header while
    // paint stepped 28, so every provider row's click target sat four
    // pixels below the row you could see — invisible in isolation, and
    // exactly the kind of thing only a paint-vs-geometry comparison finds.
    //
    // Each provider row but the last strokes a hairline along its own
    // bottom edge, so those strokes are where paint thinks the rows are.
    let mut state = EditorState::default();
    state.editor_ui.agent_settings.tab = AgentSettingsTab::Agents;
    let panel = AgentSettingsPanel::for_editor(&state);
    let rect = panel.rect(1400.0, 1000.0);
    let settings: AgentSettings = state.editor_ui.agent_settings.clone();
    let mut backend = LineCapture::default();
    {
        let mut cx = crate::widgets::PaintCx {
            backend: &mut backend,
        };
        crate::widgets::Widget::paint(&panel, &mut cx, rect);
    }

    let first = agent_card_rect_in(rect, 0, &settings);
    let second = agent_card_rect_in(rect, 1, &settings);
    assert_eq!(
        second.origin.y - first.origin.y,
        ROW_H_TWO_LINE,
        "provider rows must tile flush — the hairline between them is the gap"
    );

    for i in 0..AgentProvider::ALL.len() - 1 {
        let row = agent_card_rect_in(rect, i, &settings);
        let bottom = row.origin.y + row.size.y;
        assert!(
            backend
                .lines
                .iter()
                .any(|(from, to)| (from.y - bottom).abs() < 0.01
                    && (from.x - row.origin.x).abs() < 0.01
                    && (to.x - (row.origin.x + row.size.x)).abs() < 0.01),
            "no separator painted at provider row {i}'s bottom edge ({bottom}) — \
             paint and the row ladder disagree about where the rows are"
        );
    }
}

/// Records the hairlines the modal strokes, so a test can ask where paint
/// actually put a row instead of trusting the same ladder twice.
#[derive(Default)]
struct LineCapture {
    lines: Vec<(crate::Point2D, crate::Point2D)>,
}

impl crate::RenderBackend for LineCapture {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, _: crate::Rect, _: crate::Color) {}
    fn stroke_rect(&mut self, _: crate::Rect, _: crate::Color, _: f32) {}
    fn draw_text(&mut self, _: &crate::TextLayout, _: crate::Point2D) {}
    fn clip_rect(&mut self, _: crate::Rect) {}
    fn stroke_line(&mut self, from: crate::Point2D, to: crate::Point2D, _: crate::Color, _: f32) {
        self.lines.push((from, to));
    }
    fn fill_round_rect(&mut self, _: crate::Rect, _: f32, _: crate::Color) {}
    fn stroke_round_rect(&mut self, _: crate::Rect, _: f32, _: crate::Color, _: f32) {}
    fn stroke_svg_path(&mut self, _: &str, _: crate::Point2D, _: f32, _: crate::Color, _: f32) {}
    fn save(&mut self) {}
    fn restore(&mut self) {}
    fn translate(&mut self, _: crate::Point2D) {}
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
    fn measure_text(&mut self, text: &str, font_size: f32) -> f32 {
        text.chars().count() as f32 * font_size * 0.55
    }
}
