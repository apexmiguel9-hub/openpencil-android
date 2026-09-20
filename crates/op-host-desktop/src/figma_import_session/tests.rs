use super::publish::PersistedFile;
use super::*;

fn temp_import_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!(
        "op-figma-session-{tag}-{}-{nanos}",
        std::process::id()
    ))
}

fn confirmed_replace_mode(output_path: &Path) -> ImportOutputMode {
    ImportOutputMode::ReplaceFixed {
        expected: capture_output_state(output_path).expect("capture confirmed output state"),
    }
}

#[derive(Default)]
struct ByteCounter(u64);

impl std::io::Write for ByteCounter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 += bytes.len() as u64;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedTableProbe {
    #[serde(default)]
    images: std::collections::BTreeMap<String, serde::de::IgnoredAny>,
    #[serde(default)]
    image_thumbs: std::collections::BTreeMap<String, String>,
}

fn compact_json_size(value: &impl serde::Serialize) -> u64 {
    let mut counter = ByteCounter::default();
    serde_json::to_writer(&mut counter, value).expect("count compact JSON bytes");
    counter.0
}

fn probe_saved_tables(path: &Path) -> SavedTableProbe {
    let file = std::fs::File::open(path).expect("open saved document");
    let mut deserializer = serde_json::Deserializer::from_reader(std::io::BufReader::new(file));
    deserializer.disable_recursion_limit();
    <SavedTableProbe as serde::Deserialize>::deserialize(&mut deserializer)
        .expect("probe saved tables")
}

fn no_thumbs_path(path: &Path) -> PathBuf {
    let stem = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("openpencil-tesla-blurup");
    path.with_file_name(format!("{stem}-no-thumbs.op"))
}

fn select_summary_page(state: &mut EditorState) -> usize {
    let pages = state.doc.pages.as_ref().expect("Tesla document has pages");
    let page_index = pages
        .iter()
        .position(|page| page.name.ends_with("汇总稿"))
        .unwrap_or_else(|| {
            panic!(
                "汇总稿 missing; pages={:?}",
                pages
                    .iter()
                    .map(|page| page.name.as_str())
                    .collect::<Vec<_>>()
            )
        });
    assert!(state.set_active_page(page_index));
    page_index
}

fn paint_and_pump_decode_frame(
    host: &mut WidgetHostNative,
    backend: &mut op_host_native::NativeBackend,
    image_decodes: &mut crate::image_decode_host::ImageDecodeHost,
    surface: &mut skia_safe::Surface,
    started: std::time::Instant,
) {
    const WIDTH: f32 = 1440.0;
    const HEIGHT: f32 = 900.0;
    surface.canvas().restore_to_count(1);
    surface.canvas().reset_matrix();
    surface.canvas().clear(skia_safe::Color::BLACK);
    host.set_now_ms(started.elapsed().as_millis() as u64);
    {
        let mut frame = op_host_native::NativeFrameBackend::new(backend, surface.canvas());
        host.paint(&mut frame, WIDTH, HEIGHT);
    }
    image_decodes.pump(backend);
}

fn write_surface_png(surface: &mut skia_safe::Surface, path: &Path) {
    let png = surface
        .image_snapshot()
        .encode(None, skia_safe::EncodedImageFormat::PNG, 100)
        .expect("encode verification frame as PNG");
    std::fs::write(path, png.as_bytes()).expect("write verification frame PNG");
}

