//! Shared fixtures for the variables-panel test siblings — the
//! recording `RenderBackend`, colour / caret assertions, and the two
//! seeded documents. Carved off `variables_panel/tests.rs` to keep
//! every file under the 800-line cap.

use super::super::*;
use crate::Color;
use jian_ops_schema::variable::VariableScalar;

#[derive(Default)]
pub(super) struct TextCaptureBackend {
    pub(super) texts: Vec<String>,
    pub(super) origins: Vec<Point2D>,
    pub(super) fills: Vec<(Rect, Color)>,
    pub(super) round_fills: Vec<(Rect, f32, Color)>,
    pub(super) round_strokes: Vec<(Rect, f32, Color, f32)>,
    pub(super) svg_origins: Vec<Point2D>,
    pub(super) svg_sizes: Vec<f32>,
}

impl crate::RenderBackend for TextCaptureBackend {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, rect: Rect, color: Color) {
        self.fills.push((rect, color));
    }
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn draw_text(&mut self, layout: &crate::TextLayout, origin: Point2D) {
        if let Some(run) = layout.runs().first() {
            self.texts.push(run.content.clone());
            self.origins.push(origin);
        }
    }
    fn clip_rect(&mut self, _: Rect) {}
    fn stroke_line(&mut self, _: Point2D, _: Point2D, _: Color, _: f32) {}
    fn fill_round_rect(&mut self, rect: Rect, radius: f32, color: Color) {
        self.round_fills.push((rect, radius, color));
    }
    fn stroke_round_rect(&mut self, rect: Rect, radius: f32, color: Color, width: f32) {
        self.round_strokes.push((rect, radius, color, width));
    }
    fn stroke_svg_path(&mut self, _: &str, origin: Point2D, size: f32, _: Color, _: f32) {
        self.svg_origins.push(origin);
        self.svg_sizes.push(size);
    }
    fn save(&mut self) {}
    fn restore(&mut self) {}
    fn translate(&mut self, _: Point2D) {}
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
}

pub(super) fn color_eq(a: Color, b: Color) -> bool {
    (a.r - b.r).abs() < 1e-6
        && (a.g - b.g).abs() < 1e-6
        && (a.b - b.b).abs() < 1e-6
        && (a.a - b.a).abs() < 1e-6
}

pub(super) fn caret_fills(fills: &[(Rect, Color)], theme: Theme) -> Vec<Rect> {
    fills
        .iter()
        .filter_map(|(rect, color)| {
            let shared_input_caret = (rect.size.y - 16.0).abs() < 0.01;
            let legacy_caret = (rect.size.y - 18.0).abs() < 0.01;
            (color_eq(*color, theme.foreground)
                && (rect.size.x - 1.5).abs() < 0.01
                && (shared_input_caret || legacy_caret))
                .then_some(*rect)
        })
        .collect()
}

pub(super) fn state_with_three_vars() -> EditorState {
    let mut s = EditorState::new();
    s.create_variable(
        "color-1",
        VariableKind::Color,
        VariableScalar::Str("#ff8800".into()),
    );
    s.create_variable(
        "spacing-md",
        VariableKind::Number,
        VariableScalar::Num(16.0),
    );
    s.create_variable("is-dark", VariableKind::Boolean, VariableScalar::Bool(true));
    s.ui.variables
        .active_theme
        .insert("mode".into(), "dark".into());
    s
}

pub(super) fn state_with_ts_like_themes() -> EditorState {
    let mut s = EditorState::new();
    s.doc
        .themes
        .get_or_insert_with(Default::default)
        .insert("Theme-1".into(), vec!["Default".into(), "Variant-1".into()]);
    s.doc
        .themes
        .get_or_insert_with(Default::default)
        .insert("Theme-2".into(), vec!["Default".into(), "Compact".into()]);
    s.ui.variables
        .active_theme
        .insert("Theme-1".into(), "Default".into());
    s
}
