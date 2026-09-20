//! Image decode + raster-cache behaviour: LRU eviction, entry caps,
//! coverage-aware resolution, and the adjustment matrix.
//!
//! Split out of `backend/skia/tests.rs` to keep every file under the
//! repo's 800-line cap.

use super::*;

#[test]
fn contain_rect_fits_wide_image_letterboxed_vertically() {
    let outer = Rect::xywh(0.0, 0.0, 100.0, 100.0);
    // A 200×100 image is wider than the box → width-bound, with
    // empty bands top + bottom, image centered vertically.
    let r = contain_rect(outer, 200.0, 100.0);
    assert!((r.size.x - 100.0).abs() < 1e-4);
    assert!((r.size.y - 50.0).abs() < 1e-4);
    assert!((r.origin.x - 0.0).abs() < 1e-4);
    assert!((r.origin.y - 25.0).abs() < 1e-4, "centered vertically");
}

#[test]
fn contain_rect_fits_tall_image_pillarboxed_horizontally() {
    let outer = Rect::xywh(0.0, 0.0, 100.0, 100.0);
    let r = contain_rect(outer, 100.0, 200.0);
    assert!((r.size.y - 100.0).abs() < 1e-4);
    assert!((r.size.x - 50.0).abs() < 1e-4);
    assert!((r.origin.x - 25.0).abs() < 1e-4, "centered horizontally");
}

#[test]
fn captured_tesla_affines_map_source_pixels_back_to_node_corners() {
    let cases = [
        (
            Rect::xywh(11.0, 17.0, 191.0, 236.0),
            [0.5089059, 0.0, 0.490246, 0.0, 0.28951487, 0.37636933],
        ),
        (
            Rect::xywh(23.0, 29.0, 375.0, 490.0),
            [0.9999718, 0.0, 0.00001408411, 0.0, 0.602706, 0.12054121],
        ),
    ];
    for (rect, transform) in cases {
        let local = figma_image_local_matrix(rect, 1179.0, 2556.0, transform)
            .expect("captured matrix is invertible");
        for (node_x, node_y) in [(0.0, 0.0), (1.0, 1.0)] {
            let src_x = (transform[0] * node_x + transform[1] * node_y + transform[2]) * 1179.0;
            let src_y = (transform[3] * node_x + transform[4] * node_y + transform[5]) * 2556.0;
            let dst_x = local[0] * src_x + local[1] * src_y + local[2];
            let dst_y = local[3] * src_x + local[4] * src_y + local[5];
            let expected_x = rect.origin.x + rect.size.x * node_x;
            let expected_y = rect.origin.y + rect.size.y * node_y;
            assert!((dst_x - expected_x).abs() < 0.001, "x orientation");
            assert!((dst_y - expected_y).abs() < 0.001, "y orientation");
        }
    }
}

#[test]
fn contain_rect_degenerate_image_size_falls_back_to_outer() {
    // A zero-dimension image must not divide-by-zero — it just
    // returns the outer rect unchanged.
    let outer = Rect::xywh(5.0, 6.0, 80.0, 40.0);
    let r = contain_rect(outer, 0.0, 0.0);
    assert!((r.size.x - 80.0).abs() < 1e-4);
    assert!((r.size.y - 40.0).abs() < 1e-4);
}

#[test]
fn cover_rect_crops_square_image_vertically_in_wide_rect() {
    let outer = Rect::xywh(0.0, 0.0, 360.0, 240.0);
    let r = cover_rect(outer, 200.0, 200.0);
    assert!((r.size.x - 360.0).abs() < 1e-4);
    assert!((r.size.y - 360.0).abs() < 1e-4);
    assert!((r.origin.x - 0.0).abs() < 1e-4);
    assert!(
        (r.origin.y + 60.0).abs() < 1e-4,
        "center-cropped vertically"
    );
}

