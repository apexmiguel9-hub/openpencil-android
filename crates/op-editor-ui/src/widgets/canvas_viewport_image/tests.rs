use super::*;
use crate::layout_scene::NodeKind;
use crate::{ImageAdjustments, ImageBlendMode, ImageDrawMode, Point2D, RenderBackend, TextLayout};

#[derive(Default)]
struct ImageRadiusCaptureBackend {
    clips: Vec<[f32; 4]>,
    image_corner_radii: Vec<f32>,
    image_transforms: Vec<Option<[f32; 6]>>,
    image_blend_modes: Vec<ImageBlendMode>,
    image_original_sizes: Vec<Option<[f32; 2]>>,
    image_tile_scales: Vec<f32>,
    decode_edges: Vec<u32>,
    decode_ready: Option<bool>,
}

impl RenderBackend for ImageRadiusCaptureBackend {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, _: Rect, _: Color) {}
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn draw_text(&mut self, _: &TextLayout, _: Point2D) {}
    fn clip_rect(&mut self, _: Rect) {}
    fn clip_round_rect_per_corner(&mut self, _: Rect, radii: [f32; 4]) {
        self.clips.push(radii);
    }
    fn save(&mut self) {}
    fn restore(&mut self) {}
    fn translate(&mut self, _: Point2D) {}
    fn stroke_line(&mut self, _: Point2D, _: Point2D, _: Color, _: f32) {}
    fn fill_round_rect(&mut self, _: Rect, _: f32, _: Color) {}
    fn stroke_round_rect(&mut self, _: Rect, _: f32, _: Color, _: f32) {}
    fn stroke_svg_path(&mut self, _: &str, _: Point2D, _: f32, _: Color, _: f32) {}
    fn image_decoded(&mut self, _: u64, _: &[u8], max_edge_px: u32) -> bool {
        self.decode_edges.push(max_edge_px);
        self.decode_ready.unwrap_or(true)
    }
    fn image_resident(&mut self, _: u64) -> bool {
        // These fakes model "nothing rasterized yet", so an image
        // that is not decode-ready has nothing to draw either.
        self.decode_ready.unwrap_or(true)
    }
    fn draw_image_with_options_transform_and_blend(
        &mut self,
        _: Rect,
        _: u64,
        _: &[u8],
        _: ImageDrawMode,
        _: ImageAdjustments,
        _: f32,
        corner_radius: f32,
        transform: Option<[f32; 6]>,
        blend_mode: ImageBlendMode,
    ) {
        self.image_corner_radii.push(corner_radius);
        self.image_transforms.push(transform);
        self.image_blend_modes.push(blend_mode);
    }
    fn draw_image_with_options_transform_blend_and_tile_scale(
        &mut self,
        _: Rect,
        _: u64,
        _: &[u8],
        _: ImageDrawMode,
        _: ImageAdjustments,
        _: f32,
        corner_radius: f32,
        transform: Option<[f32; 6]>,
        blend_mode: ImageBlendMode,
        original_size: Option<[f32; 2]>,
        tile_scale: f32,
    ) {
        self.image_corner_radii.push(corner_radius);
        self.image_transforms.push(transform);
        self.image_blend_modes.push(blend_mode);
        self.image_original_sizes.push(original_size);
        self.image_tile_scales.push(tile_scale);
    }
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
}

#[test]
fn image_fill_uses_per_corner_clip_instead_of_scalar_radius() {
    let _guard = lock_statics();
    let mut node = SceneNode::leaf("image", NodeKind::Rect);
    node.corner_radius = 8.0;
    node.corner_radii = Some([8.0, 0.0, 8.0, 0.0]);
    let mut backend = ImageRadiusCaptureBackend::default();

    paint_image_node(
        &mut PaintCx {
            backend: &mut backend,
        },
        &node,
        Rect::xywh(0.0, 0.0, 100.0, 50.0),
        1.0,
        "data:image/png;base64,QUJD",
        true,
    );

    assert_eq!(backend.clips, vec![[8.0, 0.0, 8.0, 0.0]]);
    assert_eq!(backend.image_corner_radii, vec![0.0]);
    assert_eq!(backend.image_transforms, vec![None]);
    assert_eq!(backend.image_original_sizes, vec![None]);
    assert_eq!(backend.image_tile_scales, vec![1.0]);
}

