use super::{load_editor_state_with_report, sidecar_path};
use std::path::PathBuf;

fn temp_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "openpencil-load-report-{tag}-{}-{}.op",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ))
}

#[test]
fn reports_normalization_and_allows_unknown_field_preserving_rewrites() {
    let safe = temp_path("safe-normalize");
    std::fs::write(
        &safe,
        r#"{"version":"1.0","children":[{"type":"path","id":"p","geometry":"M0 0L1 1"}]}"#,
    )
    .expect("write safe fixture");
    let loaded = load_editor_state_with_report(&safe, op_editor_core::Locale::EnUs)
        .expect("safe fixture loads");
    assert!(loaded.report.normalized_legacy);
    assert!(loaded.report.needs_schema_upgrade());

    let unknown = temp_path("unknown");
    std::fs::write(
        &unknown,
        r#"{"version":"1.0","mystery":{"keep":true},"children":[{"type":"path","id":"p","geometry":"M0 0L1 1"}]}"#,
    )
    .expect("write unknown fixture");
    let loaded = load_editor_state_with_report(&unknown, op_editor_core::Locale::EnUs)
        .expect("unknown fixture loads");
    assert!(loaded.report.normalized_legacy);
    assert!(!loaded.report.rewrite_blocked_by_schema_warning);
    assert!(loaded.report.needs_schema_upgrade());

    let future_minor = temp_path("future-minor");
    std::fs::write(
        &future_minor,
        r#"{"version":"1.0","formatVersion":"1.3","children":[{"type":"path","id":"p","geometry":"M0 0L1 1"}]}"#,
    )
    .expect("write future-minor fixture");
    let loaded = load_editor_state_with_report(&future_minor, op_editor_core::Locale::EnUs)
        .expect("future-minor fixture loads best effort");
    assert!(loaded.report.normalized_legacy);
    assert!(loaded.report.rewrite_blocked_by_schema_warning);
    assert!(!loaded.report.needs_schema_upgrade());

    let malformed = temp_path("malformed-format");
    std::fs::write(
        &malformed,
        r#"{"version":"1.0","formatVersion":"next","children":[{"type":"path","id":"p","geometry":"M0 0L1 1"}]}"#,
    )
    .expect("write malformed-version fixture");
    let loaded = load_editor_state_with_report(&malformed, op_editor_core::Locale::EnUs)
        .expect("malformed-version fixture remains loadable");
    assert!(loaded.report.normalized_legacy);
    assert!(loaded.report.rewrite_blocked_by_schema_warning);
    assert!(!loaded.report.needs_schema_upgrade());

    let _ = std::fs::remove_file(safe);
    let _ = std::fs::remove_file(unknown);
    let _ = std::fs::remove_file(future_minor);
    let _ = std::fs::remove_file(malformed);
}

#[test]
fn reports_only_adopted_sidecar_and_reliable_editor_meta_inference() {
    let sidecar_doc = temp_path("sidecar");
    std::fs::write(&sidecar_doc, r#"{"version":"1.0","children":[]}"#)
        .expect("write sidecar document");
    std::fs::write(
        sidecar_path(&sidecar_doc),
        r#"{"active_page_index":0,"preserve_authored_geometry":true}"#,
    )
    .expect("write legacy sidecar");
    let loaded = load_editor_state_with_report(&sidecar_doc, op_editor_core::Locale::EnUs)
        .expect("sidecar document loads");
    assert!(loaded.report.used_legacy_sidecar);
    assert!(loaded.report.needs_schema_upgrade());

    let inferred = temp_path("inferred");
    std::fs::write(
        &inferred,
        r#"{"version":"1.0","editorMeta":{"activePageIndex":0},"pages":[{"id":"figma-page-0","name":"P","children":[]}],"children":[]}"#,
    )
    .expect("write inferred metadata document");
    let loaded = load_editor_state_with_report(&inferred, op_editor_core::Locale::EnUs)
        .expect("inferred metadata document loads");
    assert!(loaded.report.inferred_editor_meta);
    assert!(loaded.report.needs_schema_upgrade());

    let ordinary = temp_path("ordinary-missing-meta");
    std::fs::write(&ordinary, r#"{"version":"1.0","children":[]}"#)
        .expect("write ordinary document");
    let loaded = load_editor_state_with_report(&ordinary, op_editor_core::Locale::EnUs)
        .expect("ordinary document loads");
    assert!(!loaded.report.needs_schema_upgrade());

    let _ = std::fs::remove_file(sidecar_path(&sidecar_doc));
    let _ = std::fs::remove_file(sidecar_doc);
    let _ = std::fs::remove_file(inferred);
    let _ = std::fs::remove_file(ordinary);
}

/// Top-level frames open collapsed in the LayerPanel.
///
/// A six-slide deck expands to ~90 rows, pushing the boards themselves off
/// the panel — the list stops answering "what is in this document", which is
/// the only question it exists for.
#[test]
fn loading_collapses_top_level_frames_but_not_leaves() {
    let source = serde_json::json!({
        "version": "1.0",
        "children": [
            {
                "type": "frame", "id": "board-1", "name": "01", "width": 1920, "height": 1080,
                "children": [{"type": "text", "id": "t1", "content": "hi"}]
            },
            {
                "type": "frame", "id": "board-2", "name": "02", "width": 1920, "height": 1080,
                "children": [{"type": "text", "id": "t2", "content": "hi"}]
            },
            // A childless top-level node: collapsing it would only render a
            // disclosure arrow that does nothing.
            {"type": "rectangle", "id": "loose", "name": "Loose", "width": 10, "height": 10}
        ]
    })
    .to_string();

    let state =
        super::load_editor_state_from_source(&source, op_editor_core::Locale::EnUs).expect("loads");
    let collapsed = &state.editor_ui.collapsed_layers;
    assert!(collapsed.contains(&op_editor_core::NodeId::new("board-1")));
    assert!(collapsed.contains(&op_editor_core::NodeId::new("board-2")));
    assert!(
        !collapsed.contains(&op_editor_core::NodeId::new("loose")),
        "a leaf has nothing to collapse"
    );
}
