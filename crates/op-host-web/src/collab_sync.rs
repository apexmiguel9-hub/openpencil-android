//! Drive the daemon's collaboration runtime from the web shell.
//!
//! The wasm bundle carries the collaboration *panel* but no transport: a relay
//! session needs device credentials only a native process can hold, so the
//! session lives in the daemon and this module is the wire between them. It is
//! the collaboration twin of [`crate::web_auth_sync`] and follows its shape —
//! one interval, `thread_local` latches, every write through the shared
//! `RepaintContext`.
//!
//! Three jobs per tick, in this order:
//!
//! 1. drain the panel's queued [`CollabUiAction`] to `POST /api/collab/action`;
//! 2. pull `GET /api/collab/state` when the projection moved, and install it;
//! 3. publish the local cursor to `POST /api/collab/presence`.
//!
//! ## Two counters, two meanings
//!
//! The daemon publishes `documentRevision` and `collabSeq` separately.
//! [`crate::live_sync_glue`]'s version poll already runs every 400 ms, so it
//! hands the observed `collabSeq` to [`note_version_probe`] instead of this
//! module opening a second probe. A `collabSeq` bump pulls *state*; it must
//! never pull the document, or every remote cursor move would refetch the whole
//! canvas.
//!
//! ## Availability
//!
//! Only ever what the daemon projected. Before the first successful `/state`
//! the panel keeps its default `Unavailable`, so a shell that cannot reach a
//! daemon offers nothing rather than offering a session it cannot start.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use op_editor_core::collab_wire::{
    CollabActionWire, CollabLocalPresenceWire, CollabPointWire, CollabStateWire,
};
use op_editor_core::{collab_routes, CollabConnectionPhase, CollabUiAction};

use crate::live_sync;
use crate::repaint_ctx::RepaintContext;

/// Tick cadence with no session — matched to the document version poll, since
/// with nothing running there is nothing to be responsive about.
const IDLE_TICK_MS: i32 = 400;
/// Tick cadence once a session exists. Presence and admission prompts are the
/// interactive parts of collaboration; 150 ms keeps them from feeling laggy
/// without approaching a per-frame request rate.
const SESSION_TICK_MS: i32 = 150;
/// Floor between presence posts. The daemon throttles again on its side to one
/// frame interval, so anything under this is spent bytes for no visible gain.
const PRESENCE_MIN_INTERVAL_MS: f64 = 100.0;

thread_local! {
    /// Latest `collabSeq` seen on the wire, from either probe.
    static OBSERVED_SEQ: Cell<u64> = const { Cell::new(0) };
    /// `collabSeq` of the projection currently installed in the panel.
    /// `None` until the first `/state` lands, which is what forces that first
    /// pull regardless of the counter.
    static APPLIED_SEQ: Cell<Option<u64>> = const { Cell::new(None) };
    /// One `/state` request in flight at a time.
    static STATE_BUSY: Cell<bool> = const { Cell::new(false) };
    /// An action already posted and not yet answered.
    static ACTION_BUSY: Cell<bool> = const { Cell::new(false) };
    /// A Cmd+Z is waiting to be posted as a `RequestUndo` action.
    static UNDO_REQUESTED: Cell<bool> = const { Cell::new(false) };
    /// An action the daemon refused with `collab-busy`, kept for the next tick.
    /// Losing it would silently drop something the user clicked.
    static ACTION_RETRY: RefCell<Option<CollabUiAction>> = const { RefCell::new(None) };
    /// `performance.now()` of the last presence post.
    static LAST_PRESENCE_MS: Cell<f64> = const { Cell::new(f64::NEG_INFINITY) };
    /// Last cursor actually sent, so an unmoved cursor costs nothing.
    static LAST_PRESENCE_POINT: Cell<Option<(f64, f64)>> = const { Cell::new(None) };
    /// Node ids present in the document the daemon last handed us.
    ///
    /// The browser mints ids from a local sequential counter, which is exactly
    /// what an active session cannot accept — see [`push_blocked_by_session`].
    pub(super) static DAEMON_NODE_IDS: RefCell<Option<HashSet<op_editor_core::NodeId>>> =
        const { RefCell::new(None) };

    /// The namespace this peer is currently minting ids under, when the
    /// owner-assigned allocator is enabled.
    ///
    /// Set by `sync_id_allocation` at the moment the allocator is installed
    /// and cleared the moment it is taken away, so it is exactly "the ids
    /// this peer is entitled to invent" — see `push_blocked_by_session`.
    pub(super) static SESSION_NAMESPACE: RefCell<Option<op_editor_core::PeerNamespace>> =
        const { RefCell::new(None) };
}

