use super::*;

fn temp_op_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "openpencil-save-clean-copy-{tag}-{}-{}.op",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ))
}

#[test]
fn clean_bound_op_save_as_streams_source_and_captures_current_editor_meta() {
    let source = temp_op_path("source");
    let target = temp_op_path("target");
    std::fs::write(
        &source,
        r#"{
          "version":"0.8.0",
          "futureExtension":{"keep":true},
          "pages":[
            {"id":"p1","name":"One","children":[]},
            {"id":"p2","name":"Two","children":[{"type":"rectangle","id":"kept"}]}
          ],
          "children":[],
          "editorMeta":{"activePageIndex":0,"preserveAuthoredGeometry":false}
        }"#,
    )
    .expect("write clean bound OP");
    let mut state =
        op_host_services::doc_io::load_editor_state(&source, op_editor_core::Locale::EnUs)
            .expect("load clean bound OP");
    assert!(state.set_active_page(1));
    state.editor_ui.preserve_authored_geometry = true;
    state.mark_saved_revision();
    assert!(!state.is_dirty(), "page switching is editor metadata only");

    let mut session = SaveSession::new();
    assert_eq!(
        session.enqueue_clean_bound_op_save_as(
            &state,
            9,
            source.clone(),
            target.clone(),
            true,
            None,
        ),
        EnqueueOutcome::Started
    );
    assert!(matches!(
        &session
            .running
            .as_ref()
            .expect("running clean copy")
            .snapshot
            .payload,
        SavePayload::CleanBoundOp { source_path, .. } if source_path == &source
    ));

    let completion = session.wait_next().expect("clean save completion");
    assert!(completion.result.is_ok(), "{:?}", completion.result);
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&target).expect("read clean-copy target"))
            .expect("target JSON");
    assert_eq!(saved["version"], "0.8.0", "old schema stays untouched");
    assert_eq!(saved["futureExtension"]["keep"], true);
    assert_eq!(saved["pages"][1]["children"][0]["id"], "kept");
    assert_eq!(saved["editorMeta"]["activePageIndex"], 1);
    assert_eq!(saved["editorMeta"]["preserveAuthoredGeometry"], true);

    let _ = std::fs::remove_file(source);
    let _ = std::fs::remove_file(target);
}

#[test]
fn clean_copy_eligibility_falls_back_for_dirty_missing_or_non_op_sources() {
    let source = temp_op_path("eligibility");
    let target = temp_op_path("eligibility-target");
    std::fs::write(&source, b"{}").expect("write source");
    let mut state = EditorState::new();
    state.mark_saved_revision();
    assert!(can_copy_clean_bound_op(&state, &source, &target));
    assert!(!can_copy_clean_bound_op(&state, &source, &source));

    state.mark_document_changed();
    assert!(!can_copy_clean_bound_op(&state, &source, &target));
    state.mark_saved_revision();
    std::fs::remove_file(&source).expect("remove source");
    assert!(!can_copy_clean_bound_op(&state, &source, &target));

    let pen_source = source.with_extension("pen");
    std::fs::write(&pen_source, b"{}").expect("write pen source");
    assert!(!can_copy_clean_bound_op(&state, &pen_source, &target));

    let _ = std::fs::remove_file(pen_source);
}
