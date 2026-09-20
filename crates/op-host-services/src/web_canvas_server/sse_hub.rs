//! Latest-value fan-out for the SSE update stream.
//!
//! Split out of the `web_canvas_server` spine at the 800-line cap. The whole
//! design is one decision — a subscriber gets a slot, not a queue — and the
//! reasoning for it lives on `SseSlot`.

use std::sync::{Arc, Condvar, Mutex, Weak};
use std::time::Duration;

use super::SseTick;

/// One subscriber's latest-value slot.
///
/// Deliberately NOT a queue. The client re-reads whatever the counters point
/// at, so only the newest tick has any meaning — an older one is not
/// information the client lost, it is information the newer one already
/// contains. A queue therefore buys nothing and costs the one thing that
/// matters: a subscriber that stops reading (a paused tab, a stalled socket,
/// a laptop that slept) accumulates one entry per mutation, unbounded, in a
/// process shared with every other account.
///
/// So the slot holds exactly one tick and a publisher overwrites it. A burst
/// of a thousand mutations against a subscriber that never wakes leaves one
/// tick behind, not a thousand.
pub(crate) struct SseSlot {
    latest: Mutex<Option<SseTick>>,
    ready: Condvar,
}

impl SseSlot {
    fn new() -> Self {
        Self {
            latest: Mutex::new(None),
            ready: Condvar::new(),
        }
    }

    /// Overwrite the pending tick and wake the waiter.
    fn publish(&self, tick: SseTick) {
        *self.latest.lock().unwrap_or_else(|p| p.into_inner()) = Some(tick);
        self.ready.notify_one();
    }

    /// Take the pending tick, waiting up to `timeout` for one.
    ///
    /// `None` means the timeout elapsed with nothing published — the caller's
    /// cue to emit a heartbeat, which is also how a disconnected socket is
    /// noticed.
    pub(crate) fn take_latest(&self, timeout: Duration) -> Option<SseTick> {
        let guard = self.latest.lock().unwrap_or_else(|p| p.into_inner());
        let (mut guard, _) = self
            .ready
            .wait_timeout_while(guard, timeout, |latest| latest.is_none())
            .unwrap_or_else(|p| p.into_inner());
        guard.take()
    }

    /// Take the pending tick without waiting. Tests only.
    #[cfg(test)]
    pub(crate) fn pending(&self) -> Option<SseTick> {
        self.latest.lock().unwrap_or_else(|p| p.into_inner()).take()
    }
}

/// Broadcast hub for SSE subscribers. Each `GET /api/mcp/events` connection
/// registers a slot; a document mutation publishes the new tick into every
/// one of them, and each SSE connection thread writes it to its socket.
///
/// Subscribers are held weakly, so a connection that ended is pruned on the
/// next broadcast without needing the subscriber to signal anything.
#[derive(Default)]
pub struct SseHub {
    subscribers: Mutex<Vec<Weak<SseSlot>>>,
}

impl SseHub {
    /// Register a subscriber; the SSE connection thread blocks on the returned
    /// slot for ticks. Dropping it unregisters.
    pub(crate) fn subscribe(&self) -> Arc<SseSlot> {
        let slot = Arc::new(SseSlot::new());
        let mut subscribers = self.subscribers.lock().unwrap_or_else(|p| p.into_inner());
        // Prune here too, not only on broadcast: a tenant whose clients all
        // disconnected and which then never publishes again would otherwise
        // accumulate one dead `Weak` per reconnect, forever.
        subscribers.retain(|slot| slot.strong_count() > 0);
        subscribers.push(Arc::downgrade(&slot));
        slot
    }

    /// Publish a tick to all live subscribers, pruning any whose connection
    /// ended (their `Arc` was dropped).
    pub(crate) fn broadcast(&self, tick: SseTick) {
        self.subscribers
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .retain(|slot| {
                let Some(slot) = slot.upgrade() else {
                    return false;
                };
                slot.publish(tick);
                true
            });
    }

    #[cfg(test)]
    pub(crate) fn subscriber_count(&self) -> usize {
        self.subscribers
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .len()
    }
}