fn run_zoomed_out_decode_probe(mut state: EditorState, mut host: WidgetHostNative) {
    use op_editor_ui::widgets::canvas_viewport_image::pending_decode_count;
    use std::time::{Duration, Instant};

    const WIDTH: f32 = 1440.0;
    const HEIGHT: f32 = 900.0;
    const FRAME_PERIOD: Duration = Duration::from_millis(16);
    const SETTLE_DEADLINE: Duration = Duration::from_secs(120);
    const STABLE_IDLE: Duration = Duration::from_secs(3);
    const IDLE_SAMPLE: Duration = Duration::from_secs(5);

    let page_index = select_summary_page(&mut state);
    host.install_imported_state(state);
    host.fit_content_to_viewport(WIDTH, HEIGHT);
    let mut backend = op_host_native::NativeBackend::with_dpi(1.0);
    let mut surface = skia_safe::surfaces::raster_n32_premul((WIDTH as i32, HEIGHT as i32))
        .expect("offscreen raster surface");
    let mut image_decodes = crate::image_decode_host::ImageDecodeHost::new();
    let started = Instant::now();
    let mut frames = 0u64;
    let mut idle_candidate = None;
    let mut settled = false;

    op_host_native::begin_image_paint_diagnostics();
    paint_and_pump_decode_frame(
        &mut host,
        &mut backend,
        &mut image_decodes,
        &mut surface,
        started,
    );
    frames += 1;
    let first_frame = op_host_native::image_paint_diagnostics_snapshot();
    assert!(
        first_frame.successful_thumbnail_draws > 0,
        "the reopened first frame must draw persisted blur-up thumbnails"
    );
    assert_eq!(
        first_frame.sharp_raster_hits, 0,
        "the reopened first frame starts before full rasters are installed"
    );
    let blur_capture = Path::new("/private/tmp/openpencil-tesla-blur-first.png");
    write_surface_png(&mut surface, blur_capture);

    while started.elapsed() < SETTLE_DEADLINE {
        let frame_started = Instant::now();
        paint_and_pump_decode_frame(
            &mut host,
            &mut backend,
            &mut image_decodes,
            &mut surface,
            started,
        );
        frames += 1;
        let queue_empty = !image_decodes.is_pending() && pending_decode_count() == 0;
        if queue_empty {
            let idle_since = idle_candidate.get_or_insert_with(Instant::now);
            if idle_since.elapsed() >= STABLE_IDLE {
                settled = true;
                break;
            }
        } else {
            idle_candidate = None;
        }
        if let Some(remaining) = FRAME_PERIOD.checked_sub(frame_started.elapsed()) {
            std::thread::sleep(remaining);
        }
    }

    let settled_at = started.elapsed();
    let before_idle = image_decodes
        .stats_snapshot()
        .expect("set OP_IMAGE_DECODE_STATS=1 for the real-file decode probe");
    let idle_started = Instant::now();
    while idle_started.elapsed() < IDLE_SAMPLE {
        let frame_started = Instant::now();
        paint_and_pump_decode_frame(
            &mut host,
            &mut backend,
            &mut image_decodes,
            &mut surface,
            started,
        );
        frames += 1;
        if let Some(remaining) = FRAME_PERIOD.checked_sub(frame_started.elapsed()) {
            std::thread::sleep(remaining);
        }
    }
    let idle_elapsed = idle_started.elapsed();
    let after_idle = image_decodes
        .stats_snapshot()
        .expect("decode telemetry remains active");
    let queue_pending = pending_decode_count();
    let diagnostics = op_host_native::end_image_paint_diagnostics();
    let sharp_capture = Path::new("/private/tmp/openpencil-tesla-sharp-terminal.png");
    write_surface_png(&mut surface, sharp_capture);
    let idle_installs = after_idle.0.saturating_sub(before_idle.0);
    let idle_reinstalls = after_idle.1.saturating_sub(before_idle.1);
    let sample_secs = idle_elapsed.as_secs_f64();
    assert!(after_idle.0 > 0, "the harness must install sharp rasters");
    assert!(
        diagnostics.sharp_raster_hits > 0,
        "later frames must sharpen at least one blur-up thumbnail"
    );
    assert_eq!(
        diagnostics.paint_thread_full_decodes, 0,
        "full images must never decode synchronously on the paint thread"
    );
    eprintln!(
        "fig_decode_probe: page_index={page_index}, frames={frames}, settled={settled}, \
         settle_window={:.1}s, terminal_sample={sample_secs:.1}s, terminal_installs={idle_installs}, \
         terminal_reinstalls={idle_reinstalls}, terminal_installs_per_sec={:.1}, \
         terminal_reinstalls_per_sec={:.1}, installs_total={}, reinstalls_total={}, in_flight={}, \
         pending={}, state={}, first_thumb_draws={}, first_sharp_hits={}, thumb_draws_total={}, \
         sharp_hits_total={}, paint_sync_full_decodes={}, blur_capture={}, sharp_capture={}",
        settled_at.as_secs_f64(),
        idle_installs as f64 / sample_secs,
        idle_reinstalls as f64 / sample_secs,
        after_idle.0,
        after_idle.1,
        after_idle.2,
        queue_pending,
        after_idle.4,
        first_frame.successful_thumbnail_draws,
        first_frame.sharp_raster_hits,
        diagnostics.successful_thumbnail_draws,
        diagnostics.sharp_raster_hits,
        diagnostics.paint_thread_full_decodes,
        blur_capture.display(),
        sharp_capture.display(),
    );
}

