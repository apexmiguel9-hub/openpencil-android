//! SVG-path cache, fill rules, gradients, shadows and the blur/mesh
//! pixel probes.
//!
//! Split out of `backend/skia/tests.rs` to keep every file under the
//! repo's 800-line cap.

use super::*;

#[test]
fn svg_path_cache_reuses_parsed_paths() {
    let mut be = NativeBackend::with_dpi(1.0);
    let mut surface = skia_safe::surfaces::raster_n32_premul((32, 32)).unwrap();
    let canvas = surface.canvas();
    let d = "M0 0 L10 0 L10 10 Z";

    be.fill_svg_path(canvas, d, Point2D::ZERO, 1.0, 1.0, Color::BLACK);
    assert_eq!(be.svg_path_cache_len(), 1);

    be.fill_svg_path(canvas, d, Point2D::new(4.0, 4.0), 2.0, 1.0, Color::RED);
    assert_eq!(be.svg_path_cache_len(), 1);
}

#[test]
fn svg_path_cache_holds_a_large_document_working_set() {
    // A vector-heavy Figma import easily carries >10k distinct paths.
    // A small entry cap made every zoomed-out frame cycle the whole
    // cache and re-parse every visible path from scratch — the cache
    // must hold a large working set, bounded by bytes instead.
    let mut be = NativeBackend::with_dpi(1.0);
    let mut surface = skia_safe::surfaces::raster_n32_premul((32, 32)).unwrap();
    let canvas = surface.canvas();
    for i in 0..4096 {
        let d = format!("M0 {i} L10 0 L10 10 Z");
        be.fill_svg_path(canvas, &d, Point2D::ZERO, 1.0, 1.0, Color::BLACK);
    }
    assert_eq!(be.svg_path_cache_len(), 4096);
}

#[test]
fn svg_path_cache_evicts_oldest_over_byte_budget() {
    let mut be = NativeBackend::with_dpi(1.0);
    let mut surface = skia_safe::surfaces::raster_n32_premul((32, 32)).unwrap();
    let canvas = surface.canvas();
    let first = "M0 0 L10 0 L10 10 Z";
    be.fill_svg_path(canvas, first, Point2D::ZERO, 1.0, 1.0, Color::BLACK);
    be.fill_svg_path(
        canvas,
        "M0 1 L10 0 L10 10 Z",
        Point2D::ZERO,
        1.0,
        1.0,
        Color::BLACK,
    );
    be.fill_svg_path(
        canvas,
        "M0 2 L10 0 L10 10 Z",
        Point2D::ZERO,
        1.0,
        1.0,
        Color::BLACK,
    );
    assert_eq!(be.svg_path_cache_len(), 3);

    // A budget that only fits two of the three entries drops the
    // oldest (FIFO), keeping the most recently inserted pair.
    be.evict_svg_paths_over(first.len() * 2, usize::MAX);
    assert_eq!(be.svg_path_cache_len(), 2);
    be.evict_svg_paths_over(usize::MAX, 1);
    assert_eq!(be.svg_path_cache_len(), 1);
}

#[test]
fn explicit_even_odd_rule_sets_skia_path_fill_type() {
    let mut be = NativeBackend::with_dpi(1.0);
    let d = "M0 0H20V20H0Z M5 5H15V15H5Z";
    let rect = Rect::xywh(0.0, 0.0, 20.0, 20.0);

    let nonzero = be
        .fitted_svg_path(d, rect, Some(false))
        .expect("nonzero path");
    let evenodd = be
        .fitted_svg_path(d, rect, Some(true))
        .expect("even-odd path");

    assert_eq!(nonzero.fill_type(), skia_safe::PathFillType::Winding);
    assert_eq!(evenodd.fill_type(), skia_safe::PathFillType::EvenOdd);
}

#[test]
fn native_backdrop_blur_changes_pixels_inside_clip() {
    fn render(blur: bool) -> Vec<u8> {
        let backend = NativeBackend::with_dpi(1.0);
        let mut surface = skia_safe::surfaces::raster_n32_premul((64, 32)).unwrap();
        let canvas = surface.canvas();
        canvas.clear(skia_safe::Color::WHITE);
        for x in (0..64).step_by(8) {
            let color = if (x / 8) % 2 == 0 {
                skia_safe::Color::RED
            } else {
                skia_safe::Color::BLUE
            };
            let color4f = skia_safe::Color4f::from(color);
            let paint = skia_safe::Paint::new(color4f, None);
            canvas.draw_rect(skia_safe::Rect::from_xywh(x as f32, 0.0, 8.0, 32.0), &paint);
        }
        if blur {
            canvas.save();
            backend.clip_round_rect(canvas, Rect::xywh(8.0, 4.0, 48.0, 24.0), 6.0);
            backend.push_backdrop_blur_layer(canvas, 4.0);
            canvas.restore();
            canvas.restore();
        }
        let image = surface.image_snapshot();
        image
            .peek_pixels()
            .expect("raster pixels")
            .bytes()
            .expect("pixel bytes")
            .to_vec()
    }

    assert_ne!(render(false), render(true));
}

