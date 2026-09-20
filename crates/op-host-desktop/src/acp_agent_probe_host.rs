//! Host pump for connect-time ACP agent probes.
//!
//! The job struct + probe fns live in
//! [`op_host_services::acp_agent_probe_host`] (codex Issue 5 — the job is
//! a `DesktopApp` field); this residual keeps only the `impl DesktopApp`
//! pump, which drives the job through its public API.

use op_editor_core::agent_settings::AcpAgentConnectOutcome;

use crate::DesktopApp;
// Re-export so `crate::acp_agent_probe_host::AcpAgentConnectJob` (the
// `DesktopApp` field type in `main.rs`) still resolves with zero churn.
pub use op_host_services::acp_agent_probe_host::AcpAgentConnectJob;

/// Keep the quick-add rows' "is this CLI installed?" map in step with the
/// settings modal.
///
/// Probing on open (rather than once at startup) is what makes the state
/// recoverable: a user who installs `qwen` while the app is running gets
/// an un-greyed row on the next open instead of after a restart. Closing
/// the modal drops the map so the next open re-probes rather than
/// painting a stale answer. Returns `true` when the map changed.
pub fn refresh_acp_preset_availability(
    settings: &mut op_editor_core::AgentSettings,
    open: bool,
) -> bool {
    if !open {
        let had_entries = !settings.acp_preset_installed.is_empty();
        settings.acp_preset_installed.clear();
        return had_entries;
    }
    if !settings.acp_preset_installed.is_empty() {
        return false;
    }
    settings.acp_preset_installed =
        op_host_services::acp_agent_probe_host::probe_acp_preset_availability();
    !settings.acp_preset_installed.is_empty()
}

impl DesktopApp {
    pub(crate) fn drain_acp_agent_connect(&mut self) -> bool {
        let mut changed = self.discard_stale_acp_agent_connect_job();
        changed |= self.poll_acp_agent_connect_job();
        if self
            .acp_agent_connect_job
            .as_ref()
            .is_some_and(AcpAgentConnectJob::is_pending)
        {
            return changed;
        }
        let pending = self
            .host
            .editor_state_mut()
            .editor_ui
            .agent_settings
            .pending_acp_agent_connect
            .take();
        if let Some(request) = pending {
            let agent = self
                .host
                .editor_state()
                .editor_ui
                .agent_settings
                .acp_agents
                .iter()
                .find(|agent| agent.id == request.id && agent.ready())
                .cloned();
            if let Some(agent) = agent {
                self.acp_agent_connect_job =
                    Some(AcpAgentConnectJob::spawn(request.clone(), agent));
            } else {
                let es = self.host.editor_state_mut();
                es.editor_ui
                    .agent_settings
                    .invalidate_acp_agent_connect_request(&request);
                es.rebuild_chat_models();
                self.host.mark_editor_state_dirty();
            }
            changed = true;
        }
        changed
    }

    fn discard_stale_acp_agent_connect_job(&mut self) -> bool {
        let Some(job) = self.acp_agent_connect_job.as_ref() else {
            return false;
        };
        if self
            .host
            .editor_state()
            .editor_ui
            .agent_settings
            .acp_agent_probe_is_current(job.config(), job.request())
        {
            return false;
        }

        let request = job.request().clone();
        self.acp_agent_connect_job = None;
        let replacement_pending = self
            .host
            .editor_state()
            .editor_ui
            .agent_settings
            .pending_acp_agent_connect
            .as_ref()
            .is_some_and(|pending| {
                pending.id == request.id && pending.generation != request.generation
            });
        if replacement_pending {
            // The old job's request was already drained before it spawned,
            // so a new pending request for the same id belongs to the edited
            // configuration. Keep it intact; `drain_acp_agent_connect` will
            // launch its replacement below.
            return true;
        }
        let es = self.host.editor_state_mut();
        es.editor_ui
            .agent_settings
            .invalidate_acp_agent_connect_request(&request);
        es.rebuild_chat_models();
        self.host.mark_editor_state_dirty();
        true
    }

    fn poll_acp_agent_connect_job(&mut self) -> bool {
        let Some(job) = self.acp_agent_connect_job.as_mut() else {
            return false;
        };
        let Some(outcome) = job.poll() else {
            return false;
        };
        let expected = job.config().clone();
        let request = job.request().clone();
        self.acp_agent_connect_job = None;

        let es = self.host.editor_state_mut();
        es.editor_ui
            .agent_settings
            .apply_acp_agent_connect_outcome_if_current(
                &expected,
                &request,
                AcpAgentConnectOutcome {
                    connected: outcome.connected,
                    info: outcome.info,
                    error: outcome.error,
                },
            );
        es.rebuild_chat_models();
        self.host.mark_editor_state_dirty();
        true
    }

