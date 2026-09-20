//! Host-agnostic collaboration runtime.
//!
//! Owner / guest actors, the admission queue, the relay bootstrap + locator
//! control plane, and the ticket verifier — everything between the editor
//! state and the wire. The runtime is driven by a synchronous caller (a GUI
//! frame pump, a daemon tick) and bridges to its network workers over bounded
//! channels.
//!
//! Two seams keep it free of any concrete host:
//!
//! * [`CollabHost`] — the editor surface it mutates. `HeadlessCollabHost` is
//!   the daemon/test implementation; GUI hosts implement it over their paint
//!   caches.
//! * [`BlockingExecutor`] — the async → sync bridge, installed once per
//!   process by the embedding host.

mod blocking;
mod host;
mod jwks;
mod runtime;

pub use blocking::{install_blocking_executor, BlockingExecutor};
pub use host::{CollabHost, CollabWakeNotifier, HeadlessCollabHost};
pub use runtime::local_edit::LocalEditOutcome;

pub use runtime::types::{CollabRuntimeFailure, CollabStatusEvent};
pub use runtime::CollabRuntime;

// Controlled session fixtures for downstream test suites. Compiled only for
// this crate's own tests or when a downstream dev-dependency turns on
// `test-support`, so a normal build's API surface is unchanged.
#[cfg(any(test, feature = "test-support"))]
pub use runtime::test_support;

#[cfg(test)]
pub(crate) use avatar_test_lock::lock as lock_avatar_test_registry;

/// Serializes the tests that rotate the process-global collaboration-avatar
/// generation in `op_editor_ui::collab_avatar_runtime`.
#[cfg(test)]
mod avatar_test_lock {
    use std::cell::Cell;
    use std::sync::{Mutex, MutexGuard};

    static AVATAR_TEST_LOCK: Mutex<()> = Mutex::new(());

    thread_local! {
        /// How many guards this thread holds. Only the outermost owns the
        /// mutex; the rest are bookkeeping.
        static DEPTH: Cell<usize> = const { Cell::new(0) };
    }

    /// Reentrant guard over the avatar registry.
    ///
    /// Reentrancy is the whole point. `CollabRuntime::advance_generation`
    /// takes this guard itself — the rotation is production code, so the test
    /// call sites cannot cover it — while the tests that assert *on* generation
    /// rotation take it around their whole body. A plain `Mutex` deadlocks the
    /// instant such a test calls into the runtime, which is precisely what
    /// happened. Cross-thread serialization is unchanged.
    pub(crate) struct AvatarTestGuard {
        /// Held purely for its `Drop`; only the outermost guard owns it.
        _mutex: Option<MutexGuard<'static, ()>>,
    }

    impl Drop for AvatarTestGuard {
        fn drop(&mut self) {
            // Runs before the inner guard drops, so the depth is already back
            // to zero by the time another thread can take the mutex.
            DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
        }
    }

    pub(crate) fn lock() -> AvatarTestGuard {
        let outermost = DEPTH.with(|depth| {
            let held = depth.get();
            depth.set(held + 1);
            held == 0
        });
        AvatarTestGuard {
            _mutex: outermost.then(|| AVATAR_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner())),
        }
    }
}
