//! Floating action bar for an in-flight pen session (touch-first).
//!
//! Desktop finishes a pen path with double-click, Enter or Escape; a
//! phone exposes none of those reliably (no keyboard in view, finger
//! double-taps land racy, and the start-anchor close target is small
//! even doubled). While `Tool::Pen` has a session in progress this bar
//! floats over the canvas bottom with four chips — **Done** (commit
//! open), **Close** (commit closed), **Pop** (drop the last anchor),
//! **Cancel** (discard the session) — routed by
//! `WidgetHostNative::dispatch_pen_action_bar_press` and painted by
//! the host frame after the path-anchor context menu.
//!
//! Labels are hardcoded English like every other pen-era surface (the
//! TS tool had no i18n keys for pen chrome either).

use crate::theme::Theme;
use crate::widgets::editor_state_ext::theme_for;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect, TextLayout};
use op_editor_core::{EditorState, Tool};

/// One chip's action. `Done` / `Close` commit the in-progress path
/// open / closed (the core returns to the Select tool — TS
/// `finalizePen`); `Pop` drops the last anchor (a lone anchor cancels
/// the session); `Cancel` discards the whole session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PenAction {
    Done,
    Close,
    Pop,
    Cancel,
}

/// Chip order on screen, mirroring the dispatch arms in the host.
const ACTIONS: &[(PenAction, &str)] = &[
    (PenAction::Done, "Done"),
    (PenAction::Close, "Close"),
    (PenAction::Pop, "Pop"),
    (PenAction::Cancel, "Cancel"),
];

const CHIP_W: f32 = 76.0;
const CHIP_H: f32 = 34.0;
const CHIP_GAP: f32 = 8.0;
const CHIP_FONT: f32 = 13.0;
const LABEL_INSET_X: f32 = 12.0;
const BOTTOM_MARGIN: f32 = 28.0;
/// Touch-first: the bar must float ABOVE the mobile dock (60 dp tall),
/// or chip taps would land in the dock's slots (`press_chrome_tiers.rs`
/// consumes dock presses before the canvas pen tier runs).
const DOCK_CLEAR_GAP: f32 = 16.0;

pub struct PenActionBar {
    pub theme: Theme,
    /// Viewport-bottom offset so the bar clears the mobile dock on
    /// touch-first layouts (and the property/layer sheets later).
    pub bottom_offset: f32,
}

impl PenActionBar {
    /// `Some` while the Pen tool has an in-progress session — the only
    /// state where the bar is drawn or hits.
    pub fn for_editor_ui(state: &EditorState) -> Option<Self> {
        if !matches!(state.tool, Tool::Pen) || state.ui.pen_in_progress.is_none() {
            return None;
        }
        let bottom_offset = if state.editor_ui.touch_chrome() {
            crate::widgets::host_canvas_geometry::MOBILE_DOCK_HEIGHT + DOCK_CLEAR_GAP
        } else {
            BOTTOM_MARGIN
        };
        Some(Self {
            theme: theme_for(&state.editor_ui),
            bottom_offset,
        })
    }

    /// The full bar rect, centered along the bottom of the viewport.
    pub fn rect(&self, viewport_w: f32, viewport_h: f32) -> Rect {
        let w = CHIP_W * ACTIONS.len() as f32 + CHIP_GAP * (ACTIONS.len() as f32 - 1.0);
        Rect {
            origin: Point2D::new(
                (viewport_w - w) / 2.0,
                viewport_h - CHIP_H - self.bottom_offset,
            ),
            size: Point2D::new(w, CHIP_H),
        }
    }

    /// The chip under `point`, `None` outside the bar.
    pub fn hit(&self, point: Point2D, viewport_w: f32, viewport_h: f32) -> Option<PenAction> {
        let rect = self.rect(viewport_w, viewport_h);
        if point.x < rect.origin.x
            || point.x >= rect.origin.x + rect.size.x
            || point.y < rect.origin.y
            || point.y >= rect.origin.y + rect.size.y
        {
            return None;
        }
        let idx = ((point.x - rect.origin.x) / (CHIP_W + CHIP_GAP)) as usize;
        if idx < ACTIONS.len() {
            Some(ACTIONS[idx].0)
        } else {
            None
        }
    }

    pub fn paint(&self, cx: &mut PaintCx<'_>, viewport_w: f32, viewport_h: f32) {
        let rect = self.rect(viewport_w, viewport_h);
        let mut x = rect.origin.x;
        for (_, label) in ACTIONS {
            let chip = Rect {
                origin: Point2D::new(x, rect.origin.y),
                size: Point2D::new(CHIP_W, CHIP_H),
            };
            cx.backend.fill_round_rect(chip, 8.0, self.theme.card);
            cx.backend.stroke_round_rect(chip, 8.0, self.theme.border, 1.0);
            let fg = self.theme.foreground;
            let layout = TextLayout::single_run(
                label,
                "system-ui",
                CHIP_FONT,
                jian_core::scene::Color::rgba(
                    (fg.r * 255.0) as u8,
                    (fg.g * 255.0) as u8,
                    (fg.b * 255.0) as u8,
                    255,
                ),
                Point2D::new(0.0, 0.0),
            );
            cx.backend.draw_text(
                &layout,
                Point2D::new(
                    x + LABEL_INSET_X,
                    rect.origin.y + (CHIP_H - CHIP_FONT) / 2.0,
                ),
            );
            x += CHIP_W + CHIP_GAP;
        }
    }
}