//! Core `run` happy-path / scaffold / append / fallback tests.

use super::*;

// ── existing tests (must stay green) ─────────────────────────────────────

#[test]
fn run_happy_path_applies_scaffold_and_subtasks() {
    let llm = ScriptedLlm::new(vec![
        ScriptResponse::Text(PLAN_JSON.into()),
        ScriptResponse::Text(node_json("hero")),
        ScriptResponse::Text(node_json("feat")),
    ]);
    let mut sink = VecDocSink::new();
    let mut events: Vec<Progress> = Vec::new();
    let mut on_progress = |p: Progress| events.push(p);

    let summary = futures::executor::block_on(Orchestrator::new().run(
        req(),
        &mut sink,
        &llm,
        &mut on_progress,
        &AbortFlag::new(),
        &stub_providers(),
    ))
    .expect("run ok");

    // root_frame_id 是 InsertSubtree 重映射后的真实 id —— 不是
    // plan 里的 "root" 字面值,只断言它非空。
    assert!(!summary.root_frame_id.is_empty());
    assert_eq!(summary.subtasks.len(), 2);
    assert!(summary.total_nodes >= 2);
    // undo batch 配对。
    assert_eq!(sink.batch_depth, 0);
    // 至少有 scaffold + 两个 subtask 的 InsertSubtree。
    let inserts = sink
        .applied
        .iter()
        .filter(|c| matches!(c, EditorCommand::InsertSubtree { .. }))
        .count();
    assert!(inserts >= 3, "expected >=3 InsertSubtree, got {inserts}");
    assert!(matches!(events.first(), Some(Progress::Planning)));
    // CleanupDone must be present (validation runs after it).
    assert!(
        events.iter().any(|e| matches!(e, Progress::CleanupDone)),
        "expected CleanupDone in events"
    );
}

#[test]
fn fresh_run_preserves_explicit_root_height_through_cleanup() {
    let tall_section = |name: &str| {
        format!(
            r#"I(null, {{"type":"frame","name":"{name}","width":"fill_container","height":560,"children":[{{"type":"text","content":"{name}","fontSize":18}}]}});"#
        )
    };
    let llm = ScriptedLlm::new(vec![
        ScriptResponse::Text(PLAN_JSON.into()),
        ScriptResponse::Text(tall_section("Tall Hero")),
        ScriptResponse::Text(tall_section("Tall Features")),
    ]);
    let mut sink = VecDocSink::new();
    let mut request = req();
    request.prompt = "Design a 1440×900 desktop page".into();
    request.validation_enabled = false;

    futures::executor::block_on(Orchestrator::new().run(
        request,
        &mut sink,
        &llm,
        &mut |_| {},
        &AbortFlag::new(),
        &stub_providers(),
    ))
    .expect("explicit-size run succeeds");

    let root = sink
        .state
        .active_children()
        .iter()
        .find(|node| node.base().name.as_deref() == Some("Page"))
        .expect("generated page root");
    assert_eq!(root.width_px(), Some(1440.0));
    assert_eq!(
        root.height_px(),
        Some(900.0),
        "fresh-run cleanup must not grow an explicitly requested root height"
    );
}

