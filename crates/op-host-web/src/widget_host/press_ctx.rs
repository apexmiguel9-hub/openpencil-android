//! Per-event scratch state for the web `apply_press` tier ladder.
//!
//! `apply_press` is a strictly ordered hit-test ladder (overlays before
//! panels before canvas). The tier bodies live in the `press_*_tiers.rs`
//! siblings; the values they share travel in these structs rather than in
//! a long argument list.
//!
//! Deliberately per-host: the web ladder's tier ORDER and gating differ
//! from the native host's in several places (no Git-panel gate, an
//! `over_chat_model_picker` gate the native host resolves differently, and
//! a `property_focus_committed` commit point native does not have), so
//! this mirrors — but does not share — `op-host-native`'s `press_ctx`.

/// Values computed by the press prelude and read by later tiers.
///
/// `over_chat_model_picker` and `property_focus_committed` are NOT filled
/// in at construction: the flat ladder resolves them partway down, after
/// steps that mutate `editor_state`. They are assigned at those exact
/// points so the observed state matches the original.
#[derive(Clone, Copy)]
pub(in crate::widget_host) struct PressCtx {
    pub(in crate::widget_host) x: f32,
    pub(in crate::widget_host) y: f32,
    pub(in crate::widget_host) viewport_width: f32,
    pub(in crate::widget_host) viewport_height: f32,
    pub(in crate::widget_host) rename_committed: bool,
    pub(in crate::widget_host) text_edit_was_active: bool,
    pub(in crate::widget_host) text_edit_committed: bool,
    pub(in crate::widget_host) over_chat_model_picker: bool,
    pub(in crate::widget_host) property_focus_committed: bool,
}

/// The subset the canvas tier hands to the Select-tool sub-ladder.
#[derive(Clone, Copy)]
pub(in crate::widget_host) struct CanvasPressCtx {
    pub(in crate::widget_host) x: f32,
    pub(in crate::widget_host) y: f32,
    pub(in crate::widget_host) viewport_width: f32,
    pub(in crate::widget_host) viewport_height: f32,
    pub(in crate::widget_host) rename_committed: bool,
    pub(in crate::widget_host) text_edit_was_active: bool,
    pub(in crate::widget_host) text_edit_committed: bool,
    pub(in crate::widget_host) property_focus_committed: bool,
}