#[test]
fn image_fill_forwards_figma_transform_to_backend() {
    let _guard = lock_statics();
    let transform = [0.9999718, 0.0, 0.00001408411, 0.0, 0.602706, 0.12054121];
    let mut node = SceneNode::leaf("image", NodeKind::Rect);
    node.image_transform = Some(transform);
    let mut backend = ImageRadiusCaptureBackend::default();

    paint_image_node(
        &mut PaintCx {
            backend: &mut backend,
        },
        &node,
        Rect::xywh(0.0, 0.0, 375.0, 490.0),
        1.0,
        "data:image/png;base64,QUJD",
        true,
    );

    assert_eq!(backend.image_transforms, vec![Some(transform)]);
}

#[test]
fn tile_fill_forwards_source_size_scale_and_zoom() {
    let _guard = lock_statics();
    let mut node = SceneNode::leaf("tile", NodeKind::Rect);
    node.image_fit = crate::layout_scene::SceneImageFit::Tile;
    node.image_original_size = Some([4096.0, 2048.0]);
    node.image_tile_scale = 0.38618907;
    let mut backend = ImageRadiusCaptureBackend::default();

    paint_image_node(
        &mut PaintCx {
            backend: &mut backend,
        },
        &node,
        Rect::xywh(0.0, 0.0, 220.0, 220.0),
        1.0,
        "data:image/png;base64,QUJD",
        true,
    );
    assert_eq!(backend.image_original_sizes, vec![Some([4096.0, 2048.0])]);
    assert_eq!(backend.image_tile_scales, vec![0.38618907]);

    node.image_tile_scale = f32::NAN;
    paint_image_node(
        &mut PaintCx {
            backend: &mut backend,
        },
        &node,
        Rect::xywh(0.0, 0.0, 220.0, 220.0),
        1.0,
        "data:image/png;base64,QUJD",
        true,
    );
    assert_eq!(
        backend.image_original_sizes,
        vec![Some([4096.0, 2048.0]), Some([4096.0, 2048.0])]
    );
    assert_eq!(backend.image_tile_scales, vec![0.38618907, 1.0]);

    node.image_tile_scale = 0.38618907;
    paint_image_node(
        &mut PaintCx {
            backend: &mut backend,
        },
        &node,
        Rect::xywh(0.0, 0.0, 440.0, 440.0),
        2.0,
        "data:image/png;base64,QUJD",
        true,
    );
    assert_eq!(backend.image_original_sizes[2], Some([4096.0, 2048.0]));
    assert!((backend.image_tile_scales[2] - 0.77237814).abs() < f32::EPSILON);
}

#[test]
fn tile_fill_ignores_transform_for_decode_but_forwards_it_to_draw() {
    let _guard = lock_statics();
    let transform = [0.01, 0.0, 0.4, 0.0, 0.01, 0.4];
    let mut node = SceneNode::leaf("tile", NodeKind::Rect);
    node.image_fit = crate::layout_scene::SceneImageFit::Tile;
    node.image_transform = Some(transform);
    let mut backend = ImageRadiusCaptureBackend::default();

    paint_image_node(
        &mut PaintCx {
            backend: &mut backend,
        },
        &node,
        Rect::xywh(0.0, 0.0, 220.0, 220.0),
        1.0,
        "data:image/png;base64,QUJD",
        true,
    );

    assert_eq!(
        backend.decode_edges,
        vec![256],
        "tile transforms describe repetition and must not inflate the raster request"
    );
    assert_eq!(backend.image_transforms, vec![Some(transform)]);
}