#[test]
fn run_mobile_scaffold_reveals_status_bar() {
    let _guard = crate::agent_indicator_test_support::lock();
    let epoch = op_editor_core::agent_indicators::begin();
    let llm = ScriptedLlm::new(vec![
        ScriptResponse::Text(MOBILE_PLAN_JSON.into()),
        ScriptResponse::Text(node_json("hero")),
    ]);
    let mut sink = VecDocSink::new();
    let mut events: Vec<Progress> = Vec::new();
    let mut on_progress = |p: Progress| events.push(p);
    let mut request = req();
    request.prompt = "a mobile food app".into();
    request.validation_enabled = false;

    let summary = futures::executor::block_on(Orchestrator::new().with_indicator_epoch(epoch).run(
        request,
        &mut sink,
        &llm,
        &mut on_progress,
        &AbortFlag::new(),
        &stub_providers(),
    ))
    .expect("mobile run ok");

    assert!(!summary.root_frame_id.is_empty());
    let root = sink.state.active_children().first().expect("root inserted");
    let status_id = root
        .children()
        .expect("mobile root should have children")
        .iter()
        .find(|node| {
            serde_json::to_value(node)
                .ok()
                .is_some_and(|v| v["role"] == "status-bar")
        })
        .map(|node| node.id_str().to_string())
        .expect("mobile scaffold should insert a status bar");
    let snapshot = op_editor_core::agent_indicators::snapshot();
    // Dual-cursor-identity fix (2026-07-17): the sequential path no longer
    // tags its root with an orchestrator-minted identity — the host's
    // confirmed transcript identity (`cursor_agent`) is the single source of
    // truth for a single-agent run now (see `run.rs`'s `group_identities`
    // doc). Reveal scheduling (a SEPARATE mechanism from ownership tagging)
    // is unaffected either way.
    assert!(
        !snapshot.frames.contains_key(root.id_str()),
        "sequential scaffold root must NOT get its own agent frame badge \
         anymore — it should inherit the host-confirmed identity instead"
    );
    assert!(
        snapshot.reveals.contains_key(&status_id),
        "status bar should get a reveal animation, got {:?}",
        snapshot.reveals.keys().collect::<Vec<_>>()
    );
    op_editor_core::agent_indicators::end_if_epoch(epoch);
}

#[test]
fn run_follow_on_screen_scaffold_lands_beside_existing_root() {
    let llm = ScriptedLlm::new(vec![
        ScriptResponse::Text(MOBILE_PLAN_JSON.into()),
        ScriptResponse::Text(node_json("discover")),
    ]);
    let mut sink = VecDocSink::new();
    sink.state.active_children_mut().clear();
    sink.state
        .active_children_mut()
        .push(existing_root_json("home", "Home", 80.0, 40.0, 390.0));
    let mut events: Vec<Progress> = Vec::new();
    let mut on_progress = |p: Progress| events.push(p);
    let mut request = req();
    request.prompt = "继续画出发现页".into();
    request.validation_enabled = false;

    let summary = futures::executor::block_on(Orchestrator::new().run(
        request,
        &mut sink,
        &llm,
        &mut on_progress,
        &AbortFlag::new(),
        &stub_providers(),
    ))
    .expect("follow-on run ok");

    let existing_right = 80.0 + 390.0;
    let new_root = sink
        .state
        .active_children()
        .iter()
        .find(|node| node.id_str() == summary.root_frame_id)
        .expect("new root inserted");
    assert!(
        new_root.base().x.unwrap_or(0.0) >= existing_right + 80.0,
        "new root should be laid out beside the existing screen"
    );
    assert_eq!(new_root.base().y, Some(40.0));
}

#[test]
fn run_zero_node_subtask_preserves_failure_context() {
    // 规划 OK,但第一个 subtask 吐垃圾(3 次全失败)→ 零节点 → AllFailed
    // with the parser failure context, not generic NoContent.
    // C3 引入 3-attempt 梯子:需要 3 条垃圾响应才能穷尽重试。
    let llm = ScriptedLlm::new(vec![
        ScriptResponse::Text(PLAN_JSON.into()),
        ScriptResponse::Text("the model refused".into()),
        ScriptResponse::Text("still refused".into()),
        ScriptResponse::Text("refused again".into()),
    ]);
    let mut sink = VecDocSink::new();
    let mut on_progress = |_p: Progress| {};
    let result = futures::executor::block_on(Orchestrator::new().run(
        req(),
        &mut sink,
        &llm,
        &mut on_progress,
        &AbortFlag::new(),
        &stub_providers(),
    ));
    match result {
        Err(OrchestratorError::AllFailed(message)) => assert!(!message.is_empty()),
        other => panic!("expected AllFailed with parser context, got {other:?}"),
    }
    // undo batch 仍配对。
    assert_eq!(sink.batch_depth, 0);
}

