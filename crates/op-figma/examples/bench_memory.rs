//! Allocation and resident-memory probe for the binary Figma import pipeline.
//!
//! ```text
//! cargo run -p op-figma --release --example bench_memory -- \
//!   prepare|page|all|state|active|switch /path/to/file.fig [page-index]
//! ```

use op_figma::{prepare_fig_binary, FigLayoutMode};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct CountingAllocator;

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn observe_peak(current: usize) {
    let mut peak = PEAK.load(Ordering::Relaxed);
    while current > peak {
        match PEAK.compare_exchange_weak(peak, current, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(actual) => peak = actual,
        }
    }
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            let current = CURRENT.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            observe_peak(current);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
        CURRENT.fetch_sub(layout.size(), Ordering::Relaxed);
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let replacement = unsafe { System.realloc(pointer, layout, new_size) };
        if replacement.is_null() {
            return replacement;
        }
        let current = if new_size >= layout.size() {
            let delta = new_size - layout.size();
            CURRENT.fetch_add(delta, Ordering::Relaxed) + delta
        } else {
            let delta = layout.size() - new_size;
            CURRENT.fetch_sub(delta, Ordering::Relaxed) - delta
        };
        observe_peak(current);
        replacement
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

#[cfg(target_os = "macos")]
fn max_rss() -> usize {
    #[repr(C)]
    #[derive(Default)]
    struct TimeVal {
        sec: i64,
        usec: i32,
        padding: i32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct RUsage {
        user: TimeVal,
        system: TimeVal,
        max_rss: i64,
        integral_shared: i64,
        integral_unshared_data: i64,
        integral_unshared_stack: i64,
        minor_faults: i64,
        major_faults: i64,
        swaps: i64,
        input_blocks: i64,
        output_blocks: i64,
        messages_sent: i64,
        messages_received: i64,
        signals_received: i64,
        voluntary_context_switches: i64,
        involuntary_context_switches: i64,
    }

    unsafe extern "C" {
        fn getrusage(who: i32, usage: *mut RUsage) -> i32;
    }

    let mut usage = RUsage::default();
    if unsafe { getrusage(0, &mut usage) } == 0 {
        usage.max_rss.max(0) as usize
    } else {
        0
    }
}

#[cfg(not(target_os = "macos"))]
fn max_rss() -> usize {
    0
}

fn report(stage: &str) {
    eprintln!(
        "MEM stage={stage} heap_current={} heap_peak={} max_rss={}",
        CURRENT.load(Ordering::Relaxed),
        PEAK.load(Ordering::Relaxed),
        max_rss()
    );
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mode = args
        .next()
        .expect("mode: prepare|page|all|state|active|switch");
    let path = args.next().expect("path to .fig");
    let page = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    report("start");
    let bytes = std::fs::read(path).expect("read .fig");
    report("read");

    let prepared =
        prepare_fig_binary(&bytes, "Memory Probe", FigLayoutMode::Preserve).expect("prepare");
    report("prepare");
    match mode.as_str() {
        "prepare" => {
            std::hint::black_box(&prepared);
        }
        "page" => {
            let import = prepared.into_page(page).expect("page conversion");
            std::hint::black_box(&import);
            report("page");
        }
        "all" => {
            let import = prepared.into_all_pages().expect("all conversion");
            std::hint::black_box(&import);
            report("all");
        }
        "state" => {
            let import = prepared.into_all_pages().expect("all conversion");
            let state = op_editor_core::EditorState::from_document(import.document);
            std::hint::black_box(&state);
            report("state");
        }
        "active" => {
            let import = prepared.into_all_pages().expect("all conversion");
            let state = op_editor_core::EditorState::from_document(import.document);
            let scene = op_pen_loader::editor_state_to_active_page_layout_scene(&state);
            std::hint::black_box((&state, &scene));
            report("active");
        }
        "switch" => {
            let import = prepared.into_all_pages().expect("all conversion");
            let mut state = op_editor_core::EditorState::from_document(import.document);
            let page_count = state.doc.pages.as_ref().map_or(1, Vec::len);
            report("switch-state");
            for index in 0..page_count {
                state.ui.active_page_index = index;
                let started = std::time::Instant::now();
                let scene = op_pen_loader::editor_state_to_active_page_layout_scene(&state);
                eprintln!(
                    "SWITCH page={index} elapsed_ms={:.3}",
                    started.elapsed().as_secs_f64() * 1_000.0
                );
                std::hint::black_box(&scene);
                report("switch-page");
            }
            std::hint::black_box(&state);
            report("switch-done");
        }
        _ => panic!("unknown mode {mode}"),
    };
}
