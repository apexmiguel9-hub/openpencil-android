//! Transcript tests — shared fixtures plus the module spine.
//!
//! The grouped test bodies live in the sibling `ai_chat_transcript_tests/`
//! directory so every file stays under the 800-line cap.

use super::*;
use op_editor_core::chat::ChatToolCall;

// This file is itself pulled in with `#[path]`, so rustc treats it like a
// `mod.rs` and resolves bare `mod` declarations as siblings. Spell out the
// sub-directory for each group instead.
#[path = "ai_chat_transcript_tests/activities.rs"]
mod activities;
#[path = "ai_chat_transcript_tests/design_blocks.rs"]
mod design_blocks;
#[path = "ai_chat_transcript_tests/layout.rs"]
mod layout;
#[path = "ai_chat_transcript_tests/narration.rs"]
mod narration;
#[path = "ai_chat_transcript_tests/paint_bubbles.rs"]
mod paint_bubbles;
#[path = "ai_chat_transcript_tests/selection.rs"]
mod selection;
#[path = "ai_chat_transcript_tests/tool_calls.rs"]
mod tool_calls;
#[path = "ai_chat_transcript_tests/tool_flow.rs"]
mod tool_flow;
#[path = "ai_chat_transcript_tests/wrapping.rs"]
mod wrapping;

fn body() -> Rect {
    Rect::xywh(0.0, 0.0, 340.0, 300.0)
}

#[allow(dead_code)] // test helper kept for future color-assertion tests
fn color_close(a: crate::Color, b: crate::Color) -> bool {
    (a.r - b.r).abs() < 1e-6
        && (a.g - b.g).abs() < 1e-6
        && (a.b - b.b).abs() < 1e-6
        && (a.a - b.a).abs() < 1e-6
}

#[derive(Default)]
struct TranscriptPaintBackend {
    round_rects: Vec<(Rect, f32)>,
    round_rect_colors: Vec<crate::Color>,
    ovals: usize,
    texts: Vec<String>,
    text_colors: Vec<(String, jian_core::scene::Color)>,
    svg_strokes: Vec<(Point2D, f32)>,
    svg_stroke_colors: Vec<(Point2D, f32, crate::Color)>,
    rotations: Vec<f32>,
}

impl crate::RenderBackend for TranscriptPaintBackend {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, _: Rect, _: crate::Color) {}
    fn stroke_rect(&mut self, _: Rect, _: crate::Color, _: f32) {}
    fn draw_text(&mut self, layout: &crate::TextLayout, _: Point2D) {
        if let Some(run) = layout.runs().first() {
            self.texts.push(run.content.clone());
            self.text_colors.push((run.content.clone(), run.color));
        }
    }
    fn clip_rect(&mut self, _: Rect) {}
    fn save(&mut self) {}
    fn restore(&mut self) {}
    fn translate(&mut self, _: Point2D) {}
    fn rotate(&mut self, radians: f32, _: Point2D) {
        self.rotations.push(radians);
    }
    fn stroke_line(&mut self, _: Point2D, _: Point2D, _: crate::Color, _: f32) {}
    fn fill_round_rect(&mut self, rect: Rect, radius: f32, color: crate::Color) {
        self.round_rects.push((rect, radius));
        self.round_rect_colors.push(color);
    }
    fn stroke_round_rect(&mut self, _: Rect, _: f32, _: crate::Color, _: f32) {}
    fn stroke_svg_path(&mut self, _: &str, point: Point2D, size: f32, color: crate::Color, _: f32) {
        self.svg_strokes.push((point, size));
        self.svg_stroke_colors.push((point, size, color));
    }
    fn fill_oval(&mut self, _: Rect, _: crate::Color) {
        self.ovals += 1;
    }
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
}
