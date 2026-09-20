//! Tests for the canvas agent indicators driven by the design-agent
//! tool-loop and the single-shot design orchestrator, split from
//! `design_loop_indicator.rs` to keep both files under the repo's
//! 800-line cap. Wired in as `design_loop_indicator::tests` via
//! `#[path]` so `use super::*` still resolves against the parent.

use super::*;
use crate::design_session::DesignSession;
use jian_ops_schema::node::base::PenNodeBase;
use jian_ops_schema::node::{ContainerProps, FrameNode};
use op_editor_core::EditorState;
use std::sync::mpsc;

fn make_state() -> EditorState {
    EditorState::new()
}

fn frame_node(id: &str) -> PenNode {
    PenNode::Frame(FrameNode {
        base: PenNodeBase {
            id: id.to_string(),
            name: Some("Frame".into()),
            ..Default::default()
        },
        container: ContainerProps::default(),
        children: Some(Vec::new()),
        image_search_query: None,
        reusable: None,
        screen: None,
        slot: None,
        state: None,
        bindings: None,
        events: None,
        lifecycle: None,
        semantics: None,
        gestures: None,
        route: None,
        breakpoint: None,
    })
}

/// Builds a live `DesignSession` (real mpsc channels, dropped receivers
/// so nothing blocks) bound to a fresh `agent_indicators` epoch — the
/// same shape `chat_session_launch.rs::launch_cli_standard_turn` and
/// `op_host_services::design_session::start` hand to `current_design`.
fn design_session_with_epoch() -> (DesignSession, u64) {
    let (_delta_tx, delta_rx) = mpsc::channel();
    let (_cmd_tx, cmd_rx) = mpsc::channel();
    let epoch = agent_indicators::begin();
    (
        DesignSession::from_channels_with_epoch(delta_rx, cmd_rx, epoch),
        epoch,
    )
}

fn mark_design_started(state: &mut EditorState) {
    let mut message = op_editor_core::ChatMessage::assistant_streaming();
    message.activities.push(op_editor_core::ChatActivity {
        id: "__planning".into(),
        title: "Planning the design".into(),
        detail: None,
        status: op_editor_core::ChatActivityStatus::Running,
        content_offset: None,
    });
    state.chat.messages.push(message);
}

#[test]
fn collect_top_level_frame_ids_does_not_panic_on_fresh_doc() {
    let state = make_state();
    // A fresh blank document may or may not have frames — just verify
    // the function runs without panicking.
    let _ids = collect_top_level_frame_ids(&state);
}

#[test]
fn pump_indicator_noop_when_agents_running_zero() {
    let mut state = make_state();
    let mut indicator: Option<DesignLoopIndicator> = None;
    // agents_running = (0,0), no session → stays idle.
    pump_indicator(&mut indicator, &None, &mut state);
    assert!(indicator.is_none());
    assert_eq!(state.chat.agents_running, (0, 0));
}

#[test]
fn pump_indicator_confirms_cursor_when_the_agent_name_is_published() {
    let _guard = lock_agent_indicators();
    agent_indicators::clear();
    let mut state = make_state();
    state
        .chat
        .messages
        .push(op_editor_core::ChatMessage::assistant_streaming());
    state.chat.agents_running = (1, 1);
    let epoch = agent_indicators::begin();
    let (_tx, rx) = mpsc::channel::<op_ai::chat_provider::ChatDelta>();
    let current = Some(op_editor_host_core::chat::ChatSession::from_channels(
        rx, None,
    ));
    let mut indicator = None;

    pump_indicator(&mut indicator, &current, &mut state);

    let indicator = indicator.expect("the design-loop identity is published");
    assert_eq!(indicator.epoch, epoch);
    assert_eq!(
        agent_indicators::snapshot().cursor_agent,
        Some(agent_indicators::AgentTag {
            color: indicator.color,
            name: indicator.name,
        })
    );
    agent_indicators::end_if_epoch(epoch);
}

#[test]
fn pump_indicator_teardown_clears_indicator_and_agents_running() {
    // Touches the process-global `agent_indicators` registry — guard
    // against the new `design_session_indicator_*` tests below racing
    // this one under the default parallel test runner.
    let _guard = lock_agent_indicators();
    let mut state = make_state();
    // Manually plant an indicator as if a turn had been launched.
    let epoch = op_editor_core::agent_indicators::begin();
    let mut indicator: Option<DesignLoopIndicator> = Some(DesignLoopIndicator {
        epoch,
        color: "#FF6B6B".to_string(),
        name: "Kiki".to_string(),
        initial_frame_ids: HashSet::new(),
    });
    state.chat.agents_running = (1, 1);
    // Session gone (None) → teardown path.
    pump_indicator(&mut indicator, &None, &mut state);
    assert!(indicator.is_none(), "teardown must clear the indicator");
    assert_eq!(
        state.chat.agents_running,
        (0, 0),
        "teardown must clear agents_running"
    );
}