#[test]
fn complex_svg_fill_uses_raster_cache_after_first_paint() {
    let mut be = NativeBackend::with_dpi(1.0);
    let mut surface = skia_safe::surfaces::raster_n32_premul((128, 128)).unwrap();
    let canvas = surface.canvas();
    let d = format!("M0 0 L64 0 L64 64 L0 64 Z{}", " ".repeat(4096));

    be.fill_svg_path(canvas, &d, Point2D::ZERO, 1.0, 1.0, Color::BLACK);
    assert_eq!(be.svg_path_cache_len(), 1);
    assert_eq!(be.svg_raster_cache_len(), 1);

    be.fill_svg_path(canvas, &d, Point2D::new(8.0, 8.0), 1.0, 1.0, Color::BLACK);
    assert_eq!(be.svg_path_cache_len(), 1);
    assert_eq!(be.svg_raster_cache_len(), 1);
}

#[test]
fn native_mesh_gradient_is_actually_interpolated_not_flat() {
    // PROOF (native op-host path): render a 2x2 R/G/B/Y mesh onto a raster
    // surface and read pixels back. Corners must read pure; the centre must
    // be an interpolated blend distinct from every corner. A first-vertex
    // solid fallback would paint the whole rect one colour.
    use skia_safe::{image::CachingHint, AlphaType, ColorType, ISize, ImageInfo};

    let be = NativeBackend::with_dpi(1.0);
    let (w, h) = (64i32, 64i32);
    let mut surface = skia_safe::surfaces::raster_n32_premul((w, h)).unwrap();
    surface.canvas().clear(skia_safe::Color::BLACK);

    be.fill_round_rect_mesh_gradient(
        surface.canvas(),
        Rect::xywh(0.0, 0.0, w as f32, h as f32),
        0.0, // no rounding -> centre is mesh, not clipped
        2,
        2,
        &[
            Color::rgb_u8(0xff, 0x00, 0x00), // TL red
            Color::rgb_u8(0x00, 0xff, 0x00), // TR green
            Color::rgb_u8(0x00, 0x00, 0xff), // BL blue
            Color::rgb_u8(0xff, 0xff, 0x00), // BR yellow
        ],
        1.0,
    );

    let image = surface.image_snapshot();
    let info = ImageInfo::new(
        ISize::new(w, h),
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        None,
    );
    let mut buf = vec![0u8; (w * h * 4) as usize];
    assert!(
        image.read_pixels(
            &info,
            &mut buf,
            (w as usize) * 4,
            (0, 0),
            CachingHint::Allow
        ),
        "read_pixels failed"
    );
    let px = |x: i32, y: i32| -> (u8, u8, u8) {
        let i = ((y * w + x) * 4) as usize;
        (buf[i], buf[i + 1], buf[i + 2])
    };

    let tl = px(1, 1);
    let tr = px(w - 2, 1);
    let bl = px(1, h - 2);
    let br = px(w - 2, h - 2);
    assert!(tl.0 > 200 && tl.1 < 60 && tl.2 < 60, "TL not red: {:?}", tl);
    assert!(
        tr.0 < 60 && tr.1 > 200 && tr.2 < 60,
        "TR not green: {:?}",
        tr
    );
    assert!(
        bl.0 < 60 && bl.1 < 60 && bl.2 > 200,
        "BL not blue: {:?}",
        bl
    );
    assert!(
        br.0 > 200 && br.1 > 200 && br.2 < 60,
        "BR not yellow: {:?}",
        br
    );

    // Centre sits on the TR(green)->BL(blue) triangulation diagonal -> a
    // green/blue blend (G,B mid). Proves Gouraud interpolation, not a flat fill.
    let c = px(w / 2, h / 2);
    assert!(
        c.1 > 60 && c.1 < 200 && c.2 > 60 && c.2 < 200,
        "centre is not an interpolated blend (flat fallback?): {:?}",
        c
    );
    for (name, corner) in [("TL", tl), ("TR", tr), ("BL", bl), ("BR", br)] {
        assert!(
            (c.0 as i32 - corner.0 as i32).abs()
                + (c.1 as i32 - corner.1 as i32).abs()
                + (c.2 as i32 - corner.2 as i32).abs()
                > 60,
            "centre {:?} too close to corner {} {:?} (flat fill?)",
            c,
            name,
            corner
        );
    }
}

