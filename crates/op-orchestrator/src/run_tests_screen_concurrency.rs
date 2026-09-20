//! Inter-group concurrency: parallel groups, per-group failure isolation and
//! the concurrency-1 sequential fallback.

use super::*;

/// `effective_concurrency(3, 3) == 3`: all 3 screen groups get their own
/// worker permit, so the executor drives them together via `join_all`
/// instead of the sequential loop. Content-correctness only (the workers'
/// genuine wall-clock overlap is proven separately with `CountingLlm` — a
/// `ScriptedLlm`-backed run never truly yields, so it cannot itself
/// distinguish "ran concurrently" from "ran group-by-group").
#[test]
fn three_screen_groups_run_concurrently_and_land_correct_content() {
    let llm = ScriptedLlm::new(vec![
        ScriptResponse::Text(THREE_SCREEN_PLAN_JSON.into()),
        ScriptResponse::Text(node_json("s")),
        ScriptResponse::Text(node_json("l")),
        ScriptResponse::Text(node_json("p")),
    ]);
    let mut sink = VecDocSink::new();
    let mut progress = Vec::new();

    let summary = futures::executor::block_on(Orchestrator::new().run(
        req_with_concurrency(3),
        &mut sink,
        &llm,
        &mut |p| progress.push(p),
        &AbortFlag::new(),
        &stub_providers(),
    ))
    .expect("3-group concurrent run ok");

    let roots = sink.state.active_children();
    assert_eq!(roots.len(), 3, "one root per screen group, got {roots:?}");
    for name in ["Search", "Library", "Premium"] {
        let root = find_root_by_name(roots, name);
        assert!(
            root.children().is_some_and(|c| !c.is_empty()),
            "{name} root must have received its subtask's content"
        );
    }
    assert_eq!(summary.subtasks.len(), 3);
    assert_eq!(summary.total_nodes, 3);

    // Progress completeness: every subtask reaches a terminal state
    // (Started paired with Done/Failed), regardless of interleaving order.
    for id in ["search-body", "library-body", "premium-body"] {
        assert!(
            progress.iter().any(
                |p| matches!(progress_event(p), Progress::SubtaskStarted { id: i, .. } if i == id)
            ),
            "{id} must report SubtaskStarted"
        );
        assert!(
            progress.iter().any(|p| matches!(progress_event(p),
                Progress::SubtaskDone { id: i, .. } | Progress::SubtaskFailed { id: i, .. }
                    if i == id)),
            "{id} must report a terminal Done/Failed"
        );
    }
}

