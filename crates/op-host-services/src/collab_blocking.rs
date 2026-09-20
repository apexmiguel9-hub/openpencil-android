//! The blocking executor injected into `op-collab-host`.
//!
//! The collaboration runtime is synchronous but its relay bridges and JWKS
//! fetch are futures, so it asks its host for an async → sync bridge rather
//! than owning one. This crate owns
//! [`block_on_anywhere`](crate::chat_runtime::block_on_anywhere), the
//! workspace's single sanctioned bridge, so the implementation lives here and
//! every host — the desktop GUI and the headless daemon alike — installs this
//! one rather than writing its own.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use op_collab_host::BlockingExecutor;

struct ServicesBlockingExecutor;

impl BlockingExecutor for ServicesBlockingExecutor {
    fn block_on_erased(&self, future: Pin<&mut (dyn Future<Output = ()> + '_)>) {
        crate::chat_runtime::block_on_anywhere(future);
    }
}

/// Install the process-wide executor.
///
/// Idempotent — the first install wins — so every entry point that might be
/// the first to start a session can call it unconditionally.
pub fn install() {
    let _ = op_collab_host::install_blocking_executor(Arc::new(ServicesBlockingExecutor));
}