#[test]
fn linear_gradient_angle_zero_runs_bottom_to_top() {
    // The canonical `.op` convention puts `angle = 0` at "from
    // bottom to top" (CSS `to-top`). Mirrors the TS renderer at
    // `pen-renderer/src/node-renderer.ts:155` which subtracts 90°
    // before projecting onto endpoints.
    let rect = Rect::xywh(0.0, 0.0, 100.0, 50.0);
    let (start, end) = crate::backend::skia::gradient::linear_gradient_endpoints(rect, 0.0);
    // Start at the bottom edge centre, end at the top edge centre.
    assert!((start.x - 50.0).abs() < 1e-3, "start x={}", start.x);
    assert!((start.y - 25.0 - 25.0).abs() < 1e-3, "start y={}", start.y);
    assert!((end.x - 50.0).abs() < 1e-3, "end x={}", end.x);
    assert!((end.y - 25.0 + 25.0).abs() < 1e-3, "end y={}", end.y);
}

#[test]
fn linear_gradient_angle_ninety_runs_left_to_right() {
    // `angle = 90` → horizontal, left to right.
    let rect = Rect::xywh(0.0, 0.0, 100.0, 50.0);
    let (start, end) = crate::backend::skia::gradient::linear_gradient_endpoints(rect, 90.0);
    assert!((start.x - 0.0).abs() < 1e-3, "start x={}", start.x);
    assert!((start.y - 25.0).abs() < 1e-3, "start y={}", start.y);
    assert!((end.x - 100.0).abs() < 1e-3, "end x={}", end.x);
    assert!((end.y - 25.0).abs() < 1e-3, "end y={}", end.y);
}

#[test]
fn linear_gradient_endpoints_use_ellipse_not_aabb() {
    // At 45°, endpoints must sit on the bounding ellipse — NOT on
    // the AABB diagonal. The earlier AABB-projection trick gave a
    // longer gradient line that diverged from the TS renderer.
    let rect = Rect::xywh(0.0, 0.0, 200.0, 100.0);
    let (start, end) = crate::backend::skia::gradient::linear_gradient_endpoints(rect, 45.0);
    // 45° in canonical convention = (angle - 90 = -45°) in screen
    // convention. cos(-45°) = √2/2, sin(-45°) = -√2/2.
    // dx = (√2/2) * 100 ≈ 70.71, dy = (-√2/2) * 50 ≈ -35.36.
    let dx_expected = 200.0 * 0.5 * 0.5_f32.sqrt();
    let dy_expected = -100.0 * 0.5 * 0.5_f32.sqrt();
    assert!((start.x - (100.0 - dx_expected)).abs() < 1e-2);
    assert!((start.y - (50.0 - dy_expected)).abs() < 1e-2);
    assert!((end.x - (100.0 + dx_expected)).abs() < 1e-2);
    assert!((end.y - (50.0 + dy_expected)).abs() < 1e-2);
}

#[test]
fn linear_gradient_path_renders_color_ramp() {
    // A full-rect square path filled with a left→right gradient
    // (white at offset 0, red at offset 1; angle 90° = left→right)
    // must paint a real ramp: the left edge stays green-ish (white),
    // the right edge loses green (red). A solid first-stop fallback
    // would paint the whole path white and fail the assert.
    let mut be = NativeBackend::with_dpi(1.0);
    let mut surface = skia_safe::surfaces::raster_n32_premul((40, 40)).unwrap();
    surface.canvas().clear(skia_safe::Color::BLACK);
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(40.0, 40.0),
    };
    let stops = [
        (
            0.0,
            Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
        ),
        (
            1.0,
            Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
        ),
    ];
    be.fill_svg_path_in_rect_linear_gradient(
        surface.canvas(),
        "M0 0 L1 0 L1 1 L0 1 Z",
        rect,
        &stops,
        90.0,
        1.0,
    );
    let img = surface.image_snapshot();
    let pm = img.peek_pixels().expect("peek raster pixels");
    let left = pm.get_color((3, 20));
    let right = pm.get_color((37, 20));
    assert!(
        left.g() as i32 > right.g() as i32 + 60,
        "expected a left→right ramp (left greener than right), got left.g={} right.g={}",
        left.g(),
        right.g()
    );
}

#[test]
fn inner_shadow_path_darkens_edges_not_center() {
    // A full-rect square path with a black inset shadow (offset 0,
    // blur 8) must darken the inside edges while the centre stays
    // near-white. A no-op (or outer-shadow) fallback would leave the
    // edge as bright as the centre.
    let mut be = NativeBackend::with_dpi(1.0);
    let mut surface = skia_safe::surfaces::raster_n32_premul((60, 60)).unwrap();
    surface.canvas().clear(skia_safe::Color::WHITE);
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(60.0, 60.0),
    };
    let d = "M0 0 L1 0 L1 1 L0 1 Z";
    be.fill_inner_shadow_svg_path(
        surface.canvas(),
        d,
        rect,
        0.0,
        0.0,
        8.0,
        Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
    );
    let img = surface.image_snapshot();
    let pm = img.peek_pixels().expect("peek raster pixels");
    let edge = pm.get_color((2, 30));
    let center = pm.get_color((30, 30));
    assert!(
        (edge.r() as i32) < (center.r() as i32) - 30,
        "inset shadow should darken the edge vs centre: edge.r={} center.r={}",
        edge.r(),
        center.r()
    );
}
