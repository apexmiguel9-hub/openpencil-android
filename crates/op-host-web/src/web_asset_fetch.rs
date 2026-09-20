//! Browser transport for the runtime-fetched product assets.
//!
//! The widget layer is platform-free: when a card needs a preview that is not
//! in the bundle it calls `op_editor_core::web_assets::request(route)` and
//! paints its placeholder. This module is the other half — it drains those
//! requests once per frame, fetches each over XHR, and installs the bytes back
//! into the registry so the next paint finds them.
//!
//! Three properties matter, and all three are the registry's, not this file's:
//! single-flight (a forty-card grid produces forty requests, not forty per
//! frame), exactly-one-answer (every drained route gets `install` or
//! `mark_failed`, so nothing stays `Pending` forever), and graceful failure
//! (an unavailable asset degrades to a placeholder — never a panic, never a
//! spinner that outlives the session).

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

/// How many assets may be in flight from one drain.
///
/// The Prompt Center opens with dozens of cards visible; firing every request
/// at once buries the daemon's connection pool behind a burst nobody is
/// looking at yet. A small batch per frame keeps the visible rows filling in
/// first, and the queue is drained again next frame.
const MAX_IN_FLIGHT_PER_DRAIN: usize = 6;

/// Abandon a request after this long. A hung asset must not hold a route in
/// `Pending` forever — that would leave its card on a placeholder with no
/// retry.
const FETCH_TIMEOUT_MS: u32 = 20_000;

/// Why an asset fetch did not produce bytes.
///
/// Typed rather than a string because each variant is a different operational
/// story: no XHR at all is a hostile embedding, a non-2xx is a bundle that was
/// deployed without its assets, and a timeout is a slow link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WebAssetFetchError {
    /// `XMLHttpRequest` could not be constructed.
    XhrUnavailable,
    /// `open()` was rejected — a malformed route.
    RequestOpenFailed,
    /// `send()` was rejected.
    RequestSendFailed,
    /// The response arrived with a non-2xx status (0 = network / timeout).
    Http(u16),
    /// A 2xx with no readable body.
    EmptyBody,
}

impl std::fmt::Display for WebAssetFetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::XhrUnavailable => write!(f, "XMLHttpRequest is unavailable"),
            Self::RequestOpenFailed => write!(f, "could not open the asset request"),
            Self::RequestSendFailed => write!(f, "could not send the asset request"),
            Self::Http(status) => write!(f, "asset request failed with status {status}"),
            Self::EmptyBody => write!(f, "asset response carried no body"),
        }
    }
}

/// Drain whatever the widget layer asked for and fetch it.
///
/// Called once per paint. Cheap when idle: one lock and a length test. It
/// takes no host handle on purpose — the install path wakes the editor through
/// `repaint_coalescer`, the same free-function seam the agent-indicator relay
/// uses, so this module never has to borrow a host that a DOM event may
/// already hold.
pub(crate) fn drain_pending() {
    for route in op_editor_core::web_assets::take_pending_requests(MAX_IN_FLIGHT_PER_DRAIN) {
        fetch_asset(route);
    }
}

fn fetch_asset(route: String) {
    let url = crate::daemon_base::daemon_url(&route);
    fetch_bytes(&url, move |result| match result {
        Ok(bytes) => {
            if op_editor_core::web_assets::install(&route, bytes) {
                // The icon catalog is not consumed as raw bytes: it has to be
                // parsed into the shared catalog before any lookup can see it.
                // Done here, once, on the install edge.
                if route == op_editor_ui::ICONIFY_CORE_ROUTE {
                    if let Some(json) = op_editor_core::web_assets::installed_str(&route) {
                        op_editor_ui::set_core_catalog(json);
                    }
                }
                // Wake the editor so the card showing a placeholder picks the
                // picture up; the response is not an input event, so nothing
                // else would.
                crate::repaint_coalescer::request();
            }
        }
        Err(_error) => {
            // Degrade, do not retry in place: `mark_failed` leaves the route
            // retryable, so reopening the panel asks again. Spinning here would
            // hammer a daemon that simply is not serving assets.
            op_editor_core::web_assets::mark_failed(&route);
        }
    });
}

type DoneFn = Box<dyn FnOnce(Result<Vec<u8>, WebAssetFetchError>)>;

