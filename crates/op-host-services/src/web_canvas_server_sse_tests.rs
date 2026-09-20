//! SSE fan-out tests — the hub's latest-value slots and the stream writer.
//!
//! Split out of `web_canvas_server_conn_tests.rs` at the 800-line cap; nested
//! under it so `use super::*` still reaches the mock stream and helpers.

use super::*;

#[test]
fn sse_hub_broadcasts_version_to_all_subscribers() {
    let hub = SseHub::default();
    let a = hub.subscribe();
    let b = hub.subscribe();
    hub.broadcast(tick(5, 0));
    assert_eq!(a.pending().expect("published"), tick(5, 0));
    assert_eq!(b.pending().expect("published"), tick(5, 0));
}

#[test]
fn sse_hub_prunes_disconnected_subscribers() {
    let hub = SseHub::default();
    let live = hub.subscribe();
    drop(hub.subscribe()); // a disconnected client (receiver dropped)
    assert_eq!(hub.subscriber_count(), 2);
    hub.broadcast(tick(1, 0)); // prunes the dropped one
    assert_eq!(hub.subscriber_count(), 1);
    assert_eq!(live.pending().expect("published"), tick(1, 0));
}

#[test]
fn write_sse_event_emits_data_frame() {
    let mut stream = mock_stream("");
    write_sse_event(&mut stream, tick(42, 7)).expect("write");
    assert_eq!(
        String::from_utf8_lossy(&stream.output),
        "data: {\"version\":42,\"collabSeq\":7}\n\n"
    );
}

#[test]
fn sse_payload_stays_a_superset_of_the_original_version_frame() {
    let mut stream = mock_stream("");
    write_sse_event(&mut stream, tick(3, 0)).expect("write");
    let out = String::from_utf8_lossy(&stream.output).into_owned();
    // A client written against the original `{"version":N}` frame parses this
    // one unchanged: `version` keeps its spelling and stays the first field.
    assert!(out.starts_with("data: {\"version\":3,"), "{out}");
    let payload: serde_json::Value =
        serde_json::from_str(out.trim_start_matches("data: ").trim()).expect("valid JSON");
    assert_eq!(payload["version"], 3);
    assert_eq!(payload["collabSeq"], 0);
}

#[test]
fn serve_sse_emits_the_initial_tick_then_each_published_one() {
    // The stream ends when a socket write fails, which is how a disconnected
    // client is detected — so this writes to a stream that fails on the
    // second event.
    let hub = SseHub::default();
    let slot = hub.subscribe();
    hub.broadcast(tick(9, 0));
    let mut stream = FailingStream {
        written: Vec::new(),
        writes_before_failure: 3,
    };
    let _ = serve_sse(&mut stream, &slot, tick(7, 0), Some("*"));
    let out = String::from_utf8_lossy(&stream.written);
    assert!(out.contains("text/event-stream"), "{out}");
    assert!(out.contains(r#"data: {"version":7,"#), "{out}"); // initial sync
    assert!(out.contains(r#"data: {"version":9,"#), "{out}"); // published bump
}

/// A stream that stops accepting writes, standing in for a client that went
/// away — which is the only thing that ends an SSE loop.
struct FailingStream {
    written: Vec<u8>,
    writes_before_failure: usize,
}

impl std::io::Write for FailingStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.writes_before_failure == 0 {
            return Err(std::io::Error::other("client went away"));
        }
        self.writes_before_failure -= 1;
        self.written.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn a_subscriber_that_never_reads_keeps_exactly_one_pending_tick() {
    // The unbounded-queue bug: a paused tab accumulated one entry per
    // mutation, in a process shared with every other account.
    let hub = SseHub::default();
    let slot = hub.subscribe();
    for version in 0..1000 {
        hub.broadcast(tick(version, 0));
    }
    // Only the newest survives — an older tick is not information the client
    // lost, it is information the newest one already contains.
    assert_eq!(slot.pending(), Some(tick(999, 0)));
    assert_eq!(slot.pending(), None, "taking it leaves the slot empty");
}

#[test]
fn a_dropped_subscriber_is_pruned_without_signalling_anything() {
    let hub = SseHub::default();
    let live = hub.subscribe();
    drop(hub.subscribe());
    assert_eq!(hub.subscriber_count(), 2);
    hub.broadcast(tick(1, 0));
    assert_eq!(hub.subscriber_count(), 1);
    assert_eq!(live.pending(), Some(tick(1, 0)));
}

#[test]
fn subscribing_prunes_dead_slots_even_when_nothing_is_ever_broadcast() {
    // A tenant whose clients all disconnected and which never publishes again
    // would otherwise accumulate one dead `Weak` per reconnect, forever.
    let hub = SseHub::default();
    for _ in 0..100 {
        drop(hub.subscribe());
    }
    // Each subscribe prunes the previous corpse, so at most the live one plus
    // the one just added remain.
    assert!(
        hub.subscriber_count() <= 2,
        "dead subscribers accumulated: {}",
        hub.subscriber_count()
    );
}

#[test]
fn broadcasting_to_no_subscribers_is_a_no_op() {
    let hub = SseHub::default();
    hub.broadcast(tick(1, 0));
    assert_eq!(hub.subscriber_count(), 0);
}