#[test]
fn image_node_forwards_blend_mode_to_backend() {
    let _guard = lock_statics();
    let mut node = SceneNode::leaf("image", NodeKind::Rect);
    node.image_blend_mode = ImageBlendMode::Multiply;
    let mut backend = ImageRadiusCaptureBackend::default();

    paint_image_node(
        &mut PaintCx {
            backend: &mut backend,
        },
        &node,
        Rect::xywh(0.0, 0.0, 100.0, 50.0),
        1.0,
        "data:image/png;base64,QUJD",
        true,
    );

    assert_eq!(backend.image_blend_modes, vec![ImageBlendMode::Multiply]);
}

#[test]
fn data_url_cache_reuses_decoded_bytes() {
    let _guard = lock_statics();
    let src = "data:image/png;base64,QUJD";

    let first = image_source_bytes(src, 7).expect("first decode");
    assert_eq!(first.as_ref(), b"ABC");
    assert_eq!(data_url_cache_len_for_tests(), 1);

    let second = image_source_bytes(src, 7).expect("cached decode");
    assert!(std::sync::Arc::ptr_eq(&first, &second));
    assert_eq!(data_url_cache_len_for_tests(), 1);
}

/// A captured page's `data:image/svg+xml` fallback rasterizes to PNG at the
/// cache seam — skia and CanvasKit decode neither, so caching the raw SVG
/// bytes would paint the placeholder forever.
#[test]
fn an_svg_data_url_is_rasterized_into_png_bytes() {
    let _guard = lock_statics();
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><rect width="8" height="8" fill="#0000ff"/></svg>"##;
    use base64::engine::general_purpose::STANDARD as B64;
    use base64::Engine as _;
    let src = format!("data:image/svg+xml;base64,{}", B64.encode(svg));

    let bytes = image_source_bytes(&src, 11).expect("decode");
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "cached as PNG");
}

/// CSS embeds icons as percent-encoded (non-base64) svg data URIs; those
/// must decode and rasterize the same way.
#[test]
fn a_percent_encoded_svg_data_url_also_rasterizes() {
    let _guard = lock_statics();
    let src = "data:image/svg+xml,%3Csvg%20xmlns=%22http://www.w3.org/2000/svg%22%20width=%224%22%20height=%224%22%3E%3Crect%20width=%224%22%20height=%224%22%20fill=%22%23ff00ff%22/%3E%3C/svg%3E";

    let bytes = image_source_bytes(src, 12).expect("decode");
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "cached as PNG");
}

#[test]
fn undecoded_image_is_queued_without_drawing_encoded_bytes() {
    let _guard = lock_statics();
    let mut node = SceneNode::leaf("image", NodeKind::Rect);
    node.image_src_id = 777;
    let mut backend = ImageRadiusCaptureBackend {
        decode_ready: Some(false),
        ..Default::default()
    };

    paint_image_node(
        &mut PaintCx {
            backend: &mut backend,
        },
        &node,
        Rect::xywh(0.0, 0.0, 100.0, 50.0),
        1.0,
        "data:image/png;base64,QUJD",
        true,
    );

    assert!(backend.image_transforms.is_empty());
    assert_eq!(
        take_pending_decodes(8)
            .into_iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>(),
        vec![777]
    );
    assert_eq!(cached_bytes_for(777).as_deref(), Some(b"ABC".as_slice()));
}

/// Eviction is LRU by byte budget: a payload set over budget must
/// drop the least-recently-USED entry, not the least-recently
/// inserted one — with hundreds of images painting every frame,
/// FIFO eviction of a still-visible image re-decodes it each paint.
#[test]
fn data_url_cache_evicts_least_recently_used_over_byte_budget() {
    let _guard = lock_statics();
    let payload = |b: u8| Arc::from(vec![b; 8].into_boxed_slice());
    let mut cache = DataUrlCache::new();
    cache.insert(1, payload(1));
    cache.insert(2, payload(2));
    cache.insert(3, payload(3));
    assert_eq!(cache.bytes, 24);
    // Touch id 1 so id 2 becomes least-recently-used.
    assert!(cache.get(1).is_some());
    cache.evict_over(16, usize::MAX);
    assert!(cache.contains(1), "recently used entry survives");
    assert!(!cache.contains(2), "LRU entry is evicted");
    assert!(cache.contains(3));
    assert_eq!(cache.bytes, 16, "byte accounting tracks eviction");
    // Entry cap applies independently of the byte budget.
    cache.evict_over(usize::MAX, 1);
    assert_eq!(cache.entries.len(), 1);
    // Re-inserting an existing id replaces its bytes, not doubles.
    let survivor = *cache.entries.keys().next().expect("one entry");
    cache.insert(survivor, Arc::from(vec![9u8; 4].into_boxed_slice()));
    assert_eq!(cache.bytes, 4, "replacement swaps byte accounting");
}

