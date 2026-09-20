//! Figma-style image-fill crop editing shared by the native and web
//! widget hosts (their `widget_host/image_crop_drag.rs` twins were
//! byte-identical apart from the host type name and three comments).
//!
//! Everything here reads the layout scene and mutates `EditorState`
//! only; the hosts keep the platform tail (`refresh_layout_scene`,
//! `scene_cache.invalidate`, `mark_dirty`, clearing the node-drag
//! gesture) in thin wrappers.

use op_editor_core::{EditorSnapshot, EditorState, NodeId};

use crate::layout_scene::LayoutScene;

/// In-flight crop pan over one image-fill node.
#[derive(Debug, Clone)]
pub struct ImageCropDragState {
    /// Node whose bitmap is being panned.
    pub node_id: NodeId,
    /// Last pointer position in screen px.
    pub last_screen_x: f32,
    /// Last pointer position in screen px.
    pub last_screen_y: f32,
    /// Resolved node box in doc px — the crop clamp reference.
    pub node_width: f32,
    /// Resolved node box in doc px — the crop clamp reference.
    pub node_height: f32,
    /// Root-to-editing-node inverse delta transforms. Applying these in
    /// order mirrors layout-scene hit testing through rotated/flipped parents.
    pub inverse_transforms: Vec<(f32, bool, bool)>,
    /// Latches once the crop actually moved, so a bare click pushes no
    /// history entry.
    pub moved: bool,
    /// Pre-gesture snapshot, pushed on finish when `moved`.
    pub pre_drag_snapshot: EditorSnapshot,
}

/// One step of a crop pan.
pub enum ImageCropMove {
    /// The selection anchor moved off the dragged node — the host drops
    /// the gesture and repaints. Crop editing is already cleared.
    Detached,
    /// No pointer travel; nothing consumed.
    Idle,
    /// Pointer advanced; `changed` reports whether the crop moved.
    Moved {
        /// The crop offset actually changed (not clamped at an edge).
        changed: bool,
    },
}

/// Enter the dedicated crop editor for the current selection.
///
/// `None` when the selection cannot be crop-edited; otherwise `Some`
/// with whether the editing target changed (the host repaints then).
pub fn enter_selected_image_crop_edit(state: &mut EditorState) -> Option<bool> {
    if !state.can_edit_selected_image_crop() {
        return None;
    }
    let id = state.selection.anchor.clone();
    let changed = state.editor_ui.image_crop_editing.as_ref() != Some(&id);
    state.editor_ui.image_crop_editing = Some(id);
    Some(changed)
}

/// Leave the crop editor. Returns whether an editing target was live.
pub fn clear_image_crop_editing(state: &mut EditorState) -> bool {
    state.editor_ui.image_crop_editing.take().is_some()
}

/// State-only guard for `start_image_crop_drag`, split out so the host
/// does not refresh the layout scene for a press that cannot pan.
pub fn can_start_image_crop_drag(state: &EditorState, target: &NodeId) -> bool {
    state.editor_ui.image_crop_editing.as_ref() == Some(target)
        && &state.selection.anchor == target
        && state.can_edit_selected_image_crop()
}

/// Open a crop pan over `target`, collecting the root-to-target
/// inverse transform chain from the rendered hit path so the pointer
/// delta can be mapped into the node's local space.
///
/// The caller must have refreshed the layout scene and passed the
/// `can_start_image_crop_drag` guard.
pub fn start_image_crop_drag(
    state: &EditorState,
    scene: &LayoutScene,
    target: &NodeId,
    hit_path: &[NodeId],
    x: f32,
    y: f32,
) -> Option<ImageCropDragState> {
    let page = scene.active_page()?;
    let scene_node = page.find(target.as_str())?;
    if scene_node.bounds.size.x <= 0.0 || scene_node.bounds.size.y <= 0.0 {
        return None;
    }
    let mut inverse_transforms = Vec::new();
    let mut found_target = false;
    for id in hit_path {
        let Some(node) = page.find(id.as_str()) else {
            continue;
        };
        inverse_transforms.push((node.rotation, node.flip_x, node.flip_y));
        if id == target {
            found_target = true;
            break;
        }
    }
    if !found_target {
        return None;
    }
    Some(ImageCropDragState {
        node_id: target.clone(),
        last_screen_x: x,
        last_screen_y: y,
        node_width: scene_node.bounds.size.x,
        node_height: scene_node.bounds.size.y,
        inverse_transforms,
        moved: false,
        pre_drag_snapshot: state.snapshot_for_history(),
    })
}

/// Advance a crop pan by the pointer delta, unwinding the parent
/// rotation / flip chain so the bitmap follows the cursor on screen.
pub fn image_crop_drag_cursor_move(
    state: &mut EditorState,
    drag: &mut ImageCropDragState,
    x: f32,
    y: f32,
) -> ImageCropMove {
    if drag.node_id != state.selection.anchor {
        state.editor_ui.image_crop_editing = None;
        return ImageCropMove::Detached;
    }
    let screen_dx = x - drag.last_screen_x;
    let screen_dy = y - drag.last_screen_y;
    if screen_dx == 0.0 && screen_dy == 0.0 {
        return ImageCropMove::Idle;
    }
    let zoom = state.viewport.zoom.max(0.0001);
    let mut local_dx = screen_dx / zoom;
    let mut local_dy = screen_dy / zoom;
    for (rotation, flip_x, flip_y) in &drag.inverse_transforms {
        let cos = rotation.cos();
        let sin = rotation.sin();
        let rotated_x = cos * local_dx + sin * local_dy;
        let rotated_y = -sin * local_dx + cos * local_dy;
        local_dx = if *flip_x { -rotated_x } else { rotated_x };
        local_dy = if *flip_y { -rotated_y } else { rotated_y };
    }
    let node_width = drag.node_width;
    let node_height = drag.node_height;
    // Always advance the pointer anchor, including at a clamped edge,
    // so reversing direction responds immediately.
    drag.last_screen_x = x;
    drag.last_screen_y = y;
    state.editor_ui.last_canvas_click = None;
    let changed = state.translate_selected_image_crop(node_width, node_height, local_dx, local_dy);
    if changed {
        drag.moved = true;
    }
    ImageCropMove::Moved { changed }
}

/// Close a crop pan, pushing the pre-gesture snapshot when the crop
/// actually moved. Returns whether history was pushed (the host then
/// invalidates its scene cache and repaints).
pub fn finish_image_crop_drag(state: &mut EditorState, drag: ImageCropDragState) -> bool {
    if !drag.moved {
        return false;
    }
    state.history_push_past(drag.pre_drag_snapshot);
    true
}