/// Feed the shared version probe's `collabSeq` in.
///
/// `live_sync_glue` already polls `GET /api/mcp/version` every 400 ms and the
/// daemon answers both counters there, so collaboration rides that request
/// rather than opening a second one.
pub(crate) fn note_version_probe(body: &str) {
    if let Some(seq) = op_editor_core::collab_wire::parse_collab_seq_probe(body) {
        OBSERVED_SEQ.set(seq);
    }
}

/// Record the node ids of a document just applied from the daemon.
pub(crate) fn note_daemon_document(state: &op_editor_core::EditorState) {
    DAEMON_NODE_IDS.with(|ids| *ids.borrow_mut() = Some(document_node_ids(state)));
}

/// Node ids in a document, through `op-editor-core`'s own walker so pages and
/// nodes share the one collision domain the allocator uses.
pub(super) fn document_node_ids(
    state: &op_editor_core::EditorState,
) -> HashSet<op_editor_core::NodeId> {
    op_editor_core::collect_document_ids(&state.doc)
}

/// Wire the collaboration relay onto the mounted shell. Called once from mount.
pub(crate) fn start<C: RepaintContext + 'static>(inner: &Rc<RefCell<C>>) {
    let base = crate::daemon_base::daemon_base();
    let inner = inner.clone();
    let tick: Rc<dyn Fn()> = Rc::new(move || {
        // Re-armed from the tick so a stream that dropped comes back without a
        // timer of its own; the backoff inside decides whether to actually try.
        ensure_event_stream(&base);
        drain_pending_action(&inner, &base);
        maybe_pull_state(&inner, &base);
        maybe_push_presence(&inner, &base);
    });
    schedule(tick, IDLE_TICK_MS);
}

/// Re-arming timeout rather than a fixed interval: the cadence has to follow
/// the session phase, and re-reading it at each arming is what makes a session
/// starting between two ticks speed the loop up immediately.
fn schedule(tick: Rc<dyn Fn()>, delay_ms: i32) {
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;

    let Some(window) = web_sys::window() else {
        return;
    };
    let closure = Closure::once_into_js(move || {
        tick();
        schedule(tick, current_cadence_ms());
    });
    let _ = window
        .set_timeout_with_callback_and_timeout_and_arguments_0(closure.unchecked_ref(), delay_ms);
}

/// Cadence for the next arming, from the phase the last projection installed.
fn current_cadence_ms() -> i32 {
    match APPLIED_SEQ.get() {
        // Before the first projection there is nothing to be responsive to.
        None => IDLE_TICK_MS,
        Some(_) if SESSION_LIVE.get() => SESSION_TICK_MS,
        Some(_) => IDLE_TICK_MS,
    }
}

thread_local! {
    /// Whether the installed projection is anything other than Idle. Read by
    /// the scheduler, which has no access to the host.
    static SESSION_LIVE: Cell<bool> = const { Cell::new(false) };
}