#[test]
fn remote_miss_is_queued_once_and_drained_by_the_host() {
    let _guard = lock_statics();
    let url = "https://example.com/cat.png";

    // First paint records the miss; repeated paints don't dup it.
    assert!(image_source_bytes(url, 101).is_none());
    assert!(image_source_bytes(url, 101).is_none());
    assert!(has_pending_remote_image_requests());
    let taken = take_remote_image_requests(8);
    assert_eq!(taken, vec![(101, url.to_string())]);
    assert!(!has_pending_remote_image_requests());

    assert!(image_source_bytes(url, 101).is_none());
    assert!(take_remote_image_requests(8).is_empty());
}

#[test]
fn stored_remote_bytes_hit_the_shared_cache_on_next_paint() {
    let _guard = lock_statics();
    let url = "https://example.com/dog.png";

    assert!(image_source_bytes(url, 102).is_none());
    let _ = take_remote_image_requests(8);
    store_remote_image_bytes(102, b"PNGBYTES".to_vec());

    let bytes = image_source_bytes(url, 102).expect("cache hit after store");
    assert_eq!(bytes.as_ref(), b"PNGBYTES");
    assert!(!has_pending_remote_image_requests());
}

#[test]
fn failed_fetch_is_negative_cached_and_never_requeued() {
    let _guard = lock_statics();
    let url = "https://example.com/404.png";

    assert!(image_source_bytes(url, 103).is_none());
    let _ = take_remote_image_requests(8);
    mark_remote_image_failed(103);

    // Paint keeps missing but the miss never re-queues.
    assert!(image_source_bytes(url, 103).is_none());
    assert!(!has_pending_remote_image_requests());
    assert!(take_remote_image_requests(8).is_empty());
}

#[test]
fn empty_stored_bytes_count_as_failure() {
    let _guard = lock_statics();
    let url = "https://example.com/empty.png";

    assert!(image_source_bytes(url, 104).is_none());
    let _ = take_remote_image_requests(8);
    store_remote_image_bytes(104, Vec::new());

    assert!(image_source_bytes(url, 104).is_none());
    assert!(!has_pending_remote_image_requests());
}

#[test]
fn miss_queue_is_bounded() {
    let _guard = lock_statics();
    for i in 0..(REMOTE_MISS_QUEUE_CAP as u64 + 10) {
        note_remote_image_miss(200 + i, "https://example.com/n.png");
    }
    let taken = take_remote_image_requests(usize::MAX);
    assert_eq!(taken.len(), REMOTE_MISS_QUEUE_CAP);
}

#[test]
fn transformed_crop_requests_source_resolution_for_visible_uv_window() {
    let tesla_transform = Some([0.5089059, 0.0, 0.490246, 0.0, 0.28951487, 0.37636933]);
    assert_eq!(
        required_raster_edge_with_transform(
            Rect::xywh(0.0, 0.0, 195.25409, 240.81339),
            1.0,
            tesla_transform,
        ),
        1024,
        "the 0.2895-high UV crop needs roughly 832 source pixels"
    );
    assert_eq!(
        required_raster_edge(Rect::xywh(0.0, 0.0, 195.25409, 240.81339), 1.0),
        256,
        "an untransformed image keeps the normal target-sized decode"
    );
}