#[test]
fn deferred_thumbnail_binding_uses_the_final_source_string() {
    use base64::engine::general_purpose::STANDARD as B64;
    use base64::Engine as _;
    use jian_ops_schema::node::image_src::paint_image_id;

    let provisional = b"raw import bytes that are replaced";
    let final_bytes = b"final transformed bytes";
    let final_src = format!("data:image/jpeg;base64,{}", B64.encode(final_bytes));
    let provisional_src = format!("data:image/png;base64,{}", B64.encode(provisional));
    let doc = serde_json::from_value(serde_json::json!({
        "version": "1.0.0",
        "children": [{
            "type": "rectangle",
            "id": "image-fill",
            "fill": [{"type": "image", "url": final_src}]
        }]
    }))
    .expect("test document");
    let jpeg = vec![0xff, 0xd8, 0xff, 0xd9];
    let mut pending = PendingImportThumbs::default();
    pending.record(final_bytes, jpeg.clone());

    bind_import_thumbnails(&doc, &mut pending);

    let final_id = paint_image_id(&final_src);
    assert_eq!(
        &*jian_ops_schema::image_thumbs::thumb_for(final_id).expect("final id is bound"),
        jpeg
    );
    assert!(
        jian_ops_schema::image_thumbs::thumb_for(paint_image_id(&provisional_src)).is_none(),
        "the callback's provisional source is never persisted"
    );
}

#[test]
fn adjacent_output_uses_the_figma_source_stem() {
    let source = Path::new("/designs/Ant Design Open Source (Community).FIG");
    assert_eq!(
        adjacent_op_base_path(source).unwrap(),
        PathBuf::from("/designs/Ant Design Open Source (Community).op")
    );
}

#[test]
fn existing_adjacent_output_maps_replace_copy_and_cancel_choices() {
    let dir = temp_import_dir("overwrite-choice");
    std::fs::create_dir_all(&dir).expect("create import directory");
    let source = dir.join("Design.fig");
    std::fs::write(&source, b"fig fixture").expect("write source marker");

    assert_eq!(
        select_output_mode(&source, |_| panic!("new output needs no prompt")).unwrap(),
        Some(ImportOutputMode::CreateFixed)
    );

    std::fs::write(dir.join("Design.op"), b"existing OP").expect("write existing OP");
    assert_eq!(
        select_output_mode(&source, |_| ExistingOutputDecision::Replace).unwrap(),
        Some(confirmed_replace_mode(&dir.join("Design.op")))
    );
    assert_eq!(
        select_output_mode(&source, |_| ExistingOutputDecision::NumberedCopy).unwrap(),
        Some(ImportOutputMode::NumberedCopy)
    );
    assert_eq!(
        select_output_mode(&source, |_| ExistingOutputDecision::Cancel).unwrap(),
        None
    );

    std::fs::remove_dir_all(dir).expect("remove import directory");
}