fn drain_pending_action<C: RepaintContext + 'static>(inner: &Rc<RefCell<C>>, base: &str) {
    if ACTION_BUSY.get() {
        return;
    }
    // Undo jumps the queue: it is a keystroke the user just pressed, while the
    // pending slot holds panel actions that can wait a tick.
    if UNDO_REQUESTED.replace(false) {
        ACTION_BUSY.set(true);
        let body = serde_json::to_string(&CollabActionWire::RequestUndo)
            .expect("a unit variant always serializes");
        let started = live_sync::post_json_with_status(
            &format!("{base}{}", collab_routes::ACTION),
            &body,
            Rc::new(move |_status, _response| ACTION_BUSY.set(false)),
        );
        if !started {
            ACTION_BUSY.set(false);
        }
        return;
    }
    let action = ACTION_RETRY
        .with(|slot| slot.borrow_mut().take())
        .or_else(|| {
            inner
                .try_borrow_mut()
                .ok()?
                .host_mut()
                .editor_state_mut()
                .editor_ui
                .collab
                .take_pending_action()
        });
    let Some(action) = action else {
        return;
    };
    // The panel's reapply control is the user-reachable way back to a document
    // an online auto-accept overwrote. Handled here rather than on the wire: a
    // server-authoritative deployment has no collaboration session, so the
    // daemon has nothing to replay this into — the copy lives in this tab.
    if matches!(action, op_editor_core::CollabUiAction::ReapplyDiscarded)
        && crate::live_sync_recovery::has_stash()
    {
        restore_stashed_document(inner);
        return;
    }
    let Some(wire) = wire_action(&action) else {
        // No wire form: nothing the daemon could do with it. Dropping is
        // correct — re-queueing would spin forever.
        return;
    };
    let Ok(body) = serde_json::to_string(&wire) else {
        return;
    };
    ACTION_BUSY.set(true);
    let retry_action = action.clone();
    let started = live_sync::post_json_with_status(
        &format!("{base}{}", collab_routes::ACTION),
        &body,
        Rc::new(move |status, response| {
            ACTION_BUSY.set(false);
            if status == 409 && response.contains("collab-busy") {
                // The daemon's single action slot was still full. Hold the
                // action for the next tick instead of dropping what the user
                // asked for.
                ACTION_RETRY.with(|slot| *slot.borrow_mut() = Some(retry_action.clone()));
            }
        }),
    );
    if !started {
        ACTION_BUSY.set(false);
        ACTION_RETRY.with(|slot| *slot.borrow_mut() = Some(action));
    }
}

/// Map the panel's action onto its wire form.
///
/// Exhaustive by construction so a new `CollabUiAction` variant fails to
/// compile here rather than silently becoming a no-op at runtime.
fn wire_action(action: &CollabUiAction) -> Option<CollabActionWire> {
    let key = |k: &op_editor_core::CollabAdmissionRequestKey| k.as_str().to_owned();
    Some(match action {
        CollabUiAction::OpenCreate => CollabActionWire::OpenCreate,
        CollabUiAction::Start => CollabActionWire::Start,
        CollabUiAction::StartLan => CollabActionWire::StartLan,
        CollabUiAction::SetRelayRegion { region } => CollabActionWire::SetRelayRegion {
            region: (*region).into(),
        },
        CollabUiAction::OpenJoin => CollabActionWire::OpenJoin,
        CollabUiAction::BeginDiscovery => CollabActionWire::BeginDiscovery,
        CollabUiAction::JoinDiscovered { discovery_id } => CollabActionWire::JoinDiscovered {
            discovery_id: discovery_id.clone(),
        },
        CollabUiAction::JoinAddress { endpoint } => CollabActionWire::JoinAddress {
            endpoint: endpoint.clone(),
        },
        CollabUiAction::Cancel => CollabActionWire::Cancel,
        CollabUiAction::Retry => CollabActionWire::Retry,
        CollabUiAction::Leave => CollabActionWire::Leave,
        CollabUiAction::DiscardPending => CollabActionWire::DiscardPending,
        CollabUiAction::ReapplyDiscarded => CollabActionWire::ReapplyDiscarded,
        CollabUiAction::SaveAsFork => CollabActionWire::SaveAsFork,
        CollabUiAction::ApproveAdmissionEditor { request_key } => {
            CollabActionWire::ApproveAdmissionEditor {
                request_key: key(request_key),
            }
        }
        CollabUiAction::ApproveAdmissionViewer { request_key } => {
            CollabActionWire::ApproveAdmissionViewer {
                request_key: key(request_key),
            }
        }
        CollabUiAction::RejectAdmission { request_key } => CollabActionWire::RejectAdmission {
            request_key: key(request_key),
        },
        CollabUiAction::ConfirmOwnerIdentity { request_key } => {
            CollabActionWire::ConfirmOwnerIdentity {
                request_key: key(request_key),
            }
        }
        CollabUiAction::RejectOwnerIdentity { request_key } => {
            CollabActionWire::RejectOwnerIdentity {
                request_key: key(request_key),
            }
        }
    })
}

