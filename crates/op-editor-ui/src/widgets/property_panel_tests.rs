//! Tests for `widgets::property_panel` — moved to a sibling file to
//! keep `property_panel.rs` under the 800-line cap.
//!
//! Phase 6: the panel builds from `op_editor_core::EditorState`, so
//! the fixtures construct `EditorState` values.
//!
//! This spine now holds only the local paint-recording backend + the
//! colour comparator; the cases themselves live in the sibling
//! `property_panel_tests/` submodules so every file stays under the
//! openpencil 800-line cap. Shared fixtures (`state_from`,
//! `visible_for`, `CountingBackend`) still come from
//! `property_panel_test_support`.

use crate::{Color, Point2D, Rect, TextLayout};

mod menus;
mod pickers;
mod selection;
mod shape_sections;
mod sizing;

#[derive(Default)]
struct RoundFillBackend {
    fills: Vec<(Rect, Color)>,
}

impl crate::RenderBackend for RoundFillBackend {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, _: Rect, _: Color) {}
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn draw_text(&mut self, _: &TextLayout, _: Point2D) {}
    fn clip_rect(&mut self, _: Rect) {}
    fn stroke_line(&mut self, _: Point2D, _: Point2D, _: Color, _: f32) {}
    fn fill_round_rect(&mut self, rect: Rect, _: f32, color: Color) {
        self.fills.push((rect, color));
    }
    fn stroke_round_rect(&mut self, _: Rect, _: f32, _: Color, _: f32) {}
    fn stroke_svg_path(&mut self, _: &str, _: Point2D, _: f32, _: Color, _: f32) {}
    fn save(&mut self) {}
    fn restore(&mut self) {}
    fn translate(&mut self, _: Point2D) {}
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
}

fn color_eq(a: Color, b: Color) -> bool {
    (a.r - b.r).abs() < 0.001
        && (a.g - b.g).abs() < 0.001
        && (a.b - b.b).abs() < 0.001
        && (a.a - b.a).abs() < 0.001
}

// ④ fit-content hover-wash tests (`action_wash_rect`) live in the sibling
// `property_panel_wash_tests.rs` to keep this file under the 800-line cap.