#[test]
fn adjacent_persist_atomically_replaces_the_fixed_sibling_op() {
    let dir = temp_import_dir("adjacent-replace");
    std::fs::create_dir_all(&dir).expect("create import directory");
    let source = dir.join("Design.fig");
    let output = dir.join("Design.op");
    let stale_sidecar = op_host_services::doc_io::sidecar_path(&output);
    std::fs::write(&source, b"fig fixture").expect("write source marker");
    std::fs::write(&output, b"previous OP contents").expect("write prior OP");
    std::fs::write(&stale_sidecar, b"stale editor metadata").expect("write stale sidecar");

    let imported = op_pen_loader::load_canonical(
        r#"{"version":"1.0.0","pages":[{"id":"figma-page-0","name":"Imported","children":[{"type":"frame","id":"latest","name":"Latest Version","layout":"vertical","children":[{"type":"rectangle","id":"content","name":"content"},{"type":"rectangle","id":"vector","name":"Vector"}]}]}],"children":[]}"#,
    )
    .expect("load imported-order fixture");
    let mut state = EditorState::from_document(imported.value);
    state.editor_ui.preserve_authored_geometry = true;
    let completed = persist_import_next_to_source(
        PreparedImport {
            state,
            warnings: Vec::new(),
        },
        &source,
        confirmed_replace_mode(&output),
        &CancellationToken::default(),
    );
    let persisted = completed.persisted.expect("persist adjacent OP");
    let output_path = persisted.commit();

    assert_eq!(output_path, output);
    let reloaded =
        op_host_services::doc_io::load_editor_state(&output, op_editor_core::Locale::EnUs)
            .expect("committed OP is loadable");
    assert!(
        reloaded.editor_ui.preserve_authored_geometry,
        "adjacent Figma OP must preserve authored geometry when reopened"
    );
    let pages = reloaded.doc.pages.as_deref().expect("reopened pages");
    let jian_ops_schema::node::PenNode::Frame(frame) = &pages[0].children[0] else {
        panic!("Latest Version frame expected");
    };
    let children = frame.children.as_deref().expect("frame children");
    assert!(matches!(children, [
        jian_ops_schema::node::PenNode::Rectangle(content),
        jian_ops_schema::node::PenNode::Rectangle(vector)
    ] if content.base.name.as_deref() == Some("content")
        && vector.base.name.as_deref() == Some("Vector")));
    assert!(
        !dir.join("Design (1).op").exists(),
        "re-import must reuse Design.op rather than create a numbered copy"
    );
    assert!(
        !stale_sidecar.exists(),
        "atomic replacement must remove the old destination sidecar"
    );
    assert!(std::fs::read_dir(&dir)
        .expect("read import directory")
        .all(|entry| !entry
            .expect("read directory entry")
            .file_name()
            .to_string_lossy()
            .contains("op-import")));

    std::fs::remove_dir_all(dir).expect("remove import directory");
}

#[test]
fn declined_overwrite_publishes_the_next_numbered_copy() {
    let dir = temp_import_dir("adjacent-numbered");
    std::fs::create_dir_all(&dir).expect("create import directory");
    let source = dir.join("Design.fig");
    let fixed = dir.join("Design.op");
    let first_copy = dir.join("Design (1).op");
    let expected = dir.join("Design (2).op");
    std::fs::write(&source, b"fig fixture").expect("write source marker");
    std::fs::write(&fixed, b"fixed original").expect("write fixed OP");
    std::fs::write(&first_copy, b"first copy").expect("write first numbered OP");

    let completed = persist_import_next_to_source(
        PreparedImport {
            state: EditorState::starter(),
            warnings: Vec::new(),
        },
        &source,
        ImportOutputMode::NumberedCopy,
        &CancellationToken::default(),
    );
    let output = completed.persisted.expect("persist numbered OP").commit();

    assert_eq!(output, expected);
    assert_eq!(std::fs::read(&fixed).unwrap(), b"fixed original");
    assert_eq!(std::fs::read(&first_copy).unwrap(), b"first copy");
    op_host_services::doc_io::load_editor_state(&output, op_editor_core::Locale::EnUs)
        .expect("numbered copy is loadable");

    std::fs::remove_dir_all(dir).expect("remove import directory");
}

#[test]
fn confirmed_overwrite_preserves_a_fixed_output_changed_during_conversion() {
    let dir = temp_import_dir("adjacent-replace-changed");
    std::fs::create_dir_all(&dir).expect("create import directory");
    let source = dir.join("Design.fig");
    let fixed = dir.join("Design.op");
    let expected = dir.join("Design (1).op");
    std::fs::write(&source, b"fig fixture").expect("write source marker");
    std::fs::write(&fixed, b"version approved for replacement").expect("write fixed OP");
    let output_mode = select_output_mode(&source, |_| ExistingOutputDecision::Replace)
        .expect("inspect fixed output")
        .expect("replace is selected");

    std::fs::write(&fixed, b"external update after overwrite confirmation")
        .expect("update fixed OP during conversion");
    let completed = persist_import_next_to_source(
        PreparedImport {
            state: EditorState::starter(),
            warnings: Vec::new(),
        },
        &source,
        output_mode,
        &CancellationToken::default(),
    );
    let output = completed.persisted.expect("persist fallback OP").commit();

    assert_eq!(output, expected);
    assert_eq!(
        std::fs::read(&fixed).expect("fixed output remains readable"),
        b"external update after overwrite confirmation"
    );
    op_host_services::doc_io::load_editor_state(&output, op_editor_core::Locale::EnUs)
        .expect("fallback numbered copy is loadable");

    std::fs::remove_dir_all(dir).expect("remove import directory");
}

