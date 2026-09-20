//! End-to-end capture of the left rail's slides tab against a real deck
//! template: open `pitch-deck-dark.op`, paint frames until every board
//! thumbnail has rendered, and write the rail out as a PNG for review.
//!
//! Also reports the per-thumbnail render cost, which is the number the
//! per-frame budget in `slides_panel_thumbs.rs` is sized against. Set
//! `OPENPENCIL_SLIDES_PANEL_SHOT` to a path to keep the image; without
//! it the test still runs the whole loop and asserts the rail filled in.
//!
//! Windows-gated like every other test that solves layout through
//! `jian_skia::SkiaMeasure`.

#![cfg(all(feature = "gl-host", not(target_os = "windows")))]

use op_editor_core::scene_template_catalog::TemplateScene;
use op_editor_core::{EditorState, LeftPanelTab};
use op_host_native::backend::{NativeBackend, NativeFrameBackend};
use op_host_native::WidgetHostNative;

const VW: i32 = 1_400;
const VH: i32 = 900;
const TEMPLATE: &str =
    include_str!("../../op-editor-core/assets/scene_templates/pitch-deck-dark.op");

#[test]
fn a_real_deck_fills_the_rail_with_rendered_thumbnails() {
    let document = jian_ops_schema::load_str(TEMPLATE)
        .expect("the pitch-deck-dark template parses")
        .value;
    let mut host = WidgetHostNative::new();
    let mut state = EditorState::from_document(document);
    state.editor_ui.scenario = Some(TemplateScene::Slides);
    host.install_imported_state(state);
    host.editor_state_mut().editor_ui.slides_panel.tab = LeftPanelTab::Slides;
    host.editor_state_mut().editor_ui.slide_thumbnails_supported = true;

    let mut backend = NativeBackend::with_dpi(1.0);
    let mut surface =
        skia_safe::surfaces::raster_n32_premul((VW, VH)).expect("raster surface allocated");

    // Paint until the cache stops asking to be woken. The clock is
    // advanced past the debounce each frame, which is what a settled
    // document looks like to the panel.
    let mut now_ms = 1_000u64;
    let mut frames = 0;
    let started = std::time::Instant::now();
    for _ in 0..40 {
        host.set_now_ms(now_ms);
        {
            let mut frame = NativeFrameBackend::new(&mut backend, surface.canvas());
            host.paint(&mut frame, VW as f32, VH as f32);
        }
        frames += 1;
        if host.slide_thumbnail_render_count() >= 6 {
            break;
        }
        now_ms += 500;
    }
    let elapsed = started.elapsed();
    // One settling frame: a raster produced during frame N is blitted by
    // frame N+1, because the blit runs before the render inside a frame
    // (a row that already has pixels must show them THIS frame, whether
    // or not it is also due a re-render). The running app gets that
    // frame from the 16 ms wake the cache arms; the capture asks for it.
    now_ms += 500;
    host.set_now_ms(now_ms);
    {
        let mut frame = NativeFrameBackend::new(&mut backend, surface.canvas());
        host.paint(&mut frame, VW as f32, VH as f32);
    }

    let rendered = host.slide_thumbnail_render_count();
    assert_eq!(
        rendered, 6,
        "every board in the deck got a thumbnail (after {frames} frames)"
    );
    // Two per frame, plus the debounce beat before the first batch.
    assert!(
        frames <= 5,
        "six thumbnails should take about three painting frames, took {frames}"
    );
    eprintln!(
        "slides panel: {rendered} thumbnails over {frames} frames in {:?} \
         (~{:?} per thumbnail, paint pass included)",
        elapsed,
        elapsed / rendered.max(1) as u32
    );

    if let Ok(path) = std::env::var("OPENPENCIL_SLIDES_PANEL_SHOT") {
        let image = surface.image_snapshot();
        // Crop to the rail plus a slice of canvas, so the shot shows the
        // panel in context rather than floating.
        let rail = image
            .make_subset(
                None,
                skia_safe::IRect::from_xywh(0, 0, 320, VH),
                skia_safe::image::RequiredProperties::default(),
            )
            .expect("crop the rail");
        let data = rail
            .encode(None, skia_safe::EncodedImageFormat::PNG, 100)
            .expect("encode PNG");
        std::fs::write(&path, data.as_bytes()).expect("write the shot");
        eprintln!("slides panel shot written to {path}");
    }
}