// ── pump_design_session_indicator: current_design-driven pump ──────────

/// Guards every test below against the process-global `agent_indicators`
/// registry racing a concurrently-running test in this same binary
/// (`main.rs::agent_indicator_test_lock`, the established pattern also
/// used by `sub_agent_session_tests.rs` / `chat_intent_host_tests.rs`).
fn lock_agent_indicators() -> std::sync::MutexGuard<'static, ()> {
    crate::agent_indicator_test_lock::LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

#[test]
fn design_session_indicator_noop_when_current_design_none() {
    let _guard = lock_agent_indicators();
    agent_indicators::clear();
    let mut state = make_state();
    let mut indicator: Option<DesignLoopIndicator> = None;
    pump_design_session_indicator(&mut indicator, &None, &mut state, None);
    assert!(indicator.is_none());
}

#[test]
fn design_session_indicator_stays_dormant_without_an_active_epoch() {
    // Contract: this driver only READS the epoch the launch site began;
    // it never calls `begin()` itself. With no epoch active, a live
    // `current_design` must not spin one up out of thin air.
    let _guard = lock_agent_indicators();
    agent_indicators::clear();
    let mut state = make_state();
    mark_design_started(&mut state);
    let (session, _epoch) = design_session_with_epoch();
    // Retire the epoch immediately so none is active by the time the
    // pump runs, mirroring "epoch already ended elsewhere".
    agent_indicators::clear();
    let current = Some(session);
    let mut indicator: Option<DesignLoopIndicator> = None;
    pump_design_session_indicator(&mut indicator, &current, &mut state, None);
    assert!(
        indicator.is_none(),
        "no active epoch → driver must stay dormant, not fabricate one"
    );
}

#[test]
fn design_session_indicator_waits_for_design_progress_after_classification() {
    let _guard = lock_agent_indicators();
    agent_indicators::clear();
    let mut state = make_state();
    let mut message = op_editor_core::ChatMessage::assistant_streaming();
    message.agent_name = Some("Claude Code".into());
    state.chat.messages.push(message);
    let (session, _epoch) = design_session_with_epoch();
    let current = Some(session);
    let mut indicator: Option<DesignLoopIndicator> = None;

    pump_design_session_indicator(&mut indicator, &current, &mut state, None);

    assert!(indicator.is_none());
    assert_eq!(
        state.chat.messages.last().unwrap().agent_name.as_deref(),
        Some("Claude Code"),
        "a parked design session must not relabel a turn before intent is known"
    );
}

#[test]
fn design_session_indicator_registers_frames_that_appear_after_the_turn_starts() {
    let _guard = lock_agent_indicators();
    agent_indicators::clear();
    let mut state = make_state();
    mark_design_started(&mut state);
    let (session, epoch) = design_session_with_epoch();
    let current = Some(session);
    let mut indicator: Option<DesignLoopIndicator> = None;

    // First pump: no frames on the canvas yet — snapshots the (empty)
    // initial set and assigns the agent identity.
    pump_design_session_indicator(&mut indicator, &current, &mut state, None);
    assert!(indicator.is_some(), "live current_design must create one");
    assert_eq!(indicator.as_ref().unwrap().epoch, epoch);
    assert!(!agent_indicators::is_frame_generating("frame-1"));

    // The orchestrator inserts a top-level frame mid-turn.
    state.active_children_mut().push(frame_node("frame-1"));
    pump_design_session_indicator(&mut indicator, &current, &mut state, None);

    assert!(
        agent_indicators::is_frame_generating("frame-1"),
        "a frame added during the turn must be tagged as generating"
    );
}

#[test]
fn design_session_indicator_stamps_one_persona_on_transcript_and_canvas() {
    let _guard = lock_agent_indicators();
    agent_indicators::clear();
    let mut state = make_state();
    let mut message = op_editor_core::ChatMessage::assistant_streaming();
    message.agent_name = Some("Claude Code".into());
    message.activities.push(op_editor_core::ChatActivity {
        id: "content".into(),
        title: "Build content".into(),
        detail: None,
        status: op_editor_core::ChatActivityStatus::Running,
        content_offset: None,
    });
    state.chat.messages.push(message);
    let (session, _epoch) = design_session_with_epoch();
    let current = Some(session);
    let mut indicator: Option<DesignLoopIndicator> = None;

    pump_design_session_indicator(&mut indicator, &current, &mut state, None);

    let indicator = indicator
        .as_ref()
        .expect("design activity starts indicator");
    let transcript = state.chat.messages.last().expect("streaming message");
    assert_ne!(indicator.name, "Claude Code");
    assert_eq!(
        transcript.agent_name.as_deref(),
        Some(indicator.name.as_str())
    );
    assert_eq!(
        transcript.agent_color.as_deref(),
        Some(indicator.color.as_str())
    );
    assert_eq!(
        agent_indicators::snapshot().cursor_agent,
        Some(agent_indicators::AgentTag {
            color: indicator.color.clone(),
            name: indicator.name.clone(),
        }),
        "the canvas cursor identity is confirmed in the same pump as the transcript name"
    );
}