#[test]
fn confirmed_overwrite_preserves_a_fixed_output_created_after_confirmation() {
    let dir = temp_import_dir("adjacent-replace-created");
    std::fs::create_dir_all(&dir).expect("create import directory");
    let source = dir.join("Design.fig");
    let fixed = dir.join("Design.op");
    let expected = dir.join("Design (1).op");
    std::fs::write(&source, b"fig fixture").expect("write source marker");
    std::fs::write(&fixed, b"output shown by the dialog").expect("write fixed OP");
    let output_mode = select_output_mode(&source, |path| {
        std::fs::remove_file(path).expect("remove output while dialog is open");
        ExistingOutputDecision::Replace
    })
    .expect("capture the missing state after confirmation")
    .expect("replace is selected");

    std::fs::write(&fixed, b"new output created while conversion runs")
        .expect("create racing fixed OP");
    let completed = persist_import_next_to_source(
        PreparedImport {
            state: EditorState::starter(),
            warnings: Vec::new(),
        },
        &source,
        output_mode,
        &CancellationToken::default(),
    );
    let output = completed.persisted.expect("persist fallback OP").commit();

    assert_eq!(output, expected);
    assert_eq!(
        std::fs::read(&fixed).expect("fixed output remains readable"),
        b"new output created while conversion runs"
    );

    std::fs::remove_dir_all(dir).expect("remove import directory");
}

#[test]
fn fixed_output_appearing_after_precheck_falls_back_to_a_numbered_copy() {
    let dir = temp_import_dir("adjacent-create-race");
    std::fs::create_dir_all(&dir).expect("create import directory");
    let source = dir.join("Design.fig");
    let fixed = dir.join("Design.op");
    let expected = dir.join("Design (1).op");
    std::fs::write(&source, b"fig fixture").expect("write source marker");
    // `CreateFixed` represents the earlier no-file precheck. This file then
    // appears before publication, as another process or import could create it.
    std::fs::write(&fixed, b"racing OP").expect("write racing fixed OP");

    let completed = persist_import_next_to_source(
        PreparedImport {
            state: EditorState::starter(),
            warnings: Vec::new(),
        },
        &source,
        ImportOutputMode::CreateFixed,
        &CancellationToken::default(),
    );
    let output = completed.persisted.expect("persist fallback OP").commit();

    assert_eq!(output, expected);
    assert_eq!(std::fs::read(&fixed).unwrap(), b"racing OP");
    op_host_services::doc_io::load_editor_state(&output, op_editor_core::Locale::EnUs)
        .expect("fallback numbered copy is loadable");

    std::fs::remove_dir_all(dir).expect("remove import directory");
}

#[test]
fn adjacent_save_failure_keeps_the_converted_state_available() {
    let source = temp_import_dir("missing-parent").join("Design.fig");
    let expected = EditorState::starter().doc;
    let completed = persist_import_next_to_source(
        PreparedImport {
            state: EditorState::starter(),
            warnings: vec!["conversion warning".to_string()],
        },
        &source,
        ImportOutputMode::CreateFixed,
        &CancellationToken::default(),
    );

    assert!(completed.persisted.is_err());
    assert_eq!(completed.prepared.state.doc, expected);
    assert_eq!(
        completed.prepared.warnings,
        vec!["conversion warning".to_string()]
    );
}

#[test]
fn unadopted_completed_import_keeps_the_fixed_output() {
    let output = temp_import_dir("unadopted-import").with_extension("op");
    std::fs::write(&output, b"completed replacement").expect("write replacement fixture");

    drop(PersistedFile::new(output.clone()));

    assert_eq!(
        std::fs::read(&output).expect("fixed output remains readable"),
        b"completed replacement"
    );
    std::fs::remove_file(output).expect("remove replacement fixture");
}

