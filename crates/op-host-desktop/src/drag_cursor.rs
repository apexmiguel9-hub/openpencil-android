//! Where the pointer is DURING a native file-drag.
//!
//! winit's drag events (`HoveredFile` / `DroppedFile`) carry a path and
//! nothing else, and the platform suppresses the normal `CursorMoved` stream
//! while a drag session owns the pointer — so the host's cached cursor is the
//! position from *before* the drag started. Dropping a file "onto a node"
//! needs the live position, which has to come from the window system.
//!
//! macOS reads it from AppKit (`NSEvent.mouseLocation` is current regardless
//! of the window's event stream). Other platforms have no equivalent hook
//! wired yet and report `None`; their callers degrade to the position-free
//! behaviour (open documents, insert at the viewport centre).

/// Live pointer position in the window's LOGICAL, top-left-origin coordinate
/// space — the same space `WindowEvent::CursorMoved` reports, so it can be
/// fed straight into the host's hit-tests.
#[cfg(target_os = "macos")]
pub fn window_cursor_position(window: &winit::window::Window) -> Option<(f32, f32)> {
    use objc2_app_kit::{NSEvent, NSView};
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = window.window_handle().ok()?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return None;
    };
    // SAFETY: raw-window-handle's `ns_view` is the live NSView owned by
    // `window`. This runs synchronously on winit's main thread while handling
    // an event for that window, and the borrow does not outlive the call.
    let ns_view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
    let ns_window = ns_view.window()?;

    // SAFETY: `mouseLocation` is a class method with no receiver state; the
    // remaining conversions are main-thread AppKit geometry calls.
    let screen_point = unsafe { NSEvent::mouseLocation() };
    let window_point = ns_window.convertPointFromScreen(screen_point);
    let view_point = ns_view.convertPoint_fromView(window_point, None);
    // AppKit's default view origin is bottom-left; winit reports top-left.
    let y = if ns_view.isFlipped() {
        view_point.y
    } else {
        ns_view.bounds().size.height - view_point.y
    };
    Some((view_point.x as f32, y as f32))
}

/// No live drag-position hook on this platform — see the module docs.
#[cfg(not(target_os = "macos"))]
pub fn window_cursor_position(_window: &winit::window::Window) -> Option<(f32, f32)> {
    None
}
