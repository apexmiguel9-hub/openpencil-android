//! Transient pointer-drag state shared by the native and web hosts.
//!
//! Both `widget_host.rs` spines declared byte-identical copies of these
//! plain-data structs. They carry no platform types — only `NodeId`,
//! `EditorSnapshot`, `pen::PathHandleSide` and screen/document points —
//! so they live here and each host re-exports them under its historical
//! `widget_host` paths.
//!
//! These types are deliberately dumb: they record where a gesture
//! started and what it grabbed. The state machines that read them stay
//! host-side, because whether a given gesture is even reachable depends
//! on the platform's event model (trackpad vs. wheel, pointer capture
//! vs. mouse-up-anywhere).

use crate::render_backend::Point2D;

/// Canvas pan-drag — tracks the previous cursor position so each move
/// applies an incremental screen-space delta to the viewport.
#[derive(Debug, Clone, Copy)]
pub struct DragState {
    pub last_x: f32,
    pub last_y: f32,
}

/// Floating AI-chat panel header drag. `grab_*` is the pointer offset
/// within the panel rect at press time — subtracting it from the live
/// cursor gives the panel top-left, so the panel doesn't jump on press.
/// `pos_*` is that live top-left (logical px, viewport-relative);
/// release snaps to the nearest corner via `ChatAnchor::nearest`.
#[derive(Debug, Clone, Copy)]
pub struct ChatDragState {
    pub grab_dx: f32,
    pub grab_dy: f32,
    pub pos_x: f32,
    pub pos_y: f32,
}

/// Header drag on a floating panel (Design-MD / Component-Browser /
/// Icon-picker). All three carry only the grab offset, so they share one
/// shape; the native host aliases its three historical names onto it.
#[derive(Debug, Clone, Copy)]
pub struct PanelDragState {
    pub grab_dx: f32,
    pub grab_dy: f32,
}

/// Generated-code preview text selection drag — `anchor` is the byte
/// offset the press landed on; moves extend `anchor..focus`.
#[derive(Debug, Clone, Copy)]
pub struct CodeSelectionDragState {
    pub anchor: usize,
}

/// Chat input text selection drag.
#[derive(Debug, Clone, Copy)]
pub struct ChatInputSelectionDragState {
    pub anchor: usize,
}

/// Chat transcript text selection drag — scoped to one message so a
/// drag can't run across bubble boundaries.
#[derive(Debug, Clone, Copy)]
pub struct ChatTextSelectionDragState {
    pub message_index: usize,
    pub anchor: usize,
}

/// Inline canvas text-edit selection drag — `anchor` is the byte offset
/// placed by the press; cursor moves extend `anchor..focus`.
#[derive(Debug, Clone, Copy)]
pub struct TextEditSelectionDragState {
    pub anchor: usize,
}

/// Active marquee rect-select drag. Endpoints are in SCREEN coordinates
/// so paint can draw the rect without re-deriving the canvas→screen
/// transform; release converts to doc space once to ask the scene which
/// nodes overlap.
#[derive(Debug, Clone, Copy)]
pub struct MarqueeDragState {
    pub start_screen_x: f32,
    pub start_screen_y: f32,
    pub current_screen_x: f32,
    pub current_screen_y: f32,
    /// Whether shift was held at press time. `false` REPLACES the
    /// selection with the hit set; `true` is ADD-only — every hit joins
    /// and already-selected hits stay selected (TS shift-marquee
    /// parity). Both hosts implement it this way.
    pub additive: bool,
}

/// Active LayerPanel drag-to-reorder gesture.
#[derive(Debug, Clone)]
pub struct LayerDragState {
    /// Row the user pressed on — what gets moved on release.
    pub source: crate::NodeId,
    /// Cursor y at press time, panel-local. Suppresses drag activation
    /// until the cursor has moved a few pixels, so a regular click is
    /// never promoted to a drag.
    pub start_y: f32,
    /// Live cursor x / y for the drop-target hit-test.
    pub current_x: f32,
    pub current_y: f32,
    /// `false` = still a candidate click; `true` = committed drag (paint
    /// the drop indicator).
    pub active: bool,
}

/// What a path-anchor drag is editing — the anchor body itself, or one
/// of its two bezier control handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorDragTarget {
    /// The anchor point — drag moves the whole anchor.
    Anchor,
    /// A bezier control handle.
    Handle(crate::pen::PathHandleSide),
}

/// Path-anchor drag — which anchor (or which of its bezier handles) of
/// which Path node is being dragged, under the Pen or Select tool. Move
/// dispatches apply the TS-style cumulative cursor delta
/// (`movePathControl`, `path-editing.ts:66-114` — the grab offset is
/// preserved, no snap); release commits a history snapshot ONLY when it
/// actually moved (a press-release without motion otherwise pushed a
/// no-op snapshot that polluted the undo stack).
#[derive(Debug, Clone)]
pub struct PathAnchorDragState {
    pub node_id: crate::NodeId,
    pub anchor_index: usize,
    /// Whether the anchor body or a handle is being dragged.
    pub target: AnchorDragTarget,
    /// The dragged anchor's absolute doc position, fixed at press —
    /// handle drags compute their offset relative to it.
    pub anchor_doc: Point2D,
    /// Press cursor doc point (un-rotated into the node's local frame
    /// for rotated paths) — base of the cumulative drag delta and the
    /// did-it-move gate.
    pub start_doc: Point2D,
    /// The grabbed handle's offset at press. `Some` = an existing
    /// handle, edited with TS `movePathControl` semantics; `None` for
    /// the anchor body or a Pen-tool ghost mint (deliberate Rust
    /// superset — TS cannot grab an unset handle).
    pub grab_offset: Option<Point2D>,
    /// Shift held at press — a ghost-handle MINT with Shift produces
    /// independent (broken) handles instead of mirrored ones.
    pub shift: bool,
    /// Set to true on the first cursor-move that mutates the target.
    pub moved: bool,
    /// Snapshot captured at drag-start; pushed only if `moved`.
    pub pre_drag_snapshot: crate::EditorSnapshot,
}