/// Dual-cursor-identity fix (2026-07-17): when the orchestrator's D-lite
/// concurrent screen-group path has ALREADY confirmed a `cursor_agent`
/// (its primary group's identity) before this pump ever runs, the
/// transcript must ADOPT that identity rather than minting an
/// independent random one — otherwise the chat bubble shows a THIRD
/// persona unrelated to any of the visible canvas cursors.
#[test]
fn design_session_indicator_adopts_an_already_confirmed_identity_instead_of_minting_one() {
    let _guard = lock_agent_indicators();
    agent_indicators::clear();
    let mut state = make_state();
    let mut message = op_editor_core::ChatMessage::assistant_streaming();
    message.activities.push(op_editor_core::ChatActivity {
        id: "content".into(),
        title: "Build content".into(),
        detail: None,
        status: op_editor_core::ChatActivityStatus::Running,
        content_offset: None,
    });
    state.chat.messages.push(message);
    let (session, epoch) = design_session_with_epoch();
    // The orchestrator's concurrent phase already confirmed its primary
    // group's identity before this pump runs.
    agent_indicators::confirm_cursor_agent(epoch, "#6C5CE7", "Pixel");
    let current = Some(session);
    let mut indicator: Option<DesignLoopIndicator> = None;

    pump_design_session_indicator(&mut indicator, &current, &mut state, None);

    let transcript = state.chat.messages.last().expect("streaming message");
    assert_eq!(
        transcript.agent_name.as_deref(),
        Some("Pixel"),
        "the transcript must adopt the already-confirmed identity, not mint a new one"
    );
    assert_eq!(transcript.agent_color.as_deref(), Some("#6C5CE7"));
    // Adopting must not disturb the already-confirmed cursor_agent.
    assert_eq!(
        agent_indicators::snapshot().cursor_agent,
        Some(agent_indicators::AgentTag {
            color: "#6C5CE7".into(),
            name: "Pixel".into(),
        })
    );
}

#[test]
fn design_session_identity_ignores_appended_worker_bubbles() {
    let _guard = lock_agent_indicators();
    agent_indicators::clear();
    let mut state = make_state();
    let mut primary = op_editor_core::ChatMessage::assistant_streaming();
    primary.activities.push(op_editor_core::ChatActivity {
        id: "__planning".into(),
        title: "Planning the design".into(),
        detail: None,
        status: op_editor_core::ChatActivityStatus::Running,
        content_offset: None,
    });
    state.chat.messages.push(primary);
    let mut worker = op_editor_core::ChatMessage::assistant_streaming();
    worker.design_worker_group = Some(1);
    worker.design_worker_screen = Some("Profile".into());
    worker.agent_name = Some("Mochi".into());
    worker.agent_color = Some("#4ECDC4".into());
    worker.activities.push(op_editor_core::ChatActivity {
        id: "profile-body".into(),
        title: "Profile body".into(),
        detail: None,
        status: op_editor_core::ChatActivityStatus::Running,
        content_offset: None,
    });
    state.chat.messages.push(worker);
    let (_session, epoch) = design_session_with_epoch();
    agent_indicators::confirm_cursor_agent(epoch, "#FF6B6B", "Fern");

    let identity = ensure_design_session_transcript_identity(&mut state, None)
        .expect("primary design identity");

    assert_eq!(identity, ("Fern".into(), "#FF6B6B".into()));
    assert_eq!(state.chat.messages[0].agent_name.as_deref(), Some("Fern"));
    assert_eq!(state.chat.messages[1].agent_name.as_deref(), Some("Mochi"));
    assert_eq!(
        agent_indicators::snapshot().cursor_agent,
        Some(agent_indicators::AgentTag {
            color: "#FF6B6B".into(),
            name: "Fern".into(),
        }),
        "the worker bubble must not replace the canonical cursor persona"
    );
    agent_indicators::end_if_epoch(epoch);
}

