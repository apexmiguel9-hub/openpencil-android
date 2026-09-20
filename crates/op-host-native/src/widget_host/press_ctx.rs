//! Per-event scratch state for the `apply_press` tier ladder.
//!
//! `apply_press` is a strictly ordered hit-test ladder (overlays before
//! panels before canvas — see `crates/CLAUDE.md`). The tier bodies live in
//! the `press_*_tiers.rs` siblings; the handful of values they all need
//! travel in these structs rather than in a long argument list.

use op_editor_ui::{Point2D, Rect};

/// Values computed by the press prelude and read by later tiers.
///
/// `in_git_panel` / `in_chat_model_picker` are deliberately NOT filled in
/// at construction: the original ladder resolves them only after the
/// top-most overlay tier and the commit-on-blur step have run, and some of
/// those steps mutate `editor_state`. They are assigned at that exact point
/// so the geometry is sampled from the same state as before.
#[derive(Clone, Copy)]
pub(in crate::widget_host) struct PressCtx {
    pub(in crate::widget_host) x: f32,
    pub(in crate::widget_host) y: f32,
    pub(in crate::widget_host) viewport_width: f32,
    pub(in crate::widget_host) viewport_height: f32,
    pub(in crate::widget_host) rename_committed: bool,
    pub(in crate::widget_host) text_edit_was_active: bool,
    pub(in crate::widget_host) text_edit_committed: bool,
    /// The floating Git panel paints over the right rail, so every rail
    /// tier skips a point it would otherwise own.
    pub(in crate::widget_host) in_git_panel: bool,
    pub(in crate::widget_host) in_chat_model_picker: bool,
}

/// Extra geometry the canvas tier resolves once and shares with the
/// Select-tool sub-ladder.
#[derive(Clone, Copy)]
pub(in crate::widget_host) struct CanvasPressCtx {
    pub(in crate::widget_host) x: f32,
    pub(in crate::widget_host) y: f32,
    pub(in crate::widget_host) viewport_width: f32,
    pub(in crate::widget_host) viewport_height: f32,
    pub(in crate::widget_host) canvas_rect: Rect,
    pub(in crate::widget_host) doc_point: Point2D,
    pub(in crate::widget_host) text_edit_was_active: bool,
    /// Whether the canvas press already blurred the chrome text inputs.
    pub(in crate::widget_host) blurred: bool,
}
