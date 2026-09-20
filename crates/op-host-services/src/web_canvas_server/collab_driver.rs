//! The daemon's collaboration pump.
//!
//! The desktop drives its runtime from the winit frame loop. A daemon has no
//! frames, so it gets a dedicated thread that ticks on a timeout and is woken
//! early by anything that gives the runtime work: a network event (through the
//! runtime's own wake notifier) or a browser POST.
//!
//! ## Locking
//!
//! The tick takes the state lock for its whole body, and the wake notifier
//! never takes it — it only pokes a bounded channel. That is what keeps a
//! network worker thread from blocking on the document mutex while the REST
//! tier is serving a large document push.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{SseHub, WebCanvasState};

/// Tick cadence with no live session. Long enough to cost nothing, short
/// enough that a start/join action posted just after a tick still feels
/// immediate.
const IDLE_TICK: Duration = Duration::from_millis(250);

/// Tick cadence during a session, matching the presence frame budget.
const ACTIVE_TICK: Duration = Duration::from_millis(100);

/// Pokes the driver awake from any thread.
///
/// A one-slot channel: the driver only needs to know that *something*
/// happened, so a wake arriving while one is already pending is dropped rather
/// than queued. `try_send` never blocks, which is what makes this safe to call
/// from a network worker holding transport locks.
pub(crate) struct CollabWaker {
    tx: SyncSender<()>,
}

impl CollabWaker {
    pub(crate) fn wake(&self) {
        let _ = self.tx.try_send(());
    }
}

/// Start the collaboration driver for this daemon.
///
/// Installs the blocking executor the runtime needs, registers the wake path,
/// and spawns the pump. The thread observes `shutdown` on every tick, so it
/// retires within one tick of the daemon being asked to stop.
pub(crate) fn spawn(
    state: &Arc<Mutex<WebCanvasState>>,
    hub: &Arc<SseHub>,
    shutdown: &Arc<AtomicBool>,
) {
    crate::collab_blocking::install();

    let (tx, rx) = sync_channel(1);
    let waker = Arc::new(CollabWaker { tx });
    {
        let mut guard = state.lock().unwrap_or_else(|p| p.into_inner());
        guard.collab.set_waker(Arc::clone(&waker));
        let notifier_waker = Arc::clone(&waker);
        guard
            .collab
            .runtime
            .set_wake_notifier(Arc::new(move || notifier_waker.wake()));
    }

    let state = Arc::clone(state);
    let hub = Arc::clone(hub);
    let shutdown = Arc::clone(shutdown);
    let spawned = std::thread::Builder::new()
        .name("op-serve-web-collab".into())
        .spawn(move || run(&state, &hub, &shutdown, &rx));
    if spawned.is_err() {
        eprintln!(
            "openpencil-desktop --serve-web: could not start the collaboration driver; \
             collaboration stays unavailable"
        );
    }
}

fn run(state: &Mutex<WebCanvasState>, hub: &SseHub, shutdown: &AtomicBool, wakes: &Receiver<()>) {
    while !shutdown.load(Ordering::Acquire) {
        let active = tick(state, hub);
        let timeout = if active { ACTIVE_TICK } else { IDLE_TICK };
        // A timeout and a wake are the same signal here; the tick is
        // idempotent, so a spurious wake only costs one pass.
        let _ = wakes.recv_timeout(timeout);
    }
}

