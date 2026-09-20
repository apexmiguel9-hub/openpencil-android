//! Tests for `widgets::agent_settings_panel` — shared fixtures plus the
//! module spine.
//!
//! The grouped test bodies live in the sibling `agent_settings_panel_tests/`
//! directory so every file stays under the 800-line cap.

use crate::widgets::agent_settings_panel::{AgentSettingsHit, AgentSettingsPanel};
use crate::widgets::icons::Icon;
use crate::widgets::{PaintCx, Widget};
use crate::{Color, Point2D, Rect, RenderBackend, TextLayout};
use op_editor_core::agent_settings::{
    AcpAgentField, AgentProvider, AgentSettingsTab, BuiltinAgentField, ImageSearchField,
    ImageTestStatus, SettingsFocus,
};
use op_editor_core::{AgentSettingsButton, ButtonPressTarget, EditorState};

mod acp_agents;
mod acp_presets;
mod builtin_agents;
mod chrome;
mod images_tab;
mod mcp_tab;
mod system_tab;

#[derive(Default)]
struct CaptureBackend {
    fills: Vec<(Rect, Color)>,
    round_fills: Vec<(Rect, Color)>,
    icon_strokes: Vec<(Point2D, f32, usize)>,
    svg_strokes: Vec<(String, Point2D, f32)>,
    text_points: Vec<Point2D>,
    text_effective_points: Vec<(String, Point2D)>,
    ops: Vec<&'static str>,
}

impl RenderBackend for CaptureBackend {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, rect: Rect, color: Color) {
        self.fills.push((rect, color));
    }
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn draw_text(&mut self, layout: &TextLayout, point: Point2D) {
        self.text_points.push(point);
        if let Some(run) = layout.runs().first() {
            self.text_effective_points.push((
                run.content.clone(),
                Point2D::new(point.x + run.origin.x, point.y + run.origin.y),
            ));
        }
    }
    fn clip_rect(&mut self, _: Rect) {}
    fn stroke_line(&mut self, _: Point2D, _: Point2D, _: Color, _: f32) {}
    fn fill_round_rect(&mut self, rect: Rect, _: f32, color: Color) {
        self.round_fills.push((rect, color));
    }
    fn stroke_round_rect(&mut self, _: Rect, _: f32, _: Color, _: f32) {}
    fn stroke_svg_path(&mut self, d: &str, at: Point2D, size: f32, _: Color, _: f32) {
        self.icon_strokes.push((at, size, self.ops.len()));
        self.svg_strokes.push((d.to_owned(), at, size));
        self.ops.push("icon");
    }
    fn save(&mut self) {
        self.ops.push("save");
    }
    fn restore(&mut self) {
        self.ops.push("restore");
    }
    fn translate(&mut self, _: Point2D) {}
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
}

fn color_eq(a: Color, b: Color) -> bool {
    (a.r - b.r).abs() < 1e-6
        && (a.g - b.g).abs() < 1e-6
        && (a.b - b.b).abs() < 1e-6
        && (a.a - b.a).abs() < 1e-6
}

fn caret_fills(fills: &[(Rect, Color)], color: Color) -> Vec<Rect> {
    fills
        .iter()
        .filter_map(|(rect, fill)| {
            (color_eq(*fill, color)
                && (rect.size.x - 1.5).abs() < 0.01
                && (14.0..=16.0).contains(&rect.size.y))
            .then_some(*rect)
        })
        .collect()
}