#[test]
fn design_session_indicator_teardown_clears_local_handle_only() {
    let _guard = lock_agent_indicators();
    agent_indicators::clear();
    let mut state = make_state();
    mark_design_started(&mut state);
    let (session, _epoch) = design_session_with_epoch();
    let mut current = Some(session);
    let mut indicator: Option<DesignLoopIndicator> = None;

    pump_design_session_indicator(&mut indicator, &current, &mut state, None);
    state.active_children_mut().push(frame_node("frame-1"));
    pump_design_session_indicator(&mut indicator, &current, &mut state, None);
    assert!(agent_indicators::is_frame_generating("frame-1"));

    // Turn finished: `design_session::pump_progress` already dropped the
    // session (which itself retired the epoch via `Drop`) before this
    // driver observes `current_design == None`.
    current = None;
    pump_design_session_indicator(&mut indicator, &current, &mut state, None);

    assert!(indicator.is_none(), "teardown must clear the local handle");
    assert!(
        !agent_indicators::is_frame_generating("frame-1"),
        "DesignSession::drop already retired the epoch — frames stop registering as generating"
    );
    // The epoch is retired, not reused — a fresh turn always gets a new one.
    assert_eq!(agent_indicators::active_epoch(), None);
}

#[test]
fn chat_loop_and_design_session_drivers_do_not_interfere() {
    // `launch_cli_standard_turn` parks a ChatSession AND a DesignSession
    // together while classification resolves, but never sets
    // `agents_running` — so the chat-loop driver's lazy-creation gate
    // must stay closed even though `current_chat` is conceptually "live"
    // (represented here just by the agents_running invariant it reads).
    let _guard = lock_agent_indicators();
    agent_indicators::clear();
    let mut state = make_state();
    mark_design_started(&mut state);
    let (session, epoch) = design_session_with_epoch();
    let current_design = Some(session);
    let mut chat_indicator: Option<DesignLoopIndicator> = None;
    let mut design_indicator: Option<DesignLoopIndicator> = None;

    // Chat-loop driver: agents_running stays (0, 0) for this route.
    pump_indicator(&mut chat_indicator, &None, &mut state);
    assert!(chat_indicator.is_none());

    // Design-session driver: drives off the same epoch independently.
    pump_design_session_indicator(&mut design_indicator, &current_design, &mut state, None);
    assert!(design_indicator.is_some());
    assert_eq!(design_indicator.as_ref().unwrap().epoch, epoch);
}

/// D-lite three-piece visibility fix (2026-07-17): a screen-group root
/// the orchestrator ALREADY tagged with its OWN per-group identity must
/// keep that tag across every later pump — `register_new_frames` must
/// never clobber it back down to this driver's single identity, or N
/// concurrent agents' distinct badges/cursors collapse to one.
#[test]
fn register_new_frames_does_not_clobber_an_already_tagged_frame() {
    let _guard = lock_agent_indicators();
    agent_indicators::clear();
    let epoch = agent_indicators::begin();
    // Simulate the orchestrator having already tagged a screen-group
    // root with a DIFFERENT identity than this driver's own, before
    // this pump ever sees the frame.
    agent_indicators::add_frame(epoch, "frame-1", "#5B8DEF", "Pixel");
    let indicator = DesignLoopIndicator {
        epoch,
        color: "#FF6B6B".to_string(),
        name: "Kiki".to_string(),
        initial_frame_ids: HashSet::new(),
    };
    let mut state = make_state();
    state.active_children_mut().push(frame_node("frame-1"));

    register_new_frames(&indicator, &state);

    let snap = agent_indicators::snapshot();
    assert_eq!(
        snap.frames.get("frame-1"),
        Some(&agent_indicators::AgentTag {
            color: "#5B8DEF".to_string(),
            name: "Pixel".to_string(),
        }),
        "an already-tagged frame must keep its own identity, not the driver's"
    );
    agent_indicators::end_if_epoch(epoch);
}

/// The ordinary single-agent case is unaffected: a frame this driver
/// sees for the FIRST time (nothing tagged it yet) still gets tagged
/// with the driver's own identity, exactly as `add_frame` always did.
#[test]
fn register_new_frames_still_tags_a_fresh_untagged_frame() {
    let _guard = lock_agent_indicators();
    agent_indicators::clear();
    let epoch = agent_indicators::begin();
    let indicator = DesignLoopIndicator {
        epoch,
        color: "#FF6B6B".to_string(),
        name: "Kiki".to_string(),
        initial_frame_ids: HashSet::new(),
    };
    let mut state = make_state();
    state.active_children_mut().push(frame_node("frame-1"));

    register_new_frames(&indicator, &state);

    let snap = agent_indicators::snapshot();
    assert_eq!(
        snap.frames.get("frame-1"),
        Some(&agent_indicators::AgentTag {
            color: "#FF6B6B".to_string(),
            name: "Kiki".to_string(),
        })
    );
    agent_indicators::end_if_epoch(epoch);
}
