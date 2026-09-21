//! Geometry/vertex edit FFI bindings.
//!
//! These functions allow the host (Android) to enter geometry edit
//! mode, drag anchors and handles, and exit — all while the engine
//! owns the mutation bridge, history management, and scene rebuild.
//!
//! The host sends screen-space coordinates and the engine handles
//! the rest: coordinate conversion, geometry mutation, history, and
//! scene invalidation.

use crate::lifecycle::call_session;
use crate::OpStatus;
use op_editor_core::geometry_edit::{DraggedHandle, GeometryHitTarget};
use op_editor_core::Point2D;
use op_editor_core::Rect;

/// Hit-test result encoding for the C ABI.
/// Value 0 means empty. Otherwise `hit_type << 24 | index`.
const HIT_EMPTY: i32 = 0;
const HIT_ANCHOR: i32 = 1;
const HIT_HANDLE_IN: i32 = 2;
const HIT_HANDLE_OUT: i32 = 3;
const HIT_SEGMENT: i32 = 4;

/// Encode a hit-test result as a jint.
fn encode_hit(hit: GeometryHitTarget) -> i32 {
    match hit {
        GeometryHitTarget::Empty => HIT_EMPTY,
        GeometryHitTarget::Anchor(idx) => (HIT_ANCHOR << 24) | (idx as i32),
        GeometryHitTarget::Handle(DraggedHandle::In(idx)) => (HIT_HANDLE_IN << 24) | (idx as i32),
        GeometryHitTarget::Handle(DraggedHandle::Out(idx)) => (HIT_HANDLE_OUT << 24) | (idx as i32),
        GeometryHitTarget::Segment(idx) => (HIT_SEGMENT << 24) | (idx as i32),
    }
}

/// Enter geometry/vertex edit mode for the given node.
///
/// `node_id` is a null-terminated UTF-8 string, or an empty string
/// (first byte == 0) to use the currently selected node. Returns `OpStatus::Ok`
/// on success or `OpStatus::InvalidArg` if the node id is invalid
/// (non-empty but not found) or no node is selected when `node_id`
/// is empty.
#[no_mangle]
pub unsafe extern "C" fn op_editor_geometry_enter(
    engine: *mut crate::OpEngine,
    node_id: *const std::ffi::c_char,
) -> OpStatus {
    if node_id.is_null() {
        return OpStatus::InvalidArg;
    }
    // Handle empty string (first byte == 0) without calling CStr::from_ptr
    // which requires null-termination. Empty string means "use current selection".
    if *node_id == 0 {
        return call_session(engine, |session| {
            let host = session.editor_mut()?;
            let host_ptr: *mut op_host_native::WidgetHostNative = host as *const _ as *mut _;
            let state: &op_editor_core::state::EditorState = unsafe { &*host_ptr }.editor_state();
            let hit = if let Some(geo_session) = state.geometry_edit_session() {
                if let Some(node_id) = geo_session.edited_node_ids.iter().next() {
                    unsafe { &mut *host_ptr }.editor_state_mut().enter_geometry_edit(node_id);
                    true
                } else {
                    false
                }
            } else if let Some(node_id) = state.selection.set.iter().next() {
                let node_id_str: &str = node_id.as_str();
                unsafe { &mut *host_ptr }.editor_state_mut().enter_geometry_edit(node_id_str);
                true
            } else {
                false
            };
            if hit { Ok(()) } else {
                Err(crate::error::FfiError::new(
                    OpStatus::InvalidArg,
                    "no selected node",
                ))
            }
        });
    }
    let node_id_str = match std::ffi::CStr::from_ptr(node_id).to_str() {
        Ok(s) => s,
        Err(_) => return OpStatus::InvalidArg,
    };
    call_session(engine, |session| {
        let host = session.editor_mut()?;
        host.editor_state_mut().enter_geometry_edit(node_id_str);
        Ok(())
    })
}

/// Exit geometry/vertex edit mode.
#[no_mangle]
pub unsafe extern "C" fn op_editor_geometry_exit(engine: *mut crate::OpEngine) -> OpStatus {
    call_session(engine, |session| {
        let host = session.editor_mut()?;
        host.editor_state_mut().exit_geometry_edit();
        Ok(())
    })
}