#[test]
fn image_adjustment_matrix_matches_ts_formula() {
    let matrix = image_adjustment_matrix(op_editor_ui::ImageAdjustments {
        exposure: 100.0,
        contrast: -100.0,
        saturation: 100.0,
        temperature: 100.0,
        tint: -100.0,
        highlights: 100.0,
        shadows: 100.0,
    })
    .expect("non-neutral adjustments produce a color matrix");

    // With contrast = -100%, c = 0, so every RGB multiplier is zero.
    // The visible change comes from the TS offset formula.
    assert!((matrix[0] - 0.0).abs() < 1e-6);
    assert!((matrix[5] - 0.0).abs() < 1e-6);
    assert!((matrix[10] - 0.0).abs() < 1e-6);
    assert!((matrix[4] - 0.80).abs() < 1e-6);
    assert!((matrix[9] - 0.50).abs() < 1e-6);
    assert!((matrix[14] - 0.50).abs() < 1e-6);
}

#[test]
fn image_decoded_is_false_before_install_and_true_after() {
    let mut be = NativeBackend::with_dpi(1.0);
    let png = encode_test_png(4, 3);
    assert!(!be.image_decoded(7, &png, 64));

    let image = decode_raster(&png).expect("valid PNG rasterizes");
    be.install_raster_image(7, image, u32::MAX);

    assert!(be.image_decoded(7, &png, 64));
    assert_eq!(
        be.raster_image(7).expect("installed").dimensions(),
        (4, 3).into()
    );
}

#[test]
fn prompt_center_jpeg_decodes_into_a_capped_raster() {
    let previews: [(&str, &[u8]); 4] = [
        (
            "starter-travel-app",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../op-editor-ui/assets/prompt_center_previews/starter-travel-app.jpg"
            ))
            .as_slice(),
        ),
        (
            "starter-dashboard",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../op-editor-ui/assets/prompt_center_previews/starter-dashboard.jpg"
            ))
            .as_slice(),
        ),
        (
            "starter-coffee-shop",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../op-editor-ui/assets/prompt_center_previews/starter-coffee-shop.jpg"
            ))
            .as_slice(),
        ),
        (
            "starter-barbershop",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../op-editor-ui/assets/prompt_center_previews/starter-barbershop.jpg"
            ))
            .as_slice(),
        ),
    ];

    for (prompt_id, encoded) in previews {
        let (image, covers_edge_px) = decode_raster_capped(encoded, 320)
            .unwrap_or_else(|| panic!("{prompt_id} preview JPEG must rasterize"));
        assert_eq!(image.dimensions(), (320, 200).into(), "{prompt_id}");
        assert_eq!(covers_edge_px, 320, "{prompt_id}");
    }
}

#[test]
fn image_cache_evicts_least_recently_used_over_byte_budget() {
    let mut be = NativeBackend::with_dpi(1.0);
    let png = encode_test_png(4, 3);
    be.install_raster_image(1, decode_raster(&png).unwrap(), u32::MAX);
    be.install_raster_image(2, decode_raster(&png).unwrap(), u32::MAX);
    be.install_raster_image(3, decode_raster(&png).unwrap(), u32::MAX);
    assert_eq!(be.image_cache_len(), 3);
    // Touch id 1 so id 2 becomes the least-recently-used entry.
    be.raster_image(1);
    let raster_bytes = 4 * 3 * 4;
    // A budget that only fits two rasters must evict exactly the
    // LRU entry (id 2) — not the most recently touched (id 1).
    be.evict_images_over(raster_bytes * 2, usize::MAX);
    assert_eq!(be.image_cache_len(), 2, "one entry evicted");
    assert!(be.raster_image(1).is_some());
    assert!(be.raster_image(3).is_some());
    assert_eq!(
        be.image_cache_len(),
        2,
        "ids 1 and 3 survived — re-touching them adds no new entries"
    );
}

#[test]
fn image_cache_entry_cap_bounds_small_rasters() {
    let mut be = NativeBackend::with_dpi(1.0);
    let png = encode_test_png(1, 1);
    for id in 0..10u64 {
        be.install_raster_image(id, decode_raster(&png).unwrap(), u32::MAX);
    }
    be.evict_images_over(usize::MAX, 4);
    assert_eq!(be.image_cache_len(), 4, "entry cap enforced");
}