    pub(crate) fn acp_agent_connect_pending(&self) -> bool {
        self.acp_agent_connect_job
            .as_ref()
            .is_some_and(AcpAgentConnectJob::is_pending)
            || self
                .host
                .editor_state()
                .editor_ui
                .agent_settings
                .pending_acp_agent_connect
                .is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_editor_core::agent_settings::{AcpAgentConnectPhase, AcpConnectionType};
    use op_host_services::acp_agent_probe_host::AcpAgentProbeOutcome;
    use std::collections::BTreeMap;

    /// A `DesktopApp` whose ACP slate is empty.
    ///
    /// `DesktopApp::new` runs the real `settings_io::load`, so it inherits
    /// whatever ACP agents the developer running the suite happens to have
    /// configured. Every test below then adds one fixture agent and drives
    /// it as index 0 / id `acp-1` — which silently becomes a *different*
    /// agent the moment the machine has one of its own. Clearing here is
    /// what makes the fixture mean what it reads like.
    fn app_with_empty_acp_slate() -> DesktopApp {
        let mut app = DesktopApp::new(None);
        let settings = &mut app.host.editor_state_mut().editor_ui.agent_settings;
        settings.acp_agents.clear();
        settings.acp_agent_connection.clear();
        settings.pending_acp_agent_connect = None;
        settings.next_acp_agent_id = 1;
        app
    }

    #[test]
    fn preset_availability_is_probed_on_open_and_dropped_on_close() {
        let mut settings = op_editor_core::AgentSettings::default();
        assert!(settings.acp_preset_installed.is_empty());

        assert!(refresh_acp_preset_availability(&mut settings, true));
        assert_eq!(
            settings.acp_preset_installed.len(),
            op_editor_core::ACP_AGENT_PRESETS.len(),
            "an open modal should classify every preset, installed or not"
        );
        // Whatever this machine has, no preset may be left Unknown once the
        // host has actually looked.
        for preset in &op_editor_core::ACP_AGENT_PRESETS {
            assert_ne!(
                settings.acp_preset_availability(preset.id),
                op_editor_core::AcpPresetAvailability::Unknown
            );
        }
        assert!(
            !refresh_acp_preset_availability(&mut settings, true),
            "a second frame with the modal still open must not re-probe PATH"
        );

        assert!(refresh_acp_preset_availability(&mut settings, false));
        assert!(
            settings.acp_preset_installed.is_empty(),
            "closing drops the snapshot so reopening re-probes instead of \
             painting a stale answer"
        );
        assert!(!refresh_acp_preset_availability(&mut settings, false));
    }

    #[test]
    fn landed_acp_probe_failure_keeps_agent_disconnected() {
        let mut app = app_with_empty_acp_slate();
        let settings = &mut app.host.editor_state_mut().editor_ui.agent_settings;
        settings.add_acp_agent_config(
            "Claude Code",
            AcpConnectionType::Local,
            "claude",
            Vec::new(),
            BTreeMap::new(),
            None,
            true,
        );
        settings.begin_acp_agent_connect(0);
        let request = settings
            .pending_acp_agent_connect
            .take()
            .expect("probe request");
        let expected = settings.acp_agents[0].clone();
        let (job, tx) = AcpAgentConnectJob::pending_for_test(request, expected);
        app.acp_agent_connect_job = Some(job);

        tx.send(AcpAgentProbeOutcome::failed("initialize failed"))
            .unwrap();

        assert!(app.drain_acp_agent_connect());
        let settings = &app.host.editor_state().editor_ui.agent_settings;
        assert!(!settings.acp_agents[0].connected);
        let conn = settings.acp_agent_connection_for("acp-1");
        assert_eq!(conn.phase, AcpAgentConnectPhase::Error);
        assert_eq!(conn.error.as_deref(), Some("initialize failed"));
    }

    #[test]
    fn landed_acp_probe_success_marks_agent_connected() {
        let mut app = app_with_empty_acp_slate();
        let settings = &mut app.host.editor_state_mut().editor_ui.agent_settings;
        settings.add_acp_agent_config(
            "Claude Code",
            AcpConnectionType::Local,
            "claude",
            Vec::new(),
            BTreeMap::new(),
            None,
            true,
        );
        settings.begin_acp_agent_connect(0);
        let request = settings
            .pending_acp_agent_connect
            .take()
            .expect("probe request");
        let expected = settings.acp_agents[0].clone();
        let (job, tx) = AcpAgentConnectJob::pending_for_test(request, expected);
        app.acp_agent_connect_job = Some(job);

        tx.send(AcpAgentProbeOutcome::connected("Claude Code 1.0".into()))
            .unwrap();

        assert!(app.drain_acp_agent_connect());
        let settings = &app.host.editor_state().editor_ui.agent_settings;
        assert!(settings.acp_agents[0].connected);
        let conn = settings.acp_agent_connection_for("acp-1");
        assert_eq!(conn.phase, AcpAgentConnectPhase::Connected);
        assert_eq!(conn.info.as_deref(), Some("Claude Code 1.0"));
    }

    #[test]
    fn probe_result_is_discarded_when_configuration_changes_in_flight() {
        let mut app = app_with_empty_acp_slate();
        let settings = &mut app.host.editor_state_mut().editor_ui.agent_settings;
        settings.add_acp_agent_config(
            "Claude Code",
            AcpConnectionType::Local,
            "claude",
            Vec::new(),
            BTreeMap::new(),
            None,
            true,
        );
        settings.begin_acp_agent_connect(0);
        let request = settings
            .pending_acp_agent_connect
            .take()
            .expect("probe request");
        let expected = settings.acp_agents[0].clone();
        let (job, tx) = AcpAgentConnectJob::pending_for_test(request, expected);
        app.acp_agent_connect_job = Some(job);
        app.host
            .editor_state_mut()
            .editor_ui
            .agent_settings
            .acp_agents[0]
            .command = "different-agent".into();

        tx.send(AcpAgentProbeOutcome::connected("Old Agent 1.0".into()))
            .unwrap();

        assert!(app.drain_acp_agent_connect());
        let settings = &app.host.editor_state().editor_ui.agent_settings;
        assert!(!settings.acp_agents[0].connected);
        assert_eq!(settings.pending_acp_agent_connect, None);
        assert_eq!(
            settings.acp_agent_connection_for("acp-1"),
            Default::default()
        );
        assert!(app.acp_agent_connect_job.is_none());
    }

    #[test]
    fn stale_job_discard_preserves_reconnect_for_edited_configuration() {
        let mut app = app_with_empty_acp_slate();
        let settings = &mut app.host.editor_state_mut().editor_ui.agent_settings;
        settings.add_acp_agent_config(
            "Claude Code",
            AcpConnectionType::Local,
            "claude",
            Vec::new(),
            BTreeMap::new(),
            None,
            true,
        );
        settings.begin_acp_agent_connect(0);
        let request = settings
            .pending_acp_agent_connect
            .take()
            .expect("probe request");
        let expected = settings.acp_agents[0].clone();
        let (job, _tx) = AcpAgentConnectJob::pending_for_test(request, expected);
        app.acp_agent_connect_job = Some(job);

        let settings = &mut app.host.editor_state_mut().editor_ui.agent_settings;
        settings.invalidate_acp_agent_connection("acp-1");
        settings.acp_agents[0].command = "different-agent".into();
        settings.begin_acp_agent_connect(0);

        assert!(app.discard_stale_acp_agent_connect_job());
        let settings = &app.host.editor_state().editor_ui.agent_settings;
        assert_eq!(
            settings
                .pending_acp_agent_connect
                .as_ref()
                .map(|request| request.id.as_str()),
            Some("acp-1")
        );
        assert_eq!(
            settings.acp_agent_connection_for("acp-1").phase,
            AcpAgentConnectPhase::Probing
        );
        assert!(app.acp_agent_connect_job.is_none());
    }

    #[test]
    fn ready_old_job_is_discarded_after_same_config_disconnect_reconnect() {
        let mut app = app_with_empty_acp_slate();
        let settings = &mut app.host.editor_state_mut().editor_ui.agent_settings;
        settings.add_acp_agent_config(
            "Claude Code",
            AcpConnectionType::Local,
            "claude",
            Vec::new(),
            BTreeMap::new(),
            None,
            true,
        );
        settings.begin_acp_agent_connect(0);
        let old_request = settings
            .pending_acp_agent_connect
            .take()
            .expect("old probe request");
        let expected = settings.acp_agents[0].clone();
        let (job, tx) = AcpAgentConnectJob::pending_for_test(old_request.clone(), expected);
        app.acp_agent_connect_job = Some(job);

        let settings = &mut app.host.editor_state_mut().editor_ui.agent_settings;
        settings.disconnect_acp_agent(0);
        settings.begin_acp_agent_connect(0);
        let new_request = settings
            .pending_acp_agent_connect
            .clone()
            .expect("replacement probe request");
        assert!(new_request.generation > old_request.generation);
        tx.send(AcpAgentProbeOutcome::connected("Old Agent 1.0".into()))
            .unwrap();

        assert!(app.discard_stale_acp_agent_connect_job());
        let settings = &app.host.editor_state().editor_ui.agent_settings;
        assert_eq!(
            settings.pending_acp_agent_connect.as_ref(),
            Some(&new_request)
        );
        let connection = settings.acp_agent_connection_for("acp-1");
        assert_eq!(connection.phase, AcpAgentConnectPhase::Probing);
        assert_eq!(connection.generation, new_request.generation);
        assert!(!settings.acp_agents[0].connected);
        assert!(app.acp_agent_connect_job.is_none());
    }
}

#[cfg(test)]
#[path = "acp_local_e2e_tests.rs"]
mod local_e2e_tests;