/// Hit-test the geometry edit overlay.
///
/// Returns a jint encoding the hit target:
/// - 0 = empty
/// - `1 << 24 | idx` = anchor hit at index `idx`
/// - `2 << 24 | idx` = incoming handle hit at anchor `idx`
/// - `3 << 24 | idx` = outgoing handle hit at anchor `idx`
/// - `4 << 24 | idx` = segment hit at index `idx`
///
/// `canvas_w`, `canvas_h` are the canvas dimensions in screen pixels.
/// `screen_x`, `screen_y` are in screen pixels (logical px, top-left origin).
///
/// Returns `HIT_EMPTY` if no geometry session is active, canvas size is
/// invalid, or the layout scene is not available.
#[no_mangle]
pub unsafe extern "C" fn op_editor_geometry_hit_test(
    engine: *mut crate::OpEngine,
    screen_x: f32,
    screen_y: f32,
    canvas_w: i32,
    canvas_h: i32,
) -> i32 {
    // Early validation
    if canvas_w <= 0 || canvas_h <= 0 {
        return HIT_EMPTY;
    }

    let mut result = HIT_EMPTY;
    let status = call_session(engine, |session| {
        let host = session.editor_mut()?;
        // Use the combined accessor to get both state and layout scene
        // atomically, avoiding borrow conflicts and ensuring the scene
        // is fresh for the current editor state.
        let (state, scene) = host.editor_state_mut_and_layout_scene();
        let geo_session = state.geometry_edit_session();
        if let Some(gs) = geo_session {
            let canvas_rect = Rect {
                origin: Point2D::new(0.0, 0.0),
                size: Point2D::new(canvas_w as f32, canvas_h as f32),
            };
            let point = Point2D::new(screen_x, screen_y);
            let hit = op_editor_ui::widgets::canvas_viewport::geometry_hit_test(
                canvas_rect,
                scene,
                state,
                gs,
                point,
            );
            result = encode_hit(hit);
        }
        Ok(())
    });
    if status == OpStatus::Ok { result } else { HIT_EMPTY }
}

/// Get the node ID currently being geometry-edited.
/// Returns empty string when no node is being edited.
#[no_mangle]
pub unsafe extern "C" fn op_editor_geometry_get_node_id(
    engine: *mut crate::OpEngine,
    out: *mut std::ffi::c_char,
    out_len: usize,
) -> OpStatus {
    if out.is_null() || out_len == 0 {
        return OpStatus::InvalidArg;
    }
    call_session(engine, |session| {
        let host = session.editor_mut()?;
        let state = host.editor_state();
        if let Some(geo_session) = state.geometry_edit_session() {
            if let Some(node_id) = geo_session.edited_node_ids.iter().next() {
                let bytes = node_id.as_bytes();
                let len = bytes.len().min(out_len - 1);
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), out, len);
                *out.add(len) = 0;
                return Ok(());
            }
        }
        unsafe { *out = 0; }
        Ok(())
    })
}

/// Begin dragging an anchor. Pushes exactly one history snapshot.
///
/// `screen_x`, `screen_y` are in screen pixels (logical px, top-left origin).
/// The engine converts to document space using the viewport zoom.
#[no_mangle]
pub unsafe extern "C" fn op_editor_geometry_begin_anchor_drag(
    engine: *mut crate::OpEngine,
    node_id: *const std::ffi::c_char,
    anchor_idx: usize,
    screen_x: f32,
    screen_y: f32,
) -> OpStatus {
    if node_id.is_null() {
        return OpStatus::InvalidArg;
    }
    let node_id_str = match std::ffi::CStr::from_ptr(node_id).to_str() {
        Ok(s) => s,
        Err(_) => return OpStatus::InvalidArg,
    };
    if node_id_str.is_empty() {
        return OpStatus::InvalidArg;
    }
    call_session(engine, |session| {
        let host = session.editor_mut()?;
        let state = host.editor_state_mut();
        let screen_point = op_editor_core::Point2D::new(screen_x, screen_y);
        if !state.geometry_begin_anchor_drag(node_id_str, anchor_idx, screen_point) {
            return Err(crate::error::FfiError::new(
                OpStatus::InvalidArg,
                "begin_anchor_drag failed",
            ));
        }
        Ok(())
    })
}

/// Move the dragged anchor by a screen-space delta.
///
/// `screen_dx`, `screen_dy` are the pixel displacement since the last
/// call (or since drag-begin). The engine converts to document space
/// using the viewport zoom.
#[no_mangle]
pub unsafe extern "C" fn op_editor_geometry_move_anchor_drag(
    engine: *mut crate::OpEngine,
    node_id: *const std::ffi::c_char,
    anchor_idx: usize,
    screen_dx: f32,
    screen_dy: f32,
) -> OpStatus {
    if node_id.is_null() {
        return OpStatus::InvalidArg;
    }
    let node_id_str = match std::ffi::CStr::from_ptr(node_id).to_str() {
        Ok(s) => s,
        Err(_) => return OpStatus::InvalidArg,
    };
    if node_id_str.is_empty() {
        return OpStatus::InvalidArg;
    }
    call_session(engine, |session| {
        let host = session.editor_mut()?;
        let viewport = host.editor_state().viewport;
        let screen_delta = op_editor_core::Point2D::new(screen_dx, screen_dy);
        let state = host.editor_state_mut();
        if !state.geometry_move_anchor_drag(node_id_str, anchor_idx, screen_delta, viewport) {
            return Err(crate::error::FfiError::new(
                OpStatus::InvalidArg,
                "move_anchor_drag failed",
            ));
        }
        Ok(())
    })
}