#[test]
fn append_all_failed_preserves_existing_target_and_rolls_back_run_state() {
    let llm = ScriptedLlm::new(vec![
        ScriptResponse::Text(MOBILE_PLAN_JSON.into()),
        // A non-retryable failure stops the normal ladder after attempt 1.
        ScriptResponse::Fail(crate::types::LlmError {
            message: "content blocked by policy".into(),
            aborted: false,
        }),
        // The end-of-run salvage pass still makes one final attempt.
        ScriptResponse::Fail(crate::types::LlmError {
            message: "content blocked by policy".into(),
            aborted: false,
        }),
    ]);
    let mut sink = VecDocSink::new();
    sink.state.active_children_mut().clear();
    let target: jian_ops_schema::node::PenNode = serde_json::from_value(serde_json::json!({
        "type": "frame",
        "id": "existing-target",
        "name": "Existing Screen",
        "x": 80.0,
        "y": 40.0,
        "width": 390.0,
        "height": 844.0,
        "layout": "vertical",
        "children": [{
            "type": "frame",
            "id": "sentinel-section",
            "name": "Do Not Delete",
            "width": 390.0,
            "height": 120.0,
            "children": [{
                "type": "text",
                "id": "sentinel-copy",
                "name": "Sentinel Copy",
                "content": "existing content must remain byte-for-byte unchanged",
                "fontSize": 18.0
            }]
        }]
    }))
    .expect("valid existing append target");
    sink.state.active_children_mut().push(target);

    let target_before = serde_json::to_value(&sink.state.active_children()[0])
        .expect("serialize target before append run");
    assert!(sink.state.doc.variables.is_none());
    assert!(sink.state.doc.themes.is_none());

    let mut request = req();
    request.prompt = "继续生成这个页面的下一部分".into();
    request.validation_enabled = false;
    request.append_context = Some(crate::types::AppendContext {
        target_parent_id: "existing-target".into(),
        target_width: 390.0,
        existing_section_labels: vec!["Do Not Delete".into()],
        is_mobile: true,
    });

    let result = futures::executor::block_on(Orchestrator::new().run(
        request,
        &mut sink,
        &llm,
        &mut |_| {},
        &AbortFlag::new(),
        &stub_providers(),
    ));

    assert!(
        matches!(result, Err(OrchestratorError::AllFailed(_))),
        "all failed append run must retain the concrete failure: {result:?}"
    );
    let target_after = sink
        .state
        .active_children()
        .iter()
        .find(|node| node.id_str() == "existing-target")
        .expect("the pre-existing append target must survive an all-failed run");
    assert_eq!(
        serde_json::to_value(target_after).expect("serialize target after append run"),
        target_before,
        "target and sentinel content must remain byte-for-byte unchanged"
    );
    assert!(
        !sink.applied.iter().any(|command| matches!(
            command,
            EditorCommand::DeleteNode { node_id, .. }
                if node_id.as_str() == "existing-target"
        )),
        "append failure must never issue DeleteNode for a user-owned target"
    );
    assert!(
        sink.applied
            .iter()
            .any(|command| matches!(command, EditorCommand::MergeThemePreset { .. })),
        "the run must actually seed variables before rollback is tested"
    );
    assert!(
        sink.applied
            .iter()
            .any(|command| matches!(command, EditorCommand::DeleteVariable { .. })),
        "failed run must issue variable rollback commands"
    );
    assert!(
        sink.state
            .doc
            .variables
            .as_ref()
            .map(|variables| variables.is_empty())
            .unwrap_or(true),
        "semantic variables seeded for the failed run must be rolled back"
    );
    assert!(
        sink.state
            .doc
            .themes
            .as_ref()
            .map(|themes| themes.is_empty())
            .unwrap_or(true),
        "theme axes seeded for the failed run must be rolled back"
    );
    assert_eq!(
        sink.batch_depth, 0,
        "undo batch must be balanced on failure"
    );
}

#[test]
fn run_planning_failure_uses_fallback_plan() {
    // 规划吐垃圾 → fallback plan;subtask 正常 → 成功。
    let llm = ScriptedLlm::new(vec![
        ScriptResponse::Text("no json here".into()),
        ScriptResponse::Text("no json here".into()),
        ScriptResponse::Text(node_json("section-1")),
    ]);
    let mut sink = VecDocSink::new();
    let mut on_progress = |_p: Progress| {};
    let summary = futures::executor::block_on(Orchestrator::new().run(
        req(),
        &mut sink,
        &llm,
        &mut on_progress,
        &AbortFlag::new(),
        &stub_providers(),
    ))
    .expect("fallback run ok");
    assert!(summary.total_nodes >= 1);
}