fn maybe_pull_state<C: RepaintContext + 'static>(inner: &Rc<RefCell<C>>, base: &str) {
    if STATE_BUSY.get() {
        return;
    }
    let applied = APPLIED_SEQ.get();
    let observed = OBSERVED_SEQ.get();
    // Pull when the projection moved, when nothing has ever been pulled, or
    // continuously while a session runs — presence and admission prompts change
    // faster than the shared version probe reports.
    let due = applied != Some(observed) || applied.is_none() || SESSION_LIVE.get();
    if !due {
        return;
    }
    STATE_BUSY.set(true);
    let inner = inner.clone();
    let started = live_sync::get_with_status(
        &format!("{base}{}", collab_routes::STATE),
        Rc::new(move |status, body| {
            STATE_BUSY.set(false);
            if status != 200 {
                return;
            }
            let Ok(wire) = serde_json::from_str::<CollabStateWire>(&body) else {
                return;
            };
            apply_state(&inner, &wire);
        }),
    );
    if !started {
        STATE_BUSY.set(false);
    }
}

fn apply_state<C: RepaintContext + 'static>(inner: &Rc<RefCell<C>>, wire: &CollabStateWire) {
    let Ok(mut context) = inner.try_borrow_mut() else {
        return;
    };
    let now_ms = now_ms();
    let state = context.host_mut().editor_state_mut();
    // Everything lands through the projection's own `set_*` API, which
    // re-runs each sanitising constructor — the wire is data, not state.
    wire.apply_to(&mut state.editor_ui.collab, now_ms);
    APPLIED_SEQ.set(Some(wire.collab_seq));
    SESSION_LIVE.set(state.editor_ui.collab.phase != CollabConnectionPhase::Idle);
    sync_id_allocation(context.host_mut(), wire);
    context.host_mut().mark_editor_state_dirty();
    let _ = context.repaint();
}

/// Follow the projection into and out of namespaced id allocation.
///
/// Enabled only while a session is `Active` *and* the daemon published a
/// namespace; anything else restores the standalone counter. A session whose
/// namespace is absent — an older daemon — therefore keeps the local counter,
/// and `push_blocked_by_session` is what stops those ids from reaching a peer.
fn sync_id_allocation(host: &mut crate::widget_host::WidgetHost, wire: &CollabStateWire) {
    let namespace = (wire.phase == op_editor_core::collab_wire::CollabPhaseWire::Active)
        .then(|| wire.session.as_ref().and_then(|s| s.peer_namespace.clone()))
        .flatten();
    match namespace {
        // The installed allocator disagrees with the namespace the daemon now
        // publishes — a different session (or a different account) owns this
        // document. Tear the old allocator down so the arm below installs the
        // new one; leaving it would keep minting ids in a namespace this peer
        // no longer holds.
        Some(namespace)
            if host.collaboration_ids_enabled()
                && !session_namespace_matches(namespace.as_str()) =>
        {
            host.disable_collaboration_ids();
            set_session_namespace(None);
            if let Ok(parsed) = op_editor_core::PeerNamespace::parse(namespace.clone()) {
                let installed = parsed.clone();
                if host.enable_collaboration_ids(parsed).is_ok() {
                    set_session_namespace(Some(installed));
                }
            }
        }
        Some(namespace) if !host.collaboration_ids_enabled() => {
            match op_editor_core::PeerNamespace::parse(namespace) {
                Ok(namespace) => {
                    let enabled = namespace.clone();
                    if let Err(error) = host.enable_collaboration_ids(namespace) {
                        // The document already carries ids this namespace
                        // cannot resume above. Staying on the standalone
                        // counter keeps the canvas usable; the push gate is
                        // what keeps those ids off the wire.
                        let _ = error;
                    } else {
                        set_session_namespace(Some(enabled));
                    }
                }
                Err(_) => {
                    host.disable_collaboration_ids();
                    set_session_namespace(None);
                }
            }
        }
        Some(_) => {}
        None => {
            if host.collaboration_ids_enabled() {
                host.disable_collaboration_ids();
            }
            set_session_namespace(None);
        }
    }
}