/// One pass of the runtime pump. Returns whether a session is live, which
/// selects the next tick cadence.
fn tick(state: &Mutex<WebCanvasState>, hub: &SseHub) -> bool {
    let mut guard = state.lock().unwrap_or_else(|p| p.into_inner());
    let guard = &mut *guard;

    let revision_before = guard.editor.document_revision();
    let presence_cursor = guard.collab.presence_override();

    let (runtime, mut host) = guard.collab_runtime_and_host();
    // Order matches the desktop frame pump: availability first so the panel
    // never offers an action the runtime would refuse, then the queued action,
    // then inbound network events, then outbound presence.
    let mut projection_changed = runtime.refresh_availability(&mut host);
    projection_changed |= runtime.drain_ui_action(&mut host);
    projection_changed |= runtime.poll(&mut host);
    projection_changed |= runtime.publish_local_presence(&mut host, presence_cursor);

    for status in runtime.drain_status_events() {
        eprintln!("[collab] {status:?}");
    }

    // The host dirty flag is raised by every runtime write through the seam,
    // including pure projection writes that change no document content.
    projection_changed |= guard.collab.take_dirty();

    let revision_after = guard.editor.document_revision();
    if revision_after != revision_before {
        // Document content moved (a remote commit landed, or a local edit was
        // accepted). Only this bumps the document version, so a browser
        // refetches the document for content changes and nothing else.
        guard.version = guard.version.saturating_add(1);
        projection_changed = true;
    }
    if projection_changed {
        guard.collab.bump_seq();
        // Broadcast inside the lock, matching the REST tier's contract: the
        // bump and its broadcast must be atomic or two mutations can publish
        // out of order.
        hub.broadcast(guard.sse_tick());
    }

    guard.collab_session_is_active()
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_editor_core::EditorState;

    fn daemon() -> Mutex<WebCanvasState> {
        Mutex::new(WebCanvasState::new(EditorState::starter(), 0))
    }

    #[test]
    fn an_idle_tick_publishes_nothing_after_the_first_settling_pass() {
        let state = daemon();
        let hub = SseHub::default();
        let sub = hub.subscribe();

        // The first pass may publish the initial availability projection.
        assert!(!tick(&state, &hub));
        let settled = state.lock().unwrap_or_else(|p| p.into_inner()).collab.seq();
        sub.pending();

        // Every pass after that is silent, which is what makes a 250 ms idle
        // cadence free: no bump, no broadcast, no browser refetch.
        for _ in 0..3 {
            assert!(!tick(&state, &hub));
        }
        assert_eq!(
            state.lock().unwrap_or_else(|p| p.into_inner()).collab.seq(),
            settled
        );
        assert!(sub.pending().is_none(), "an idle daemon must stay quiet");
    }

    #[test]
    fn a_host_write_bumps_only_the_projection_sequence_and_broadcasts_it() {
        let state = daemon();
        let hub = SseHub::default();
        let sub = hub.subscribe();
        tick(&state, &hub);
        sub.pending();

        let (version_before, seq_before) = {
            let mut guard = state.lock().unwrap_or_else(|p| p.into_inner());
            let before = (guard.version, guard.collab.seq());
            // Stand in for a runtime write that touched only the projection —
            // a peer's cursor moving, say.
            let (_runtime, mut host) = guard.collab_runtime_and_host();
            op_collab_host::CollabHost::mark_editor_state_dirty(&mut host);
            before
        };

        assert!(!tick(&state, &hub));
        let guard = state.lock().unwrap_or_else(|p| p.into_inner());
        assert_eq!(
            guard.version, version_before,
            "presence-shaped changes must not move the document version, or \
             every cursor move would make the browser refetch the document"
        );
        assert_eq!(guard.collab.seq(), seq_before + 1);
        let tick_out = sub.pending().expect("the change was broadcast");
        assert_eq!(tick_out.version, version_before);
        assert_eq!(tick_out.collab_seq, seq_before + 1);
    }

    #[test]
    fn a_write_that_landed_between_ticks_is_not_bumped_a_second_time() {
        let state = daemon();
        let hub = SseHub::default();
        let sub = hub.subscribe();
        tick(&state, &hub);
        sub.pending();

        // A REST push mutated the document and published its own version while
        // the driver was asleep.
        let version_before = {
            let mut guard = state.lock().unwrap_or_else(|p| p.into_inner());
            guard.editor.mark_document_changed();
            guard.version = guard.version.saturating_add(1);
            guard.version
        };

        // The driver compares the revision across its own tick, so a change
        // that landed before the tick started is invisible to it. That is what
        // keeps one logical mutation from publishing two versions and making
        // every browser fetch the same document twice.
        tick(&state, &hub);
        assert_eq!(
            state.lock().unwrap_or_else(|p| p.into_inner()).version,
            version_before
        );
        assert!(sub.pending().is_none(), "nothing new to announce");
    }

    #[test]
    fn the_tick_drains_a_queued_action_from_the_pending_slot() {
        let state = daemon();
        let hub = SseHub::default();
        {
            let mut guard = state.lock().unwrap_or_else(|p| p.into_inner());
            guard.editor.editor_ui.collab.pending_action =
                Some(op_editor_core::CollabUiAction::OpenCreate);
        }

        tick(&state, &hub);

        let guard = state.lock().unwrap_or_else(|p| p.into_inner());
        assert!(
            guard.editor.editor_ui.collab.pending_action.is_none(),
            "the driver is what frees the single action slot; a stuck action \
             would 409 every later post"
        );
    }
}