#[test]
fn image_cache_decodes_a_valid_png() {
    let png = encode_test_png(4, 3);
    let img = decode_raster(&png).expect("valid PNG decodes");
    assert_eq!(img.width(), 4);
    assert_eq!(img.height(), 3);
    assert!(
        img.peek_pixels().is_some(),
        "decode helper returns raster pixels"
    );
}

#[test]
fn image_draw_respects_node_opacity() {
    // A solid-blue image drawn at 0.5 opacity over white must blend
    // toward white (≈ 50% each); full opacity would leave it pure blue.
    let mut be = NativeBackend::with_dpi(1.0);
    let png = encode_test_png(8, 8);
    be.install_raster_image(4242, decode_raster(&png).unwrap(), u32::MAX);
    let mut surface = skia_safe::surfaces::raster_n32_premul((20, 20)).unwrap();
    surface.canvas().clear(skia_safe::Color::WHITE);
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(20.0, 20.0),
    };
    be.draw_image_with_options(
        surface.canvas(),
        rect,
        4242,
        &png,
        op_editor_ui::ImageDrawMode::Stretch,
        op_editor_ui::ImageAdjustments::default(),
        0.5,
        0.0,
    );
    let img = surface.image_snapshot();
    let pm = img.peek_pixels().expect("peek raster pixels");
    let c = pm.get_color((10, 10));
    // 0.5 blue over white ≈ (128,128,255); full opacity would be r=0.
    assert!(
        c.r() > 80 && c.r() < 200,
        "image opacity should blend toward white, got r={}",
        c.r()
    );
    assert!(
        c.b() > 200,
        "blue channel should stay high, got b={}",
        c.b()
    );
}

/// Rastering to the size the view needs is what keeps a zoomed-out,
/// image-dense page cheap: the same source that costs megabytes at full
/// size costs kilobytes when it is drawn as a thumbnail.
#[test]
fn decode_raster_capped_scales_down_and_reports_its_coverage() {
    let png = encode_test_png(256, 128);

    let (small, covers) = decode_raster_capped(&png, 64).expect("capped decode");
    assert_eq!(small.width(), 64, "longest edge honours the cap");
    assert_eq!(small.height(), 32, "aspect ratio preserved");
    assert_eq!(covers, 64);

    // A cap at or above the source keeps the full raster and reports
    // itself as sharp at any size.
    let (full, covers) = decode_raster_capped(&png, 512).expect("uncapped decode");
    assert_eq!((full.width(), full.height()), (256, 128));
    assert_eq!(covers, u32::MAX);
}

/// A cached raster that is too coarse must not count as ready: paint
/// re-queues a sharper decode while still drawing what it has.
#[test]
fn image_decoded_requires_the_cached_raster_to_cover_the_requested_size() {
    let png = encode_test_png(256, 256);
    let mut be = NativeBackend::with_dpi(1.0);
    let (small, covers) = decode_raster_capped(&png, 64).expect("capped decode");
    be.install_raster_image(9, small, covers);

    assert!(be.image_decoded(9, &png, 64), "exact level is ready");
    assert!(be.image_decoded(9, &png, 32), "a coarser need is satisfied");
    assert!(
        !be.image_decoded(9, &png, 256),
        "zooming in past the cached level asks for a sharper decode"
    );
    assert!(
        be.raster_image(9).is_some(),
        "the coarse raster still draws while the sharper one decodes"
    );
}

/// Sharpening must refine in place. A raster that is merely too coarse
/// is still resident, so paint keeps drawing it while the sharper
/// decode runs instead of dropping back to placeholder art.
#[test]
fn a_coarse_raster_stays_resident_while_a_sharper_one_is_requested() {
    let png = encode_test_png(256, 256);
    let mut be = NativeBackend::with_dpi(1.0);
    let (small, covers) = decode_raster_capped(&png, 64).expect("capped decode");
    be.install_raster_image(11, small, covers);

    assert!(
        !be.image_decoded(11, &png, 256),
        "a zoom-in asks for a sharper decode"
    );
    assert!(
        be.image_resident(11),
        "but the coarse raster is still there to draw"
    );
    assert!(
        !be.image_resident(12),
        "an unknown image has nothing to draw"
    );
}