/// End the anchor drag.
#[no_mangle]
pub unsafe extern "C" fn op_editor_geometry_end_anchor_drag(engine: *mut crate::OpEngine) -> OpStatus {
    call_session(engine, |session| {
        let host = session.editor_mut()?;
        host.editor_state_mut().geometry_end_anchor_drag();
        Ok(())
    })
}

/// Begin dragging a handle. Pushes exactly one history snapshot.
///
/// `side`: 0 = incoming handle (In), 1 = outgoing handle (Out).
#[no_mangle]
pub unsafe extern "C" fn op_editor_geometry_begin_handle_drag(
    engine: *mut crate::OpEngine,
    node_id: *const std::ffi::c_char,
    anchor_idx: usize,
    side: u8,
    screen_x: f32,
    screen_y: f32,
) -> OpStatus {
    if node_id.is_null() {
        return OpStatus::InvalidArg;
    }
    let node_id_str = match std::ffi::CStr::from_ptr(node_id).to_str() {
        Ok(s) => s,
        Err(_) => return OpStatus::InvalidArg,
    };
    if node_id_str.is_empty() {
        return OpStatus::InvalidArg;
    }
    let drag_side = if side == 1 {
        op_editor_core::geometry_edit::DraggedHandle::Out(anchor_idx)
    } else {
        op_editor_core::geometry_edit::DraggedHandle::In(anchor_idx)
    };
    call_session(engine, |session| {
        let host = session.editor_mut()?;
        let state = host.editor_state_mut();
        let screen_point = op_editor_core::Point2D::new(screen_x, screen_y);
        if !state.geometry_begin_handle_drag(node_id_str, anchor_idx, drag_side, screen_point) {
            return Err(crate::error::FfiError::new(
                OpStatus::InvalidArg,
                "begin_handle_drag failed",
            ));
        }
        Ok(())
    })
}

/// Move the dragged handle by a screen-space delta.
///
/// Respects the existing Mirrored / Independent / Corner logic
/// inside `move_path_anchor_handle_ts`.
#[no_mangle]
pub unsafe extern "C" fn op_editor_geometry_move_handle_drag(
    engine: *mut crate::OpEngine,
    node_id: *const std::ffi::c_char,
    anchor_idx: usize,
    side: u8,
    screen_dx: f32,
    screen_dy: f32,
) -> OpStatus {
    if node_id.is_null() {
        return OpStatus::InvalidArg;
    }
    let node_id_str = match std::ffi::CStr::from_ptr(node_id).to_str() {
        Ok(s) => s,
        Err(_) => return OpStatus::InvalidArg,
    };
    if node_id_str.is_empty() {
        return OpStatus::InvalidArg;
    }
    let drag_side = if side == 1 {
        op_editor_core::geometry_edit::DraggedHandle::Out(anchor_idx)
    } else {
        op_editor_core::geometry_edit::DraggedHandle::In(anchor_idx)
    };
    call_session(engine, |session| {
        let host = session.editor_mut()?;
        let viewport = host.editor_state().viewport;
        let screen_delta = op_editor_core::Point2D::new(screen_dx, screen_dy);
        let state = host.editor_state_mut();
        if !state.geometry_move_handle_drag(node_id_str, anchor_idx, drag_side, screen_delta, viewport) {
            return Err(crate::error::FfiError::new(
                OpStatus::InvalidArg,
                "move_handle_drag failed",
            ));
        }
        Ok(())
    })
}

/// End the handle drag.
#[no_mangle]
pub unsafe extern "C" fn op_editor_geometry_end_handle_drag(engine: *mut crate::OpEngine) -> OpStatus {
    call_session(engine, |session| {
        let host = session.editor_mut()?;
        host.editor_state_mut().geometry_end_handle_drag();
        Ok(())
    })
}

/// Check if geometry edit mode is active.
#[no_mangle]
pub unsafe extern "C" fn op_editor_geometry_is_active(engine: *mut crate::OpEngine) -> bool {
    let mut active = false;
    let status = call_session(engine, |session| {
        let host = session.editor_mut()?;
        active = host.editor_state().is_geometry_edit_active();
        Ok(())
    });
    status == OpStatus::Ok && active
}

#[cfg(test)]
mod tests {
    use op_editor_core::geometry_edit::GeometryEditSession;

    #[test]
    fn geometry_session_creation() {
        let session = GeometryEditSession::for_node("n1");
        assert!(session.is_active());
        assert!(session.is_editing("n1"));
    }

    #[test]
    fn geometry_session_exit_clears() {
        let mut session = GeometryEditSession::for_node("n1");
        session.select_anchor(0);
        session.exit();
        assert!(!session.is_active());
        assert!(session.selected_anchor_indices.is_empty());
    }
}