#[test]
fn failed_adjacent_save_installs_unsaved_import_and_requests_save_as() {
    let mut imported = EditorState::starter();
    imported.doc.name = Some("Imported despite save failure".to_string());
    let completed = CompletedImport {
        prepared: PreparedImport {
            state: imported,
            warnings: Vec::new(),
        },
        persisted: Err(FigmaImportError::WriteStaged {
            source_path: PathBuf::from("/fixtures/design.fig"),
            message: "source directory is read-only".to_string(),
        }),
    };
    let mut host = WidgetHostNative::new();
    let mut current_path = Some(PathBuf::from("/old/document.op"));
    let reported = RefCell::new(None::<String>);

    let outcome = apply_completed_import(
        &mut host,
        completed,
        &mut current_path,
        None,
        |_host, detail| *reported.borrow_mut() = Some(detail.to_string()),
    );

    assert_eq!(outcome, PumpOutcome::CompletedOk);
    assert!(
        current_path.is_none(),
        "the next Save must route to Save As"
    );
    assert_eq!(
        host.editor_state().doc.name.as_deref(),
        Some("Imported despite save failure")
    );
    assert!(host.editor_state().is_dirty());
    let detail = reported.borrow();
    let detail = detail.as_deref().expect("Save As guidance is reported");
    assert!(detail.contains("read-only"));
    assert!(detail.contains("Save As"));
}

#[test]
fn completed_persisted_import_binds_current_path_and_display_name() {
    let output = temp_import_dir("current-path").with_extension("op");
    let state = EditorState::starter();
    let prepared = PreparedImport {
        state,
        warnings: Vec::new(),
    };
    let (tx, rx) = mpsc::channel();
    tx.send(Ok(CompletedImport {
        prepared,
        persisted: Ok(PersistedFile::new(output.clone())),
    }))
    .expect("queue completed import");
    let mut session = Some(FigmaImportSession {
        path: output.with_extension("fig"),
        stage: SessionStage::Converting(rx),
        cancellation: CancellationToken::default(),
        output_mode: ImportOutputMode::CreateFixed,
    });
    let mut host = WidgetHostNative::new();
    let mut current_path = None;

    let outcome = pump(&mut host, &mut session, &mut current_path, None);

    assert_eq!(outcome, PumpOutcome::CompletedSaved);
    assert_eq!(current_path.as_deref(), Some(output.as_path()));
    assert_eq!(
        host.editor_state().editor_ui.file_name_display.as_deref(),
        output.file_name().and_then(|name| name.to_str())
    );
    assert!(session.is_none());
    assert!(!host.editor_state().is_dirty());
}

/// Manual end-to-end smoke test for the desktop import persistence path.
/// It converts every page and publishes the generated `.op` beside the source
/// by atomically replacing the same fixed sibling used by the UI worker.
#[test]
#[ignore = "manual smoke — needs OP_FIG_ADJACENT_SMOKE pointing at a local .fig"]
fn fig_import_adjacent_smoke() {
    let source = PathBuf::from(
        std::env::var_os("OP_FIG_ADJACENT_SMOKE")
            .expect("OP_FIG_ADJACENT_SMOKE must point at a local .fig"),
    );
    let parse_started = std::time::Instant::now();
    let prepared = parse_path(&source).expect("smoke file parses and converts");
    let parse_elapsed = parse_started.elapsed();
    let page_count = prepared.state.doc.pages.as_ref().map_or(1, Vec::len);
    let warning_count = prepared.warnings.len();

    let persist_started = std::time::Instant::now();
    let completed = persist_import_next_to_source(
        prepared,
        &source,
        confirmed_replace_mode(&adjacent_op_base_path(&source).expect("adjacent output path")),
        &CancellationToken::default(),
    );
    let output = completed
        .persisted
        .expect("converted document publishes beside source")
        .commit();
    let persist_elapsed = persist_started.elapsed();
    let output_bytes = std::fs::metadata(&output)
        .expect("published output metadata")
        .len();

    eprintln!(
        "fig_import_adjacent_smoke: pages={page_count}, warnings={warning_count}, parse+convert={:.1}s, persist={:.1}s, output={:.1} MB, path={}",
        parse_elapsed.as_secs_f64(),
        persist_elapsed.as_secs_f64(),
        output_bytes as f64 / 1e6,
        output.display(),
    );
}