/// Fire a GET for binary content and hand the body (or an error) to `on_done`
/// exactly once.
///
/// The callback is slot-wrapped so the synchronous failure paths still resolve
/// it — a dropped callback would strand its route in `Pending`, which is the
/// one state the registry cannot recover from on its own.
pub(crate) fn fetch_bytes(
    url: &str,
    on_done: impl FnOnce(Result<Vec<u8>, WebAssetFetchError>) + 'static,
) {
    let slot: Rc<RefCell<Option<DoneFn>>> = Rc::new(RefCell::new(Some(Box::new(on_done))));
    let resolve = |slot: &Rc<RefCell<Option<DoneFn>>>, result| {
        if let Some(done) = slot.borrow_mut().take() {
            done(result);
        }
    };
    let Ok(xhr) = web_sys::XmlHttpRequest::new() else {
        resolve(&slot, Err(WebAssetFetchError::XhrUnavailable));
        return;
    };
    if xhr.open_with_async("GET", url, true).is_err() {
        resolve(&slot, Err(WebAssetFetchError::RequestOpenFailed));
        return;
    }
    // These are JPEGs and `.op` documents; `response_text` would mangle the
    // former, so the response is read as an ArrayBuffer and copied out.
    xhr.set_response_type(web_sys::XmlHttpRequestResponseType::Arraybuffer);
    // The assets live under the daemon's `/pkg/` route, so in managed mode the
    // bridge token has to ride along exactly as it does for every other daemon
    // call. `attach_daemon_headers` decides that from the URL.
    crate::live_sync::attach_daemon_headers(&xhr, url);
    xhr.set_timeout(FETCH_TIMEOUT_MS);
    let xhr_cb = xhr.clone();
    let slot_cb = slot.clone();
    let onloadend = Closure::<dyn FnMut()>::once_into_js(move || {
        let status = xhr_cb.status().unwrap_or(0);
        let result = if (200..300).contains(&status) {
            match xhr_cb.response() {
                Ok(value) if !value.is_null() && !value.is_undefined() => {
                    let buffer = js_sys::Uint8Array::new(&value);
                    let mut bytes = vec![0u8; buffer.length() as usize];
                    buffer.copy_to(&mut bytes);
                    if bytes.is_empty() {
                        Err(WebAssetFetchError::EmptyBody)
                    } else {
                        Ok(bytes)
                    }
                }
                _ => Err(WebAssetFetchError::EmptyBody),
            }
        } else {
            Err(WebAssetFetchError::Http(status))
        };
        if let Some(done) = slot_cb.borrow_mut().take() {
            done(result);
        }
    });
    xhr.set_onloadend(Some(onloadend.unchecked_ref()));
    if xhr.send().is_err() {
        resolve(&slot, Err(WebAssetFetchError::RequestSendFailed));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_editor_core::web_assets::{self, WebAssetState};

    /// Serialises against the process-global asset registry.
    fn lock_registry() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    #[test]
    fn every_failure_variant_reads_as_its_own_operational_story() {
        // Each is a different thing to go and fix, which is the whole reason
        // this is an enum rather than a bool.
        let messages = [
            WebAssetFetchError::XhrUnavailable.to_string(),
            WebAssetFetchError::RequestOpenFailed.to_string(),
            WebAssetFetchError::RequestSendFailed.to_string(),
            WebAssetFetchError::Http(404).to_string(),
            WebAssetFetchError::EmptyBody.to_string(),
        ];
        let unique: std::collections::HashSet<_> = messages.iter().collect();
        assert_eq!(unique.len(), messages.len());
        assert!(messages[3].contains("404"));
    }

    #[test]
    fn a_drain_is_bounded_so_one_frame_cannot_open_every_socket() {
        // The bound is what keeps a freshly opened Prompt Center from firing
        // 57 sockets in a single frame.
        let _guard = lock_registry();
        for index in 0..(MAX_IN_FLIGHT_PER_DRAIN + 4) {
            web_assets::request(&format!("/pkg/assets/drain-bound-{index}.jpg"));
        }
        assert_eq!(
            web_assets::take_pending_requests(MAX_IN_FLIGHT_PER_DRAIN).len(),
            MAX_IN_FLIGHT_PER_DRAIN
        );
        assert!(web_assets::has_pending_requests(), "the rest wait a frame");
        // Leave the shared registry clean for other tests.
        for route in web_assets::take_pending_requests(usize::MAX) {
            web_assets::mark_failed(&route);
        }
        for index in 0..MAX_IN_FLIGHT_PER_DRAIN {
            web_assets::mark_failed(&format!("/pkg/assets/drain-bound-{index}.jpg"));
        }
    }

    #[test]
    fn a_failed_asset_degrades_and_can_be_asked_for_again() {
        // This is the contract the paint sites depend on: a failure must leave
        // the card on its placeholder AND leave the door open, never wedge the
        // route in `Pending`.
        let _guard = lock_registry();
        let route = "/pkg/assets/web-asset-fetch-failure.jpg";

        web_assets::request(route);
        let drained = web_assets::take_pending_requests(usize::MAX);
        assert!(drained.iter().any(|r| r == route));

        web_assets::mark_failed(route);
        assert_eq!(web_assets::state(route), WebAssetState::Failed);
        assert!(web_assets::installed_bytes(route).is_none());

        web_assets::request(route);
        assert_eq!(web_assets::state(route), WebAssetState::Pending);
        for r in web_assets::take_pending_requests(usize::MAX) {
            web_assets::mark_failed(&r);
        }
    }
}
