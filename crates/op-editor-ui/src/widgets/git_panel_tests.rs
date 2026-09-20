//! Tests for `widgets::git_panel` — moved to a sibling file to keep
//! `git_panel.rs` under the 800-line cap.
//!
//! Shared fixtures live here; the grouped test bodies live in the sibling
//! `git_panel_tests/` directory so every file stays under the same cap.

use crate::widgets::git_panel::*;
use crate::{Color, Point2D, Rect, RenderBackend, TextLayout};
use op_editor_core::{
    ButtonPressTarget, CloneField, CloneFormState, CommitDiffPatch, CommitDiffSummary,
    CommitDiffView, EditorState, GitBranchPickerMode, GitCandidateFile, GitCommitSummary,
    GitDiffView, GitFileEntry, GitOverflowView, GitPanelState, MergeConflictRow, MergeResolveFile,
    MergeResolveState,
};

mod basics;
mod branches_and_menus;
mod diff;
mod empty_and_clone;
mod merge;
mod ready_view;
mod ready_view_polish;

#[derive(Default)]
struct RoundFillBackend {
    fills: Vec<(Rect, f32, Color)>,
}

impl RenderBackend for RoundFillBackend {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, _: Rect, _: Color) {}
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn draw_text(&mut self, _: &TextLayout, _: Point2D) {}
    fn clip_rect(&mut self, _: Rect) {}
    fn stroke_line(&mut self, _: Point2D, _: Point2D, _: Color, _: f32) {}
    fn fill_round_rect(&mut self, rect: Rect, radius: f32, color: Color) {
        self.fills.push((rect, radius, color));
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

fn color_close(a: Color, b: Color) -> bool {
    (a.r - b.r).abs() < 1e-6
        && (a.g - b.g).abs() < 1e-6
        && (a.b - b.b).abs() < 1e-6
        && (a.a - b.a).abs() < 1e-6
}

fn state_with(panel: GitPanelState) -> EditorState {
    let mut s = EditorState::new();
    s.editor_ui.git_panel = panel;
    s
}

fn open_repo() -> GitPanelState {
    GitPanelState {
        open: true,
        in_repo: true,
        ..GitPanelState::default()
    }
}

fn centre(r: Rect) -> Point2D {
    Point2D::new(r.origin.x + r.size.x / 2.0, r.origin.y + r.size.y / 2.0)
}

/// A panel rect sized to the panel's current mode.
fn panel_rect(panel: &GitPanel<'_>) -> Rect {
    Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(panel.panel_width(), panel.height()),
    }
}

fn one_commit() -> GitCommitSummary {
    GitCommitSummary {
        short_hash: "abc1234".into(),
        summary: "first".into(),
        author: "Ada".into(),
        time_label: "now".into(),
        is_initial: false,
    }
}