/// Forget everything scoped to the previous account.
///
/// The daemon snapshot and the minting namespace both belong to the session
/// the old account was in; carrying them into a new account's tab would let
/// its push gate answer from another account's document.
pub(crate) fn reset_for_new_identity() {
    // The previous account's overwritten document must not be restorable in
    // the new account's tab.
    crate::live_sync_recovery::clear();
    DAEMON_NODE_IDS.with(|ids| *ids.borrow_mut() = None);
    SESSION_NAMESPACE.with(|slot| *slot.borrow_mut() = None);
    APPLIED_SEQ.set(None);
    SESSION_LIVE.set(false);
    ACTION_RETRY.with(|slot| *slot.borrow_mut() = None);
    ACTION_BUSY.set(false);
    UNDO_REQUESTED.set(false);
}

/// Drop the id allocator the previous account's session installed.
///
/// Separate from [`reset_for_new_identity`] because it needs the host, which
/// the caller holds. Without it a new account keeps minting ids inside the
/// previous account's namespace — ids the new session never granted it.
pub(crate) fn reset_id_allocation(host: &mut crate::widget_host::WidgetHost) {
    if host.collaboration_ids_enabled() {
        host.disable_collaboration_ids();
    }
    set_session_namespace(None);
}

/// Whether the installed allocator is minting under `namespace`.
fn session_namespace_matches(namespace: &str) -> bool {
    SESSION_NAMESPACE.with(|slot| {
        slot.borrow()
            .as_ref()
            .is_some_and(|installed| installed.as_str() == namespace)
    })
}

/// Record (or clear) the namespace this peer mints under.
fn set_session_namespace(namespace: Option<op_editor_core::PeerNamespace>) {
    SESSION_NAMESPACE.with(|slot| *slot.borrow_mut() = namespace);
}

fn maybe_push_presence<C: RepaintContext + 'static>(inner: &Rc<RefCell<C>>, base: &str) {
    if !SESSION_LIVE.get() {
        return;
    }
    let now = now_ms_f64();
    if now - LAST_PRESENCE_MS.get() < PRESENCE_MIN_INTERVAL_MS {
        return;
    }
    let Ok(context) = inner.try_borrow() else {
        return;
    };
    let (width, height) = context.viewport_size();
    let cursor = context.host().last_cursor_doc_point(width, height);
    drop(context);

    if LAST_PRESENCE_POINT.get() == cursor {
        return;
    }
    let wire = CollabLocalPresenceWire {
        cursor: cursor.map(|(x, y)| CollabPointWire { x, y }),
        client_id: None,
    };
    let Ok(body) = serde_json::to_string(&wire) else {
        return;
    };
    LAST_PRESENCE_MS.set(now);
    LAST_PRESENCE_POINT.set(cursor);
    let _ = live_sync::post_json(&format!("{base}{}", collab_routes::PRESENCE), &body, None);
}

fn now_ms_f64() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map(|performance| performance.now())
        .unwrap_or(0.0)
}

/// Put the stashed local document back on the canvas.
///
/// A normal local edit, so the next push carries it to the daemon and the
/// user's work rejoins the shared document rather than sitting in a cache.
fn restore_stashed_document<C: RepaintContext + 'static>(inner: &Rc<RefCell<C>>) {
    let Some(stashed) = crate::live_sync_recovery::take() else {
        return;
    };
    let Ok(mut context) = inner.try_borrow_mut() else {
        // Put it back rather than losing it to a transient borrow.
        crate::live_sync_recovery::stash(stashed.document, stashed.stashed_at_ms);
        return;
    };
    let state = context.host_mut().editor_state_mut();
    state.replace_document(stashed.document);
    // The offer is consumed; clearing it is what stops the panel from showing
    // a reapply control that would now restore nothing.
    state.editor_ui.collab.discarded_edit = None;
    context.host_mut().mark_editor_state_dirty();
    let _ = context.repaint();
}

