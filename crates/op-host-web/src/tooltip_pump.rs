// Browser boundary: requestAnimationFrame scheduling needs browser smoke; the
// CanvasKit bundle gate covers wasm linkability.
//! Wake the shell once a hovering top-bar tooltip comes due.
//!
//! The tooltip waits out a dwell before it appears, and a dwell expiring
//! is not a DOM event. The web shell paints on events only, so without
//! this pump the tooltip would appear only if the user happened to move
//! the mouse again after waiting — the exact bug the native host avoids
//! by reporting the due instant to its winit deadline scheduler
//! (`next_animation_deadline_ms`). This is that scheduler's browser
//! equivalent, kept to the one animation the web host actually needs.
//!
//! Modelled on `agent_indicator_sync::ensure_pump`: a self-terminating
//! `raf_pump` that advances the host clock, repaints, and stops as soon
//! as `next_deadline_ms` reports nothing further is pending — which is
//! the frame the tooltip appeared on, or the frame the cursor left.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::repaint_ctx::RepaintContext;

thread_local! {
    /// One pump per page. `raf_pump` closures are self-freeing, so this
    /// is only a "don't start a second one" latch, not an owner.
    static RUNNING: Cell<bool> = const { Cell::new(false) };
}

/// Start the pump if a tooltip is pending and one isn't already running.
/// Cheap enough to call on every mouse move: the common case is a
/// `next_deadline_ms` of `None` and an immediate return.
pub(crate) fn ensure<C: RepaintContext + 'static>(inner: &Rc<RefCell<C>>) {
    {
        let Ok(borrowed) = inner.try_borrow() else {
            // The caller is mid-event with the host borrowed; that path
            // arms the pump itself once it is done.
            return;
        };
        if !borrowed.host().top_bar_tooltip_pending() {
            // Nothing due — either no hover, a suppressed button, or the
            // tooltip is already on screen.
            return;
        }
    }
    if RUNNING.with(Cell::get) {
        return;
    }
    RUNNING.with(|running| running.set(true));

    let inner = inner.clone();
    crate::raf_pump::start(Rc::new(move || {
        let Ok(mut inner_mut) = inner.try_borrow_mut() else {
            // A DOM event holds the borrow; it repaints itself. Retry
            // next frame rather than dropping the pending tooltip.
            return true;
        };
        // Advance the host clock — the dwell is measured against it.
        let now = crate::listener::now_ms_perf();
        let unix = crate::listener::now_unix_secs();
        inner_mut.host_mut().set_clocks(now, unix);
        let _ = inner_mut.repaint();
        // Paint happens BEFORE this check, so the frame that crosses the
        // dwell draws the tooltip and then retires the pump.
        let alive = inner_mut.host().top_bar_tooltip_pending();
        if !alive {
            RUNNING.with(|running| running.set(false));
        }
        alive
    }));
}
