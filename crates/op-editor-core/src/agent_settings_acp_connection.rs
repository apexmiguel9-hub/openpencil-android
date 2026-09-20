//! Runtime ACP-agent connect lifecycle state.
//!
//! The persisted ACP configuration only says how to reach an agent
//! (command/args/env or URL). Pressing Connect must run a real host
//! probe and only then mark the agent connected. This module carries
//! the wasm-clean request seam and result state; desktop/web hosts do
//! the transport-specific probe work.

use crate::agent_settings::{AcpAgentConfig, AgentSettings};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AcpAgentConnectPhase {
    #[default]
    Idle,
    Probing,
    Connected,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AcpAgentConnection {
    pub phase: AcpAgentConnectPhase,
    /// Generation of the connect request that owns this runtime state.
    /// Zero is reserved for legacy/synthetic outcomes that did not begin
    /// through [`AgentSettings::begin_acp_agent_connect`].
    pub generation: u64,
    pub info: Option<String>,
    pub error: Option<String>,
}

/// One host-drained ACP connect request.
///
/// The generation is process-local runtime state. It is intentionally not
/// persisted: its only job is distinguishing an old async result from a newer
/// disconnect/reconnect cycle for the same agent id and configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpAgentConnectRequest {
    pub id: String,
    pub generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AcpAgentConnectOutcome {
    pub connected: bool,
    pub info: Option<String>,
    pub error: Option<String>,
}

impl AgentSettings {
    pub fn acp_agent_connection_for(&self, id: &str) -> AcpAgentConnection {
        self.acp_agent_connection
            .get(id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn acp_agent_probe_in_flight(&self, id: &str) -> bool {
        self.acp_agent_connection_for(id).phase == AcpAgentConnectPhase::Probing
    }

    /// Whether an in-flight probe still targets the exact configuration
    /// that launched it. `connected` is runtime state and is deliberately
    /// excluded; every persisted/configurable field, including the display
    /// name used as the ACP initialize fallback, participates.
    pub fn acp_agent_probe_is_current(
        &self,
        expected: &AcpAgentConfig,
        request: &AcpAgentConnectRequest,
    ) -> bool {
        expected.id == request.id
            && self.acp_agent_connect_request_in_flight(request)
            && self
                .acp_agents
                .iter()
                .find(|agent| agent.id == expected.id)
                .is_some_and(|agent| same_probe_config(agent, expected))
    }

    pub fn acp_agent_verified_connected(&self, id: &str) -> bool {
        self.acp_agents
            .iter()
            .any(|agent| agent.id == id && agent.connected)
            && self.acp_agent_connection_for(id).phase == AcpAgentConnectPhase::Connected
    }

    pub fn acp_agent_verified_connected_at(&self, index: usize) -> bool {
        self.acp_agents
            .get(index)
            .is_some_and(|agent| self.acp_agent_verified_connected(&agent.id))
    }

    pub fn begin_acp_agent_connect(&mut self, index: usize) -> Option<String> {
        let id = self.acp_agents.get(index)?.id.clone();
        if self.pending_acp_agent_connect.is_some() {
            return None;
        }
        if self.acp_agent_probe_in_flight(&id) {
            return None;
        }
        if !self.acp_agents.get(index)?.ready() {
            return None;
        }
        let generation = self.acp_agent_connect_generation.checked_add(1)?;
        self.acp_agent_connect_generation = generation;
        let agent = self.acp_agents.get_mut(index)?;
        agent.connected = false;
        self.acp_agent_connection.insert(
            id.clone(),
            AcpAgentConnection {
                phase: AcpAgentConnectPhase::Probing,
                generation,
                ..AcpAgentConnection::default()
            },
        );
        self.pending_acp_agent_connect = Some(AcpAgentConnectRequest {
            id: id.clone(),
            generation,
        });
        Some(id)
    }

    pub fn disconnect_acp_agent(&mut self, index: usize) -> Option<String> {
        let id = self.acp_agents.get(index)?.id.clone();
        self.invalidate_acp_agent_connection(&id);
        Some(id)
    }

    /// Invalidate every runtime connection marker for one configured ACP
    /// agent. Configuration edits, transport changes, deletion, and explicit
    /// disconnect all converge here so no stale detail or pending request can
    /// survive one of those mutations.
    pub fn invalidate_acp_agent_connection(&mut self, id: &str) -> bool {
        let mut changed = false;
        if let Some(agent) = self.acp_agents.iter_mut().find(|agent| agent.id == id) {
            changed |= std::mem::replace(&mut agent.connected, false);
        }
        changed |= self.acp_agent_connection.remove(id).is_some();
        if self
            .pending_acp_agent_connect
            .as_ref()
            .is_some_and(|request| request.id == id)
        {
            self.pending_acp_agent_connect = None;
            changed = true;
        }
        changed
    }

    pub fn acp_agent_connect_request_in_flight(&self, request: &AcpAgentConnectRequest) -> bool {
        self.acp_agent_connection
            .get(&request.id)
            .is_some_and(|connection| {
                connection.phase == AcpAgentConnectPhase::Probing
                    && connection.generation == request.generation
            })
    }

    /// Invalidate only the runtime state still owned by this request.
    ///
    /// Unlike the id-wide explicit mutation helper above, this never clears a
    /// newer reconnect generation that reused the same agent id.
    pub fn invalidate_acp_agent_connect_request(
        &mut self,
        request: &AcpAgentConnectRequest,
    ) -> bool {
        let mut changed = false;
        if self
            .acp_agent_connection
            .get(&request.id)
            .is_some_and(|connection| connection.generation == request.generation)
        {
            if let Some(agent) = self
                .acp_agents
                .iter_mut()
                .find(|agent| agent.id == request.id)
            {
                agent.connected = false;
            }
            self.acp_agent_connection.remove(&request.id);
            changed = true;
        }
        if self.pending_acp_agent_connect.as_ref() == Some(request) {
            self.pending_acp_agent_connect = None;
            changed = true;
        }
        changed
    }

    /// Apply a probe result only if the agent still has the configuration
    /// snapshot that launched the probe and remains in the probing phase.
    ///
    /// Hosts with asynchronous probes must use this contract. A mismatched
    /// snapshot is invalidated rather than allowing an old result to mark the
    /// newly edited configuration as connected.
    pub fn apply_acp_agent_connect_outcome_if_current(
        &mut self,
        expected: &AcpAgentConfig,
        request: &AcpAgentConnectRequest,
        outcome: AcpAgentConnectOutcome,
    ) -> bool {
        if !self.acp_agent_connect_request_in_flight(request) {
            return false;
        }
        let config_matches = self
            .acp_agents
            .iter()
            .find(|agent| agent.id == request.id)
            .is_some_and(|agent| same_probe_config(agent, expected));
        if expected.id != request.id || !config_matches {
            self.invalidate_acp_agent_connect_request(request);
            return false;
        }
        self.apply_acp_agent_connect_outcome(&request.id, outcome)
    }

    /// Apply an outcome where the caller has already established that it is
    /// synchronous/current. Asynchronous hosts should use
    /// [`Self::apply_acp_agent_connect_outcome_if_current`] instead.
    pub fn apply_acp_agent_connect_outcome(
        &mut self,
        id: &str,
        outcome: AcpAgentConnectOutcome,
    ) -> bool {
        let Some(agent) = self.acp_agents.iter_mut().find(|agent| agent.id == id) else {
            self.invalidate_acp_agent_connection(id);
            return false;
        };
        agent.connected = outcome.connected;
        let generation = self
            .acp_agent_connection
            .get(id)
            .map(|connection| connection.generation)
            .unwrap_or_default();
        self.acp_agent_connection.insert(
            id.to_string(),
            AcpAgentConnection {
                phase: if outcome.connected {
                    AcpAgentConnectPhase::Connected
                } else {
                    AcpAgentConnectPhase::Error
                },
                generation,
                info: outcome.info,
                error: outcome.error,
            },
        );
        if self
            .pending_acp_agent_connect
            .as_ref()
            .is_some_and(|request| request.id == id)
        {
            self.pending_acp_agent_connect = None;
        }
        true
    }
}

fn same_probe_config(current: &AcpAgentConfig, expected: &AcpAgentConfig) -> bool {
    current.id == expected.id
        && current.display_name == expected.display_name
        && current.connection_type == expected.connection_type
        && current.command == expected.command
        && current.args == expected.args
        && current.env == expected.env
        && current.url == expected.url
        && current.enabled == expected.enabled
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_settings::AcpConnectionType;
    use std::collections::BTreeMap;

    fn configured_settings() -> AgentSettings {
        let mut settings = AgentSettings::default();
        settings.add_acp_agent_config(
            "Claude Code",
            AcpConnectionType::Local,
            "claude",
            Vec::new(),
            BTreeMap::new(),
            None,
            true,
        );
        settings
    }

    #[test]
    fn begin_acp_agent_connect_raises_request_without_marking_connected() {
        let mut settings = configured_settings();

        let id = settings
            .begin_acp_agent_connect(0)
            .expect("configured agent can start connecting");
        let request = settings
            .pending_acp_agent_connect
            .clone()
            .expect("begin should publish a request");

        assert_eq!(id, "acp-1");
        assert_eq!(request.id, "acp-1");
        assert_eq!(request.generation, 1);
        assert!(!settings.acp_agents[0].connected);
        let connection = settings.acp_agent_connection_for("acp-1");
        assert_eq!(connection.phase, AcpAgentConnectPhase::Probing);
        assert_eq!(connection.generation, request.generation);
    }

    #[test]
    fn failed_acp_agent_connect_keeps_agent_disconnected() {
        let mut settings = configured_settings();
        settings.begin_acp_agent_connect(0);

        assert!(settings.apply_acp_agent_connect_outcome(
            "acp-1",
            AcpAgentConnectOutcome {
                connected: false,
                error: Some("ACP initialize failed".into()),
                ..AcpAgentConnectOutcome::default()
            },
        ));

        assert!(!settings.acp_agents[0].connected);
        assert_eq!(settings.pending_acp_agent_connect, None);
        let conn = settings.acp_agent_connection_for("acp-1");
        assert_eq!(conn.phase, AcpAgentConnectPhase::Error);
        assert_eq!(conn.error.as_deref(), Some("ACP initialize failed"));
    }

    #[test]
    fn successful_acp_agent_connect_marks_agent_connected() {
        let mut settings = configured_settings();
        settings.begin_acp_agent_connect(0);

        assert!(settings.apply_acp_agent_connect_outcome(
            "acp-1",
            AcpAgentConnectOutcome {
                connected: true,
                info: Some("Claude Code".into()),
                ..AcpAgentConnectOutcome::default()
            },
        ));

        assert!(settings.acp_agents[0].connected);
        assert_eq!(settings.pending_acp_agent_connect, None);
        let conn = settings.acp_agent_connection_for("acp-1");
        assert_eq!(conn.phase, AcpAgentConnectPhase::Connected);
        assert_eq!(conn.info.as_deref(), Some("Claude Code"));
    }

    #[test]
    fn disconnect_acp_agent_clears_runtime_connection_state() {
        let mut settings = configured_settings();
        settings.apply_acp_agent_connect_outcome(
            "acp-1",
            AcpAgentConnectOutcome {
                connected: true,
                info: Some("Claude Code".into()),
                ..AcpAgentConnectOutcome::default()
            },
        );

        assert_eq!(settings.disconnect_acp_agent(0).as_deref(), Some("acp-1"));

        assert!(!settings.acp_agents[0].connected);
        assert_eq!(
            settings.acp_agent_connection_for("acp-1"),
            AcpAgentConnection::default()
        );
    }

    #[test]
    fn stale_probe_outcome_is_rejected_after_configuration_changes() {
        let mut settings = configured_settings();
        settings.begin_acp_agent_connect(0);
        let expected = settings.acp_agents[0].clone();
        let request = settings
            .pending_acp_agent_connect
            .clone()
            .expect("probe request");
        settings.acp_agents[0].command = "different-agent".into();

        assert!(!settings.apply_acp_agent_connect_outcome_if_current(
            &expected,
            &request,
            AcpAgentConnectOutcome {
                connected: true,
                info: Some("Old Agent".into()),
                ..AcpAgentConnectOutcome::default()
            },
        ));

        assert!(!settings.acp_agents[0].connected);
        assert_eq!(settings.pending_acp_agent_connect, None);
        assert_eq!(
            settings.acp_agent_connection_for("acp-1"),
            AcpAgentConnection::default()
        );
    }

    #[test]
    fn old_generation_cannot_land_after_same_config_reconnect() {
        let mut settings = configured_settings();
        settings.begin_acp_agent_connect(0);
        let expected = settings.acp_agents[0].clone();
        let old_request = settings
            .pending_acp_agent_connect
            .clone()
            .expect("old request");

        settings.disconnect_acp_agent(0);
        settings.begin_acp_agent_connect(0);
        let new_request = settings
            .pending_acp_agent_connect
            .clone()
            .expect("replacement request");
        assert!(new_request.generation > old_request.generation);

        assert!(!settings.apply_acp_agent_connect_outcome_if_current(
            &expected,
            &old_request,
            AcpAgentConnectOutcome {
                connected: true,
                info: Some("Old Agent".into()),
                ..AcpAgentConnectOutcome::default()
            },
        ));

        assert!(!settings.acp_agents[0].connected);
        assert_eq!(
            settings.pending_acp_agent_connect.as_ref(),
            Some(&new_request)
        );
        let connection = settings.acp_agent_connection_for("acp-1");
        assert_eq!(connection.phase, AcpAgentConnectPhase::Probing);
        assert_eq!(connection.generation, new_request.generation);
    }

    #[test]
    fn third_connect_cannot_overwrite_the_single_queued_request() {
        let mut settings = configured_settings();
        for name in ["Gemini", "Codex"] {
            settings.add_acp_agent_config(
                name,
                AcpConnectionType::Local,
                name.to_ascii_lowercase(),
                Vec::new(),
                BTreeMap::new(),
                None,
                true,
            );
        }

        settings.begin_acp_agent_connect(0);
        let active_request = settings
            .pending_acp_agent_connect
            .take()
            .expect("host drains the first request into its active job");
        assert_eq!(active_request.id, "acp-1");

        assert_eq!(
            settings.begin_acp_agent_connect(1).as_deref(),
            Some("acp-2")
        );
        let queued_request = settings
            .pending_acp_agent_connect
            .clone()
            .expect("second request remains queued");

        assert_eq!(settings.begin_acp_agent_connect(2), None);
        assert_eq!(
            settings.pending_acp_agent_connect.as_ref(),
            Some(&queued_request)
        );
        assert_eq!(
            settings.acp_agent_connection_for("acp-2").phase,
            AcpAgentConnectPhase::Probing
        );
        assert_eq!(
            settings.acp_agent_connection_for("acp-3").phase,
            AcpAgentConnectPhase::Idle
        );
    }

    #[test]
    fn removing_acp_agent_clears_pending_and_runtime_map() {
        let mut settings = configured_settings();
        settings.begin_acp_agent_connect(0);
        assert!(settings.acp_agent_connection.contains_key("acp-1"));

        assert!(settings.remove_acp_agent("acp-1"));

        assert!(settings.acp_agents.is_empty());
        assert_eq!(settings.pending_acp_agent_connect, None);
        assert!(!settings.acp_agent_connection.contains_key("acp-1"));
    }

    #[test]
    fn verified_connection_requires_successful_probe_state() {
        let mut settings = configured_settings();
        settings.acp_agents[0].connected = true;

        assert!(
            !settings.acp_agent_verified_connected("acp-1"),
            "a stale persisted connected flag is not a real ACP connection"
        );

        settings.acp_agent_connection.insert(
            "acp-1".into(),
            AcpAgentConnection {
                phase: AcpAgentConnectPhase::Connected,
                info: Some("Claude Code".into()),
                error: None,
                ..AcpAgentConnection::default()
            },
        );

        assert!(settings.acp_agent_verified_connected("acp-1"));
        assert!(settings.acp_agent_verified_connected_at(0));
    }
}
