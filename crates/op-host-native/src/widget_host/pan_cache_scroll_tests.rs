//! Pan-cache registration: every pixel the cached layer serves must
//! land where a direct paint would put it — the strips repainted while
//! the layer scrolls in place mid-gesture, and the margin the
//! gesture-end rebase vacates. Both are painted into the overdraw
//! margin, off screen, so only a LATER pan exposes them; a test that
//! only checks the frame that painted them proves nothing.

use super::WidgetHostNative;
use crate::backend::{NativeBackend, NativeFrameBackend};

const W: i32 = 900;
const H: i32 = 700;

/// A dense grid of small squares: a misregistered band shows up as a
/// large pixel diff, and the varied colours make a whole-cell shift
/// detectable rather than self-similar.
fn grid_document_json() -> String {
    const PALETTE: [&str; 4] = ["#ff2d2d", "#2d7dff", "#12b886", "#f59f00"];
    let mut children = String::new();
    let mut i = 0usize;
    let mut y = -900;
    while y <= 1200 {
        let mut x = -900;
        while x <= 1600 {
            if i > 0 {
                children.push(',');
            }
            children.push_str(&format!(
                r##"{{"type":"rectangle","id":"n{i}","x":{x},"y":{y},"width":64,"height":64,"fill":[{{"type":"solid","color":"{c}"}}]}}"##,
                c = PALETTE[i % PALETTE.len()],
            ));
            i += 1;
            x += 120;
        }
        y += 120;
    }
    format!(r##"{{"version":"1.0.0","children":[{children}]}}"##)
}

fn seed_grid(host: &mut WidgetHostNative, zoom: f32) {
    let doc = jian_ops_schema::load_str(&grid_document_json())
        .expect("fixture JSON parses")
        .value;
    let mut state = op_editor_core::EditorState::from_document(doc);
    state.chat.minimize();
    state.viewport.zoom = zoom;
    *host.editor_state_mut() = state;
    host.mark_paint_dirty_for_test();
}

/// Paint one frame the way the desktop runner does: a physical-pixel
/// surface carrying the DPI scale, logical sizes handed to `paint`.
fn paint_frame(host: &mut WidgetHostNative, backend: &mut NativeBackend, dpi: f32) -> Vec<u8> {
    let (pw, ph) = ((W as f32 * dpi) as i32, (H as f32 * dpi) as i32);
    let mut surface =
        skia_safe::surfaces::raster_n32_premul((pw, ph)).expect("raster surface allocated");
    surface.canvas().clear(skia_safe::Color::WHITE);
    surface.canvas().scale((dpi, dpi));
    {
        let mut frame = NativeFrameBackend::new(backend, surface.canvas());
        host.paint(&mut frame, W as f32, H as f32);
    }
    let stride = (pw * 4) as usize;
    let mut pixels = vec![0u8; stride * ph as usize];
    let info = skia_safe::ImageInfo::new(
        (pw, ph),
        skia_safe::ColorType::RGBA8888,
        skia_safe::AlphaType::Premul,
        None,
    );
    assert!(surface.read_pixels(&info, &mut pixels, stride, (0, 0)));
    pixels
}

/// A cache-free paint of the same document at the same viewport — the
/// ground truth every cached frame is measured against.
fn reference_frame(zoom: f32, dpi: f32, pan_x: f32, pan_y: f32) -> Vec<u8> {
    let mut host = WidgetHostNative::new();
    seed_grid(&mut host, zoom);
    host.editor_state_mut().viewport.pan_x = pan_x;
    host.editor_state_mut().viewport.pan_y = pan_y;
    host.mark_paint_dirty_for_test();
    host.set_now_ms(1_000);
    let mut backend = NativeBackend::with_dpi(dpi);
    paint_frame(&mut host, &mut backend, dpi)
}

/// Pixels differing by more than a hair, with the diff bounding box and
/// a column profile so a failure names the band.
///
/// The tolerance skips the 1/255 rounding a tile-clipped repaint leaves
/// where a shape edge lands exactly on a tile seam — present with or
/// without the cache, and orders of magnitude below what
/// misregistration produces (a shifted band swaps whole fills).
fn diff_report(actual: &[u8], expected: &[u8], dpi: f32) -> Option<String> {
    const TOLERANCE: i32 = 8;
    // A handful of isolated pixels is the same seam-rounding class as the
    // per-channel tolerance above, just past its threshold — Windows'
    // scalar math rounds one glyph/tile edge differently (observed: exactly
    // 1 px on the CI runner). Misregistration cannot hide here: a shifted
    // band swaps whole fills, which is hundreds of pixels in contiguous
    // columns, far past this budget.
    const ISOLATED_PIXEL_BUDGET: usize = 4;
    let (pw, ph) = ((W as f32 * dpi) as i32, (H as f32 * dpi) as i32);
    let stride = (pw * 4) as usize;
    let (mut count, mut x0, mut y0, mut x1, mut y1) = (0usize, pw, ph, -1, -1);
    let mut cols = vec![0usize; pw as usize];
    for y in 0..ph {
        for x in 0..pw {
            let o = y as usize * stride + x as usize * 4;
            let delta = (0..3)
                .map(|c| (actual[o + c] as i32 - expected[o + c] as i32).abs())
                .max()
                .unwrap_or(0);
            if delta > TOLERANCE {
                count += 1;
                cols[x as usize] += 1;
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    if count <= ISOLATED_PIXEL_BUDGET {
        return None;
    }
    let profile: Vec<String> = (0..pw as usize)
        .filter(|&x| cols[x] > 0)
        .map(|x| format!("{x}:{}", cols[x]))
        .collect();
    Some(format!(
        "{count} px differ, bbox x[{x0}..{x1}] y[{y0}..{y1}], per-column [{}]",
        profile.join(" ")
    ))
}

fn pan(host: &mut WidgetHostNative, dx: f32, dy: f32) {
    assert!(host.apply_pan_gesture(400.0, 300.0, dx, dy, W as f32, H as f32));
}

fn assert_matches_direct_paint(
    host: &WidgetHostNative,
    actual: &[u8],
    zoom: f32,
    dpi: f32,
    what: &str,
) {
    let viewport = &host.editor_state().viewport;
    let reference = reference_frame(zoom, dpi, viewport.pan_x, viewport.pan_y);
    if let Some(report) = diff_report(actual, &reference, dpi) {
        panic!("{what} diverged from a direct paint: {report}");
    }
}

/// A diagonal pan crosses the scroll threshold on one axis only, so the
/// layer's anchor stays a whole cross-axis delta behind the live
/// viewport pan. The strip repainted for the scrolled axis must be
/// registered against that anchor, not against the live pan.
#[test]
fn diagonal_scroll_refresh_strip_is_registered_with_the_layer() {
    let _indicator_guard = crate::agent_indicator_test_support::read();
    let (zoom, dpi) = (1.16, 1.0);
    let mut host = WidgetHostNative::new();
    seed_grid(&mut host, zoom);
    let mut backend = NativeBackend::with_dpi(dpi);
    host.set_now_ms(1_000);
    let _ = paint_frame(&mut host, &mut backend, dpi);

    // Build the layer on the first hot frame.
    pan(&mut host, 20.0, 0.0);
    let _ = paint_frame(&mut host, &mut backend, dpi);
    assert!(host.pan_cache_resident_for_test());
    assert_eq!(host.pan_cache_scrolls_for_test(), 0);

    // Diagonal tick past the threshold on x: x scrolls in place, y is
    // carried as a 70 px residual the blit applies to the whole layer.
    pan(&mut host, 210.0, 70.0);
    let _ = paint_frame(&mut host, &mut backend, dpi);
    assert_eq!(host.pan_cache_scrolls_for_test(), 1);

    // Keep panning the same way so the freshly repainted strip scrolls
    // out of the margin and into the visible canvas.
    pan(&mut host, 160.0, 0.0);
    let banded = paint_frame(&mut host, &mut backend, dpi);
    assert_matches_direct_paint(&host, &banded, zoom, dpi, "scroll-refreshed strip");
}

/// The same invariant at 2× DPI and 2× zoom, with fractional gesture
/// deltas so the whole-device-pixel snap leaves a real sub-pixel
/// residual on the scrolled axis on top of the cross-axis one.
#[test]
fn scroll_refresh_stays_registered_at_high_dpi_and_zoom() {
    let _indicator_guard = crate::agent_indicator_test_support::read();
    let (zoom, dpi) = (2.0, 2.0);
    let mut host = WidgetHostNative::new();
    seed_grid(&mut host, zoom);
    let mut backend = NativeBackend::with_dpi(dpi);
    host.set_now_ms(1_000);
    let _ = paint_frame(&mut host, &mut backend, dpi);

    pan(&mut host, 12.25, 0.0);
    let _ = paint_frame(&mut host, &mut backend, dpi);
    assert!(host.pan_cache_resident_for_test());

    pan(&mut host, 205.375, 43.125);
    let _ = paint_frame(&mut host, &mut backend, dpi);
    assert_eq!(host.pan_cache_scrolls_for_test(), 1);

    pan(&mut host, 150.625, 0.0);
    let banded = paint_frame(&mut host, &mut backend, dpi);
    assert_matches_direct_paint(&host, &banded, zoom, dpi, "high-dpi scroll refresh");
}

/// A long drag: many small ticks, several threshold crossings, and an
/// axis handover partway through. Every strip left in the layer must
/// still be registered, so errors cannot accumulate into the multi-band
/// tear a single crossing only hints at.
#[test]
fn long_diagonal_drag_accumulates_no_misregistration() {
    let _indicator_guard = crate::agent_indicator_test_support::read();
    let (zoom, dpi) = (1.5, 1.0);
    let mut host = WidgetHostNative::new();
    seed_grid(&mut host, zoom);
    let mut backend = NativeBackend::with_dpi(dpi);
    host.set_now_ms(1_000);
    let _ = paint_frame(&mut host, &mut backend, dpi);

    // ~1300 px of travel in 13 px steps: x-dominant for the first half,
    // y-dominant for the second, so the refresh axis hands over.
    for step in 0..80 {
        let (dx, dy) = if step < 40 { (13.0, 4.0) } else { (4.0, 13.0) };
        pan(&mut host, dx, dy);
        let _ = paint_frame(&mut host, &mut backend, dpi);
    }
    assert!(
        host.pan_cache_scrolls_for_test() >= 4,
        "the drag must cross the scroll threshold repeatedly"
    );
    let dragged = paint_frame(&mut host, &mut backend, dpi);
    assert_matches_direct_paint(&host, &dragged, zoom, dpi, "long diagonal drag");
}

/// The gesture-end restore rebases the layer onto the current pan by
/// shifting its pixels. The margin the shift vacates is not covered by
/// the visible-region tiles, so it must not be served to the next
/// gesture as if it still held content.
#[test]
fn margin_vacated_by_the_restore_rebase_is_not_served_stale() {
    let _indicator_guard = crate::agent_indicator_test_support::read();
    let (zoom, dpi) = (1.16, 1.0);
    let mut host = WidgetHostNative::new();
    seed_grid(&mut host, zoom);
    let mut backend = NativeBackend::with_dpi(dpi);
    host.set_now_ms(1_000);
    let _ = paint_frame(&mut host, &mut backend, dpi);

    // Build the layer, then pan far enough that the gesture-end rebase
    // has a large shift to apply.
    pan(&mut host, 20.0, 0.0);
    let _ = paint_frame(&mut host, &mut backend, dpi);
    assert!(host.pan_cache_resident_for_test());
    pan(&mut host, 150.0, 0.0);
    let _ = paint_frame(&mut host, &mut backend, dpi);

    // Gesture ends: run the progressive restore to completion. The
    // budget is the visible tiles plus the strips the rebase adds.
    host.set_now_ms(10_000);
    for _ in 0..super::canvas_pan_cache::PAN_CACHE_RESTORE_TILES + 4 {
        let _ = paint_frame(&mut host, &mut backend, dpi);
        if host.pan_cache_sharp_for_test() && !host.pan_cache_restore_active_for_test() {
            break;
        }
    }
    assert!(host.pan_cache_sharp_for_test());

    // A new gesture in the same direction scrolls the rebase-vacated
    // margin back into view.
    pan(&mut host, 120.0, 0.0);
    let after = paint_frame(&mut host, &mut backend, dpi);
    assert_matches_direct_paint(&host, &after, zoom, dpi, "post-restore margin");
}