/// One screen group's subtask hits a terminal policy failure while the other
/// two groups succeed — the failure must stay ISOLATED to its own root; it
/// must never abort or empty out its siblings' content, and its own (now-empty)
/// scaffold root must still survive (only an ALL-roots-empty run deletes
/// scaffolding).
#[test]
fn one_group_failure_does_not_take_down_the_others() {
    let llm = ScriptedLlm::new(vec![
        ScriptResponse::Text(THREE_SCREEN_PLAN_JSON.into()),
        ScriptResponse::Text(node_json("s")),
        // "content blocked" is non-retryable (crate::retry::is_non_retryable)
        // — the ladder stops after attempt 1, consuming exactly one slot.
        ScriptResponse::Fail(LlmError {
            message: "content blocked by policy".into(),
            aborted: false,
        }),
        ScriptResponse::Text(node_json("p")),
    ]);
    let mut sink = VecDocSink::new();
    let mut progress = Vec::new();

    let summary = futures::executor::block_on(Orchestrator::new().run(
        req_with_concurrency(3),
        &mut sink,
        &llm,
        &mut |event| progress.push(event),
        &AbortFlag::new(),
        &stub_providers(),
    ))
    .expect("partial-failure run must still succeed overall");

    let roots = sink.state.active_children();
    assert_eq!(
        roots.len(),
        3,
        "a failed group's root is never deleted while siblings have content"
    );

    let search = find_root_by_name(roots, "Search");
    let library = find_root_by_name(roots, "Library");
    let premium = find_root_by_name(roots, "Premium");
    assert!(
        contains_text(search, "s"),
        "Search must be unaffected by Library's failure"
    );
    assert!(
        contains_text(premium, "p"),
        "Premium must be unaffected by Library's failure"
    );
    // Library's root still SURVIVES (not deleted — only an all-roots-empty
    // run deletes scaffolding) but never received subtask content — a
    // mobile scaffold root may carry pre-inserted chrome (e.g. a status
    // bar), so "failed" is checked by the ABSENCE of the subtask's own
    // marker text, not by raw emptiness.
    assert!(
        !contains_text(library, "l"),
        "Library's failed subtask must never backfill with the wrong content"
    );
    // Promise-delivery invariant: a scaffolded screen that never received
    // subtask content must not ship silently — `run` marks it on the canvas
    // itself (`unfilled_screens::mark_unfilled_screens`) and reports it in
    // `RunSummary`.
    assert_eq!(
        library.base().name.as_deref(),
        Some("Library (unfilled)"),
        "a screen whose subtask never delivered content must be marked unfilled on the canvas"
    );
    assert_eq!(
        summary.unfilled_screens,
        vec!["Library".to_string()],
        "the classic-path summary must also report the same unfilled screen"
    );

    assert_eq!(summary.subtasks.len(), 3);
    assert!(
        summary.subtasks.iter().any(|o| o.node_count == 0),
        "exactly one subtask's final outcome stays zero-node"
    );
    assert_eq!(
        summary.subtasks.iter().filter(|o| o.node_count > 0).count(),
        2,
        "the other two subtasks succeed"
    );
    assert!(
        !progress.iter().any(|event| {
            matches!(
                progress_event(event),
                Progress::SubtaskRetry {
                    id,
                    attempt: 4,
                    ..
                } if id == "library-body"
            )
        }),
        "a non-retryable policy failure must not enter the salvage pass"
    );
    assert!(
        progress.iter().any(|event| {
            matches!(
                event,
                Progress::WorkerScoped(worker)
                    if worker.group_idx == 1
                        && matches!(worker.event.as_ref(), Progress::SubtaskFailed { id, .. }
                            if id == "library-body")
            )
        }),
        "the terminal policy failure must remain on Library's worker transcript"
    );
}

/// Proves the workers genuinely overlap in wall-clock time (not just
/// "structurally concurrent code that happens to run group-by-group"):
/// `CountingLlm`'s stream yields `Pending` once before resolving, which lets
/// `join_all` poll a SIBLING worker while the first is still "in flight" —
/// `max_concurrent()` only exceeds 1 if the executor actually interleaves
/// them.
#[test]
fn concurrency_genuinely_overlaps_across_groups() {
    let llm = CountingLlm::new(vec![
        THREE_SCREEN_PLAN_JSON.into(),
        node_json("s"),
        node_json("l"),
        node_json("p"),
    ]);
    let mut sink = VecDocSink::new();

    futures::executor::block_on(Orchestrator::new().run(
        req_with_concurrency(3),
        &mut sink,
        &llm,
        &mut |_| {},
        &AbortFlag::new(),
        &stub_providers(),
    ))
    .expect("3-group concurrent run ok");

    assert!(
        llm.max_concurrent() > 1,
        "3 screen groups with concurrency=3 must overlap — max_concurrent() was {}",
        llm.max_concurrent()
    );
}

/// `concurrency=1` regression lock: even with 3 screen groups on the
/// canvas, a `request.concurrency` of 1 must take the UNTOUCHED sequential
/// loop — `effective_concurrency(1, 3) == 1` — so calls never overlap.
#[test]
fn concurrency_one_keeps_the_sequential_path_even_with_multiple_groups() {
    let llm = CountingLlm::new(vec![
        THREE_SCREEN_PLAN_JSON.into(),
        node_json("s"),
        node_json("l"),
        node_json("p"),
    ]);
    let mut sink = VecDocSink::new();

    futures::executor::block_on(Orchestrator::new().run(
        req_with_concurrency(1),
        &mut sink,
        &llm,
        &mut |_| {},
        &AbortFlag::new(),
        &stub_providers(),
    ))
    .expect("sequential 3-group run ok");

    assert_eq!(
        llm.max_concurrent(),
        1,
        "concurrency=1 must never let two subtask calls overlap"
    );
    assert_eq!(sink.state.active_children().len(), 3);
}
