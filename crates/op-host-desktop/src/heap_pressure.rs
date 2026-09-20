//! Debounced allocator pressure relief after large document transients.

use std::sync::{mpsc, OnceLock};
use std::time::{Duration, Instant};

type RequestSender = mpsc::Sender<&'static str>;

/// Schedule one process-wide allocator scan off the UI thread. The worker uses
/// a trailing-edge debounce: every later page switch resets the quiet period,
/// so a burst cannot scan while its final old scene is still dropping. The
/// allocator API never releases live allocations.
pub(crate) fn schedule_relief(reason: &'static str) {
    static SENDER: OnceLock<RequestSender> = OnceLock::new();
    let sender = SENDER.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("op-heap-relief".into())
            .spawn(move || relief_worker(rx))
            .expect("spawn allocator pressure-relief worker");
        tx
    });
    let _ = sender.send(reason);
}

fn relief_worker(rx: mpsc::Receiver<&'static str>) {
    while let Ok(mut reason) = rx.recv() {
        loop {
            match rx.recv_timeout(Duration::from_millis(750)) {
                Ok(newer_reason) => reason = newer_reason,
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
        let started = Instant::now();
        let released = platform_pressure_relief();
        if released > 0 || cfg!(debug_assertions) {
            eprintln!(
                "[memory] allocator relief after {reason}: {released} bytes in {:.1} ms",
                started.elapsed().as_secs_f64() * 1000.0
            );
        }
    }
}

#[cfg(target_os = "macos")]
fn platform_pressure_relief() -> usize {
    use std::ffi::c_void;

    extern "C" {
        fn malloc_zone_pressure_relief(zone: *mut c_void, goal: usize) -> usize;
    }

    // SAFETY: Apple's malloc API explicitly accepts a null zone to inspect all
    // zones and a zero goal to request maximal relief. It only unmaps pages the
    // allocator already considers free.
    unsafe { malloc_zone_pressure_relief(std::ptr::null_mut(), 0) }
}

#[cfg(not(target_os = "macos"))]
fn platform_pressure_relief() -> usize {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pressure_relief_is_safe_to_request_without_large_allocations() {
        let _released = platform_pressure_relief();
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn unsupported_platforms_are_a_noop() {
        assert_eq!(platform_pressure_relief(), 0);
    }
}
