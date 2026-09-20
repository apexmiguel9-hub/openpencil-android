//! Shared fixtures for the [`super::super::AIChatPlaceholder`] test
//! siblings — layout landmarks, model seeding, and the recording
//! `RenderBackend` used by the paint assertions. Carved off
//! `ai_chat_panel/tests.rs` to keep every file under the 800-line cap.

use super::super::*;

/// Y-coordinate of the textarea's vertical center.
pub(in crate::widgets) fn textarea_center_y() -> f32 {
    AI_CHAT_HEIGHT - INPUT_BASE_HEIGHT + 1.0 + INPUT_AREA_HEIGHT / 2.0
}

/// Y-coordinate of the bottom toolbar's vertical center.
pub(in crate::widgets) fn toolbar_center_y() -> f32 {
    AI_CHAT_HEIGHT - INPUT_BASE_HEIGHT + 1.0 + INPUT_AREA_HEIGHT + INPUT_TOOLBAR_HEIGHT / 2.0
}

pub(in crate::widgets) fn seed_available_model(s: &mut EditorState) {
    s.chat
        .available_models
        .push(op_editor_core::chat::ModelEntry::new(
            op_editor_core::chat::AgentProvider::CodexCli,
            "gpt-5",
            "GPT-5",
        ));
}

// Shared paint-assertion infrastructure — also used by the
// sibling `tests_transcript` module (split at the 800-line cap).
#[derive(Default)]
pub(in crate::widgets) struct PanelPaintBackend {
    pub(in crate::widgets) fills: Vec<(Rect, crate::Color)>,
    pub(in crate::widgets) round_rects: Vec<(Rect, f32, crate::Color)>,
    pub(in crate::widgets) texts: Vec<(String, f32, jian_core::scene::Color, Point2D)>,
    pub(in crate::widgets) svg_paths: Vec<String>,
    pub(in crate::widgets) svg_strokes: Vec<(Point2D, f32, crate::Color, f32)>,
    pub(in crate::widgets) stroke_lines: usize,
}

impl crate::RenderBackend for PanelPaintBackend {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, rect: Rect, color: crate::Color) {
        self.fills.push((rect, color));
    }
    fn stroke_rect(&mut self, _: Rect, _: crate::Color, _: f32) {}
    fn draw_text(&mut self, layout: &crate::TextLayout, origin: Point2D) {
        if let Some(run) = layout.runs().first() {
            self.texts
                .push((run.content.clone(), run.font_size, run.color, origin));
        }
    }
    fn clip_rect(&mut self, _: Rect) {}
    fn save(&mut self) {}
    fn restore(&mut self) {}
    fn translate(&mut self, _: Point2D) {}
    fn stroke_line(&mut self, _: Point2D, _: Point2D, _: crate::Color, _: f32) {
        self.stroke_lines += 1;
    }
    fn fill_round_rect(&mut self, rect: Rect, radius: f32, color: crate::Color) {
        self.round_rects.push((rect, radius, color));
    }
    fn stroke_round_rect(&mut self, _: Rect, _: f32, _: crate::Color, _: f32) {}
    fn stroke_svg_path(
        &mut self,
        d: &str,
        top_left: Point2D,
        size: f32,
        color: crate::Color,
        width: f32,
    ) {
        self.svg_paths.push(d.to_string());
        self.svg_strokes.push((top_left, size, color, width));
    }
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
}

pub(in crate::widgets) fn has_fill_rect(fills: &[(Rect, crate::Color)], expected: Rect) -> bool {
    fills.iter().any(|(rect, _)| {
        (rect.origin.x - expected.origin.x).abs() < 1e-4
            && (rect.origin.y - expected.origin.y).abs() < 1e-4
            && (rect.size.x - expected.size.x).abs() < 1e-4
            && (rect.size.y - expected.size.y).abs() < 1e-4
    })
}