pub(crate) fn now_ms() -> u64 {
    now_ms_f64().max(0.0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn doc_with(ids: &[&str]) -> op_editor_core::EditorState {
        let children: Vec<serde_json::Value> = ids
            .iter()
            .map(|id| {
                serde_json::json!({
                    "type": "rectangle", "id": id,
                    "x": 0, "y": 0, "width": 4, "height": 4
                })
            })
            .collect();
        let mut state = op_editor_core::EditorState::starter();
        state.doc = serde_json::from_value(serde_json::json!({
            "version": "1.0",
            "children": children,
        }))
        .expect("valid document");
        state
    }

    pub(super) fn reset_latches() {
        DAEMON_NODE_IDS.with(|ids| *ids.borrow_mut() = None);
        SESSION_NAMESPACE.with(|slot| *slot.borrow_mut() = None);
    }

    pub(super) fn namespace(value: &str) -> op_editor_core::PeerNamespace {
        op_editor_core::PeerNamespace::parse(value.to_string()).expect("valid namespace")
    }

    #[test]
    fn an_idle_shell_never_blocks_a_push() {
        reset_latches();
        let state = doc_with(&["n100", "n101"]);
        assert_eq!(
            state.editor_ui.collab.phase,
            CollabConnectionPhase::Idle,
            "precondition"
        );
        assert!(
            !push_blocked_by_session(&state),
            "a shell with no session must sync exactly as it did before"
        );
    }

    #[test]
    fn deleting_a_node_during_a_session_is_not_blocked() {
        reset_latches();
        let seed = doc_with(&["n100", "n101"]);
        note_daemon_document(&seed);

        let mut shrunk = doc_with(&["n100"]);
        shrunk
            .editor_ui
            .collab
            .set_phase(CollabConnectionPhase::Active);
        assert!(
            !push_blocked_by_session(&shrunk),
            "removals carry no new ids and must keep syncing"
        );
    }

    #[test]
    fn an_active_session_blocks_until_the_first_daemon_document_arrives() {
        reset_latches();
        let mut state = doc_with(&["n100"]);
        state
            .editor_ui
            .collab
            .set_phase(CollabConnectionPhase::Active);
        assert!(
            push_blocked_by_session(&state),
            "with no daemon document to compare against, refusing is the safe answer"
        );
    }

    #[test]
    fn every_ui_action_has_a_wire_form() {
        let key = op_editor_core::CollabAdmissionRequestKey::new("req-1").expect("valid key");
        for action in [
            CollabUiAction::OpenCreate,
            CollabUiAction::Start,
            CollabUiAction::StartLan,
            CollabUiAction::OpenJoin,
            CollabUiAction::BeginDiscovery,
            CollabUiAction::Cancel,
            CollabUiAction::Retry,
            CollabUiAction::Leave,
            CollabUiAction::DiscardPending,
            CollabUiAction::ReapplyDiscarded,
            CollabUiAction::SaveAsFork,
            CollabUiAction::JoinAddress {
                endpoint: "1.2.3.4:5".into(),
            },
            CollabUiAction::JoinDiscovered {
                discovery_id: "d".into(),
            },
            CollabUiAction::ApproveAdmissionEditor {
                request_key: key.clone(),
            },
            CollabUiAction::RejectOwnerIdentity { request_key: key },
        ] {
            let wire = wire_action(&action).expect("every action maps");
            assert!(serde_json::to_string(&wire).is_ok(), "{action:?}");
        }
    }

    #[test]
    fn admission_keys_survive_the_round_trip_to_the_daemon() {
        let key = op_editor_core::CollabAdmissionRequestKey::new("req-abc_1").expect("valid");
        let wire = wire_action(&CollabUiAction::RejectAdmission {
            request_key: key.clone(),
        })
        .expect("maps");
        assert_eq!(
            wire.clone().into_ui_action().expect("revalidates"),
            CollabUiAction::RejectAdmission { request_key: key }
        );
    }
}

/// Route Cmd/Ctrl+Z into the session, returning whether the session claimed it.
///
/// `false` means no session owns the document and the caller should run local
/// history — the same short-circuit contract the desktop host uses
/// (`collab_runtime.request_undo(host) || host.apply_undo()`).
///
/// The request is queued as a wire action rather than answered here: undo has
/// to be sequenced against the other peers, and only the daemon can do that.
pub(crate) fn request_undo(state: &mut op_editor_core::EditorState) -> bool {
    if state.editor_ui.collab.phase != CollabConnectionPhase::Active {
        return false;
    }
    UNDO_REQUESTED.set(true);
    true
}

/// Refuse redo while a session is live.
///
/// M1 collaboration sequences a selective undo per peer but has no matching
/// redo, so the honest answer is a notice rather than a local redo that would
/// diverge this tab from everyone else.
pub(crate) fn reject_redo(state: &mut op_editor_core::EditorState) -> bool {
    if state.editor_ui.collab.phase != CollabConnectionPhase::Active {
        return false;
    }
    state.editor_ui.collab.set_notice(
        op_editor_core::CollabNoticeKind::Reject(op_editor_core::CollabRejectUiCode::Unsupported),
        now_ms(),
    );
    true
}

// ---------------------------------------------------------------------------
// SSE acceleration
// ---------------------------------------------------------------------------

/// Backoff after a dropped stream, doubling to [`SSE_MAX_RETRY_MS`].
const SSE_BASE_RETRY_MS: f64 = 2_000.0;
/// Ceiling for the reconnect backoff. Past this the poll is carrying the load
/// perfectly well, so retrying harder buys nothing.
const SSE_MAX_RETRY_MS: f64 = 60_000.0;

thread_local! {
    /// The live stream, when one is open.
    static EVENT_STREAM: RefCell<Option<web_sys::EventSource>> = const { RefCell::new(None) };
    /// Earliest `performance.now()` at which a reconnect may be attempted.
    static SSE_RETRY_AT_MS: Cell<f64> = const { Cell::new(f64::NEG_INFINITY) };
    /// Consecutive failures, for the backoff exponent.
    static SSE_FAILURES: Cell<u32> = const { Cell::new(0) };
    /// One console warning per degradation, not one per retry.
    static SSE_WARNED: Cell<bool> = const { Cell::new(false) };
}

/// Open the daemon's SSE channel if it is not already open.
///
/// The stream carries the same `{"version":N,"collabSeq":M}` payload the
/// version poll returns, so it feeds the identical latch and changes only
/// *when* a change is noticed — push instead of up to one poll interval later.
/// Everything downstream is unchanged, which is what makes losing the stream a
/// slowdown rather than a failure.
fn ensure_event_stream(base: &str) {
    if EVENT_STREAM.with(|slot| slot.borrow().is_some()) {
        return;
    }
    if now_ms_f64() < SSE_RETRY_AT_MS.get() {
        return;
    }
    let Ok(stream) = web_sys::EventSource::new(&crate::daemon_base::with_tenant_param(&format!(
        "{base}/api/mcp/events"
    ))) else {
        note_stream_failure();
        return;
    };

    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;

    let on_message =
        Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
            if let Some(payload) = event.data().as_string() {
                // A live stream proves the daemon is reachable, so a past
                // failure should not keep throttling reconnects.
                SSE_FAILURES.set(0);
                SSE_WARNED.set(false);
                note_version_probe(&payload);
            }
        });
    stream.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    on_message.forget();

    let on_error = Closure::<dyn FnMut(web_sys::Event)>::new(move |_event: web_sys::Event| {
        // `EventSource` reconnects on its own, but it does so forever and
        // silently against a daemon that has gone away. Closing it and owning
        // the backoff keeps the failure visible in one place and bounded.
        close_event_stream();
        note_stream_failure();
    });
    stream.set_onerror(Some(on_error.as_ref().unchecked_ref()));
    on_error.forget();

    EVENT_STREAM.with(|slot| *slot.borrow_mut() = Some(stream));
}

fn close_event_stream() {
    EVENT_STREAM.with(|slot| {
        if let Some(stream) = slot.borrow_mut().take() {
            stream.close();
        }
    });
}

/// Record a dropped stream and arm the next reconnect.
fn note_stream_failure() {
    let failures = SSE_FAILURES.get().saturating_add(1);
    SSE_FAILURES.set(failures);
    let backoff = (SSE_BASE_RETRY_MS * 2f64.powi(failures.min(5) as i32 - 1)).min(SSE_MAX_RETRY_MS);
    SSE_RETRY_AT_MS.set(now_ms_f64() + backoff);
    // One warning per degradation. A daemon that is simply gone would
    // otherwise fill the console with an identical line every backoff.
    if !SSE_WARNED.replace(true) {
        web_sys::console::warn_1(&wasm_bindgen::JsValue::from_str(
            "[op-collab] event stream unavailable; falling back to polling",
        ));
    }
}

#[path = "collab_sync_push_gate.rs"]
mod push_gate;
pub(crate) use push_gate::push_blocked_by_session;
