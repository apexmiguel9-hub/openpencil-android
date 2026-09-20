//! Mouse selection flow for the PropertyPanel image popovers' Search /
//! Generate text inputs, shared by the native and web widget hosts.
//!
//! Their `widget_host/image_panel_selection.rs` twins used to carry this
//! logic twice. The host keeps the two pieces that are genuinely
//! host-state — the painter-measured input geometry cache and the active
//! drag — and passes them in; everything else (visibility gating, caret /
//! selection writes) is `EditorState` mutation and lives here.

use op_editor_core::EditorState;

use crate::widgets::property_panel_image_assets::{
    ImagePopoverInputGeometry, ImagePopoverInputKind,
};
use crate::widgets::PropertyPanel;
use crate::{Point2D, Rect};

/// An in-flight text selection drag inside one of the image popovers.
#[derive(Debug, Clone, Copy)]
pub struct ImageInputSelectionDrag {
    /// Which popover input the drag started in.
    pub kind: ImagePopoverInputKind,
    /// Byte offset the selection is anchored at.
    pub anchor: usize,
}

/// Whether `kind`'s popover input is actually on screen right now — the
/// Generate prompt additionally needs a configured image-gen profile.
pub fn input_visible(state: &EditorState, kind: ImagePopoverInputKind) -> bool {
    let panel = &state.editor_ui.image_panel;
    match kind {
        ImagePopoverInputKind::Search => panel.search_open,
        ImagePopoverInputKind::Generate => {
            panel.generate_open
                && state.editor_ui.agent_settings.image_generation_configured()
                && panel.active_input(true).is_some()
        }
    }
}

/// Map a press point onto a popover input + byte offset, preferring the
/// painter-measured geometry cache over the panel's layout estimate.
pub fn input_at(
    state: &EditorState,
    geometry: Option<&ImagePopoverInputGeometry>,
    panel: &PropertyPanel,
    rect: Rect,
    point: Point2D,
) -> Option<(ImagePopoverInputKind, usize)> {
    if let Some(geometry) = geometry {
        if input_visible(state, geometry.kind) {
            if let Some(offset) =
                geometry.byte_offset_at(&state.editor_ui.image_panel, point, false)
            {
                return Some((geometry.kind, offset));
            }
        }
    }
    panel.image_popover_input_at(rect, point)
}

/// Caret rect from the painter-measured geometry cache, when its input is
/// still visible.
pub fn cached_caret_rect(
    state: &EditorState,
    geometry: Option<&ImagePopoverInputGeometry>,
) -> Option<Rect> {
    let geometry = geometry?;
    input_visible(state, geometry.kind).then_some(())?;
    geometry.caret_rect(&state.editor_ui.image_panel)
}

/// Drag-time byte offset from the geometry cache (clamped to the input's
/// extent, unlike the press-time lookup).
pub fn cached_drag_offset(
    state: &EditorState,
    geometry: Option<&ImagePopoverInputGeometry>,
    kind: ImagePopoverInputKind,
    point: Point2D,
) -> Option<usize> {
    let geometry = geometry?;
    if geometry.kind != kind || !input_visible(state, kind) {
        return None;
    }
    geometry.byte_offset_at(&state.editor_ui.image_panel, point, true)
}

/// Start a selection drag: place the caret (or extend the existing
/// selection while shift is held) and report the resulting anchor.
/// `None` when the targeted input is not editable right now.
pub fn begin_selection_drag(
    state: &mut EditorState,
    kind: ImagePopoverInputKind,
    offset: usize,
    extend: bool,
    now_ms: u64,
) -> Option<ImageInputSelectionDrag> {
    let configured = state.editor_ui.agent_settings.image_generation_configured();
    let panel = &mut state.editor_ui.image_panel;
    let input = match kind {
        ImagePopoverInputKind::Search if panel.search_open => &mut panel.search_query,
        ImagePopoverInputKind::Generate if panel.generate_open && configured => {
            &mut panel.generate_prompt
        }
        _ => return None,
    };
    if extend {
        input.drag_to(offset, now_ms);
    } else {
        input.set_caret(offset, now_ms);
    }
    let anchor = input.selection().anchor;
    Some(ImageInputSelectionDrag { kind, anchor })
}

/// Extend an in-flight drag to `focus`. Returns `None` when the input
/// went away mid-drag, else `Some(changed)` — `changed` is the host's
/// `mark_dirty` trigger.
pub fn drag_selection_to(
    state: &mut EditorState,
    drag: ImageInputSelectionDrag,
    focus: usize,
    now_ms: u64,
) -> Option<bool> {
    let panel = &mut state.editor_ui.image_panel;
    let input = match drag.kind {
        ImagePopoverInputKind::Search if panel.search_open => &mut panel.search_query,
        ImagePopoverInputKind::Generate if panel.generate_open => &mut panel.generate_prompt,
        _ => return None,
    };
    let before = input.selection();
    input.set_caret(drag.anchor, now_ms);
    input.drag_to(focus, now_ms);
    Some(input.selection() != before)
}