/// Manual large-file bench for the exact worker code path
/// (`parse_path`, including the down-scale transform). Point
/// `OP_FIG_BENCH` at a `.fig` file and run:
///
/// ```sh
/// OP_IMAGE_DECODE_STATS=1 OP_FIG_BENCH=/path/to/big.fig \
///   cargo test -p op-host-desktop --release fig_import_bench -- --ignored --nocapture
/// ```
#[test]
#[ignore = "manual bench — needs OP_FIG_BENCH pointing at a local .fig"]
fn fig_import_bench() {
    let Ok(path) = std::env::var("OP_FIG_BENCH") else {
        eprintln!("OP_FIG_BENCH not set — skipping");
        return;
    };
    let output = std::env::var_os("OP_FIG_BENCH_OUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(format!(
                "/private/tmp/openpencil-tesla-blurup-{}.op",
                std::process::id()
            ))
        });
    let started = std::time::Instant::now();
    let prepared = parse_path(Path::new(&path)).expect("bench file parses");
    let import_elapsed = started.elapsed();
    let PreparedImport {
        mut state,
        warnings,
    } = prepared;
    let warning_count = warnings.len();
    let page_index = select_summary_page(&mut state);
    let inline_bytes = compact_json_size(&state.doc);

    let save_started = std::time::Instant::now();
    op_host_services::doc_io::save_to_path(&state, &output).expect("production save succeeds");
    let save_elapsed = save_started.elapsed();
    let saved_bytes = std::fs::metadata(&output).expect("saved metadata").len();
    let probe = probe_saved_tables(&output);
    assert!(!probe.image_thumbs.is_empty(), "saved imageThumbs table");
    assert!(
        probe
            .image_thumbs
            .keys()
            .all(|paint_id| paint_id.parse::<u64>().is_ok()),
        "imageThumbs keys are decimal paint ids"
    );
    let first_thumb_id = probe
        .image_thumbs
        .keys()
        .next()
        .expect("one thumbnail id")
        .parse::<u64>()
        .expect("decimal thumbnail id");
    let thumb_table_bytes = compact_json_size(&probe.image_thumbs);

    jian_ops_schema::image_thumbs::clear_registry();
    assert!(jian_ops_schema::image_thumbs::thumb_for(first_thumb_id).is_none());
    let baseline = no_thumbs_path(&output);
    op_host_services::doc_io::save_to_path(&state, &baseline)
        .expect("no-thumbnail baseline save succeeds");
    let baseline_probe = probe_saved_tables(&baseline);
    assert!(
        baseline_probe.image_thumbs.is_empty(),
        "cleared registry omits imageThumbs"
    );
    let baseline_bytes = std::fs::metadata(&baseline)
        .expect("baseline metadata")
        .len();
    let thumbnail_delta = saved_bytes as i128 - baseline_bytes as i128;
    drop(state);

    // The running desktop host exists before a document is opened. Build
    // it before reload so its empty default document cannot clear the
    // thumbnail table that the production load path is about to seed.
    let host = WidgetHostNative::new();
    let reload_started = std::time::Instant::now();
    let reloaded =
        op_host_services::doc_io::load_editor_state(&output, op_editor_core::Locale::EnUs)
            .expect("production reopen succeeds");
    let reload_elapsed = reload_started.elapsed();
    assert!(
        jian_ops_schema::image_thumbs::thumb_for(first_thumb_id).is_some(),
        "reopen seeds the persisted thumbnail registry"
    );
    eprintln!(
        "fig_import_bench: page_index={page_index}, parse+downscale={:.1}s, \
         save={:.1}s, reload={:.1}s, inline={:.1} MB, saved={:.1} MB, \
         no_thumbs={:.1} MB, thumbnail_delta={:.1} MB, thumb_table={:.1} MB, \
         images={}, imageThumbs={}, warnings={warning_count}, output={}, baseline={}",
        import_elapsed.as_secs_f64(),
        save_elapsed.as_secs_f64(),
        reload_elapsed.as_secs_f64(),
        inline_bytes as f64 / 1e6,
        saved_bytes as f64 / 1e6,
        baseline_bytes as f64 / 1e6,
        thumbnail_delta as f64 / 1e6,
        thumb_table_bytes as f64 / 1e6,
        probe.images.len(),
        probe.image_thumbs.len(),
        output.display(),
        baseline.display(),
    );
    run_zoomed_out_decode_probe(reloaded, host);
    if warning_count != 356 {
        eprintln!(
            "fig_import_bench premise drift: expected 356 warnings, observed {warning_count}"
        );
    }
}
