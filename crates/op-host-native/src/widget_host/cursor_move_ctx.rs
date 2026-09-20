//! Shared per-event state for the `apply_cursor_move` tier ladder.
//!
//! `apply_cursor_move` is a strictly ordered ladder: overlays get first
//! refusal, then floating panels, then the base rails / canvas. The tiers
//! live in sibling modules (`cursor_move_*.rs`) but several of them read
//! and write the *same* per-event scratch values, so those travel in this
//! struct instead of a growing argument list.
//!
//! Field semantics are exactly the locals the single monolithic method
//! used to declare:
//! - `upper_hover_changed` accumulates hover writes made by a tier that
//!   did NOT consume the event (because a higher surface — the chat or its
//!   model picker — owns the point). The final tier folds it into the
//!   repaint signal.
//! - `cleared` records the one-shot stale-hover clear performed when a
//!   top-most panel covers the point.
//! - `property_panel_probe` is a two-level cache: the outer `None` means
//!   "never attempted", `Some(None)` means "attempted, selection cannot
//!   produce a panel". Tiers reuse it so the expensive snapshot / i18n
//!   work happens at most once per cursor event.

use op_editor_ui::widgets::PropertyPanel;
use op_editor_ui::{Point2D, Rect};

pub(in crate::widget_host) struct CursorMoveCtx {
    pub(in crate::widget_host) x: f32,
    pub(in crate::widget_host) y: f32,
    pub(in crate::widget_host) point: Point2D,
    pub(in crate::widget_host) property_rect: Rect,
    /// The chat panel's cheap surface test (excludes the model picker).
    pub(in crate::widget_host) chat_surface_owns_point: bool,
    /// Chat surface OR its open model picker owns the point.
    pub(in crate::widget_host) chat_or_picker_owns_point: bool,
    pub(in crate::widget_host) over_topmost: bool,
    pub(in crate::widget_host) upper_hover_changed: bool,
    pub(in crate::widget_host) cleared: bool,
    pub(in crate::widget_host) property_panel_probe: Option<Option<PropertyPanel>>,
}
