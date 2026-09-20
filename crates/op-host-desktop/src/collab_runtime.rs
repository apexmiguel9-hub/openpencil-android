//! Desktop shim over the extracted collaboration runtime.
//!
//! The runtime itself lives in `op-collab-host`, which is host-agnostic: it
//! knows neither winit nor `op-host-services`. This module supplies the two
//! desktop-specific pieces it asks for — the async → sync bridge and the
//! event-loop wake notifier — and keeps the `DesktopCollabRuntime` name the
//! rest of the binary already uses.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use op_collab_host::{BlockingExecutor, CollabWakeNotifier};
use winit::event_loop::EventLoopProxy;

use crate::DesktopEvent;

pub(crate) type DesktopCollabRuntime = op_collab_host::CollabRuntime;

/// Routes the runtime's relay-bridge and JWKS futures to the workspace's
/// single sanctioned async → sync bridge.
struct ServicesBlockingExecutor;

impl BlockingExecutor for ServicesBlockingExecutor {
    fn block_on_erased(&self, future: Pin<&mut (dyn Future<Output = ()> + '_)>) {
        op_host_services::chat_runtime::block_on_anywhere(future);
    }
}

/// Install the process-wide blocking executor. Idempotent — the first install
/// wins, so calling it from more than one entry point is safe.
pub(crate) fn install_blocking_executor() {
    let _ = op_collab_host::install_blocking_executor(Arc::new(ServicesBlockingExecutor));
}

/// Wrap the winit event-loop proxy as the runtime's wake notifier.
pub(crate) fn wake_notifier(proxy: EventLoopProxy<DesktopEvent>) -> CollabWakeNotifier {
    Arc::new(move || {
        let _ = proxy.send_event(DesktopEvent::CollabWake);
    })
}

#[cfg(test)]
mod collab_host_tests;
