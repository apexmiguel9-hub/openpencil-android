//! Injected async → sync bridge.
//!
//! The collaboration runtime is synchronous end to end, but the relay bridges
//! and the JWKS trust-root fetch are futures. Rather than owning an executor —
//! which would bind this crate to one host's runtime policy and drag a host
//! crate into its dependency graph — the embedding host installs one.
//!
//! `op-host-desktop` installs an implementation that forwards to
//! `op_host_services::chat_runtime::block_on_anywhere`, the workspace's single
//! sanctioned bridge.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};

/// Drives a future to completion on the calling thread.
pub trait BlockingExecutor: Send + Sync {
    /// Run `future` to completion.
    ///
    /// The future is deliberately neither `Send` nor `'static`: several call
    /// sites hold borrows across await points, so an implementation must block
    /// the calling thread rather than offload the work elsewhere.
    fn block_on_erased(&self, future: Pin<&mut (dyn Future<Output = ()> + '_)>);
}

static EXECUTOR: OnceLock<Arc<dyn BlockingExecutor>> = OnceLock::new();

/// Install the process-wide executor.
///
/// Returns `false` when one was already installed — the first install wins, so
/// calling this repeatedly (or from several entry points) is harmless.
pub fn install_blocking_executor(executor: Arc<dyn BlockingExecutor>) -> bool {
    EXECUTOR.set(executor).is_ok()
}

/// Drive `future` to completion through the installed executor.
pub(crate) fn block_on<F: Future>(future: F) -> F::Output {
    let executor = installed_executor();
    let mut output = None;
    {
        let mut capture = std::pin::pin!(async {
            output = Some(future.await);
        });
        executor.block_on_erased(capture.as_mut());
    }
    output.expect("a blocking executor must drive the future to completion")
}

fn installed_executor() -> &'static dyn BlockingExecutor {
    #[cfg(test)]
    test_support::install();
    match EXECUTOR.get() {
        Some(executor) => Arc::as_ref(executor),
        None => panic!(
            "op-collab-host has no blocking executor; the host must call \
             op_collab_host::install_blocking_executor before starting a session"
        ),
    }
}

/// Test-only executor so this crate's tests can exercise the async paths
/// without depending on a host crate. Production hosts inject their own.
#[cfg(test)]
mod test_support {
    use super::{install_blocking_executor, Arc, BlockingExecutor, Future, OnceLock, Pin};
    use tokio::runtime::{Builder, Handle, Runtime, RuntimeFlavor};

    struct TestExecutor;

    impl BlockingExecutor for TestExecutor {
        fn block_on_erased(&self, future: Pin<&mut (dyn Future<Output = ()> + '_)>) {
            match Handle::try_current() {
                Ok(handle) if handle.runtime_flavor() == RuntimeFlavor::CurrentThread => {
                    panic!("tests must not block on a current-thread runtime's only worker")
                }
                Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
                Err(_) => runtime().block_on(future),
            }
        }
    }

    fn runtime() -> &'static Runtime {
        static RUNTIME: OnceLock<Runtime> = OnceLock::new();
        RUNTIME.get_or_init(|| {
            Builder::new_multi_thread()
                .enable_all()
                .thread_name("op-collab-host-test")
                .build()
                .expect("test runtime build")
        })
    }

    pub(super) fn install() {
        let _ = install_blocking_executor(Arc::new(TestExecutor));
    }
}
