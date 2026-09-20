//! Browser-side bridge for desktop-daemon ACP agent probes.

use std::cell::RefCell;
use std::rc::Rc;

use op_editor_core::agent_settings::{
    AcpAgentConfig, AcpAgentConnectOutcome, AcpAgentConnectRequest,
};
use op_editor_core::EditorState;
use wasm_bindgen::JsValue;

use crate::repaint_ctx::RepaintContext;

type InnerRc<C> = Rc<RefCell<C>>;

fn console_warn(msg: &str) {
    web_sys::console::warn_1(&JsValue::from_str(msg));
}

pub(crate) fn drain_pending_acp_agent_connect<C: RepaintContext + 'static>(inner: &InnerRc<C>) {
    let pending = inner
        .borrow_mut()
        .host_mut()
        .editor_state_mut()
        .editor_ui
        .agent_settings
        .pending_acp_agent_connect
        .take();
    let Some(request) = pending else {
        return;
    };
    let expected = inner
        .borrow()
        .host()
        .editor_state()
        .editor_ui
        .agent_settings
        .acp_agents
        .iter()
        .find(|agent| agent.id == request.id)
        .cloned();
    let Some(expected) = expected else {
        let mut b = inner.borrow_mut();
        b.host_mut()
            .editor_state_mut()
            .editor_ui
            .agent_settings
            .invalidate_acp_agent_connect_request(&request);
        b.host_mut().mark_editor_state_dirty();
        let _ = b.repaint();
        return;
    };

    let body = serde_json::json!({
        "id": &request.id,
        "generation": request.generation,
    })
    .to_string();
    let base = crate::daemon_base::daemon_base();
    let request_for_response = request.clone();
    let expected_for_response = expected.clone();
    let inner_for_response = inner.clone();
    let on_response: Rc<dyn Fn(String)> = Rc::new(move |response: String| {
        let mut b = inner_for_response.borrow_mut();
        if apply_acp_agent_connect_response(
            b.host_mut().editor_state_mut(),
            &expected_for_response,
            &request_for_response,
            &response,
        ) {
            b.host_mut().mark_editor_state_dirty();
            let _ = b.repaint();
        }
    });
    if !crate::live_sync::post_json(&format!("{base}/api/acp/connect"), &body, Some(on_response)) {
        let mut b = inner.borrow_mut();
        apply_acp_agent_connect_error(
            b.host_mut().editor_state_mut(),
            &expected,
            &request,
            "ACP agent connection request could not start. Is the web daemon running?",
        );
        b.host_mut().mark_editor_state_dirty();
        let _ = b.repaint();
        console_warn("ACP agent connect request could not start");
    }
}

pub(crate) fn apply_acp_agent_connect_response(
    state: &mut EditorState,
    expected: &AcpAgentConfig,
    request: &AcpAgentConnectRequest,
    body: &str,
) -> bool {
    if !state
        .editor_ui
        .agent_settings
        .acp_agent_connect_request_in_flight(request)
    {
        return false;
    }
    let parsed: serde_json::Value = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(_) => {
            return apply_acp_agent_connect_error(
                state,
                expected,
                request,
                "ACP agent connection failed: invalid daemon response",
            );
        }
    };
    if parsed
        .get("id")
        .is_some_and(|value| value.as_str() != Some(request.id.as_str()))
        || parsed
            .get("generation")
            .is_some_and(|value| value.as_u64() != Some(request.generation))
    {
        return false;
    }
    let connected = parsed
        .get("connected")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let info = str_field(&parsed, "connectionInfo").or_else(|| str_field(&parsed, "info"));
    let error = str_field(&parsed, "error");
    let applied = state
        .editor_ui
        .agent_settings
        .apply_acp_agent_connect_outcome_if_current(
            expected,
            request,
            AcpAgentConnectOutcome {
                connected,
                info,
                error,
            },
        );
    if applied {
        state.rebuild_chat_models();
    }
    applied
}

fn apply_acp_agent_connect_error(
    state: &mut EditorState,
    expected: &AcpAgentConfig,
    request: &AcpAgentConnectRequest,
    error: &str,
) -> bool {
    let applied = state
        .editor_ui
        .agent_settings
        .apply_acp_agent_connect_outcome_if_current(
            expected,
            request,
            AcpAgentConnectOutcome {
                connected: false,
                error: Some(error.to_string()),
                ..Default::default()
            },
        );
    if applied {
        state.rebuild_chat_models();
    }
    applied
}

fn str_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_editor_core::agent_settings::{AcpAgentConnectPhase, AcpConnectionType};
    use std::collections::BTreeMap;

    fn state_with_pending_acp() -> (EditorState, AcpAgentConfig, AcpAgentConnectRequest) {
        let mut state = EditorState::new();
        state.editor_ui.agent_settings.add_acp_agent_config(
            "Claude Code",
            AcpConnectionType::Local,
            "claude",
            Vec::new(),
            BTreeMap::new(),
            None,
            true,
        );
        state.editor_ui.agent_settings.begin_acp_agent_connect(0);
        let expected = state.editor_ui.agent_settings.acp_agents[0].clone();
        let request = state
            .editor_ui
            .agent_settings
            .pending_acp_agent_connect
            .clone()
            .expect("connect request");
        (state, expected, request)
    }

    #[test]
    fn web_acp_connect_response_marks_agent_connected() {
        let (mut state, expected, request) = state_with_pending_acp();
        let body = serde_json::json!({
            "id": &request.id,
            "generation": request.generation,
            "connected": true,
            "connectionInfo": "Claude Code 1.0"
        })
        .to_string();

        assert!(apply_acp_agent_connect_response(
            &mut state, &expected, &request, &body
        ));

        let settings = &state.editor_ui.agent_settings;
        assert!(settings.acp_agents[0].connected);
        let conn = settings.acp_agent_connection_for("acp-1");
        assert_eq!(conn.phase, AcpAgentConnectPhase::Connected);
        assert_eq!(conn.info.as_deref(), Some("Claude Code 1.0"));
    }

    #[test]
    fn web_acp_connect_response_failure_keeps_agent_disconnected() {
        let (mut state, expected, request) = state_with_pending_acp();
        let body = serde_json::json!({
            "id": &request.id,
            "generation": request.generation,
            "connected": false,
            "error": "failed to spawn ACP agent"
        })
        .to_string();

        assert!(apply_acp_agent_connect_response(
            &mut state, &expected, &request, &body
        ));

        let settings = &state.editor_ui.agent_settings;
        assert!(!settings.acp_agents[0].connected);
        let conn = settings.acp_agent_connection_for("acp-1");
        assert_eq!(conn.phase, AcpAgentConnectPhase::Error);
        assert_eq!(conn.error.as_deref(), Some("failed to spawn ACP agent"));
    }

    #[test]
    fn old_response_cannot_overwrite_same_config_reconnect() {
        let (mut state, expected, old_request) = state_with_pending_acp();
        state.editor_ui.agent_settings.disconnect_acp_agent(0);
        state.editor_ui.agent_settings.begin_acp_agent_connect(0);
        let new_request = state
            .editor_ui
            .agent_settings
            .pending_acp_agent_connect
            .clone()
            .expect("replacement request");
        let body = serde_json::json!({
            "id": &old_request.id,
            "generation": old_request.generation,
            "connected": true,
            "connectionInfo": "Old Agent"
        })
        .to_string();

        assert!(!apply_acp_agent_connect_response(
            &mut state,
            &expected,
            &old_request,
            &body
        ));

        let settings = &state.editor_ui.agent_settings;
        assert!(!settings.acp_agents[0].connected);
        assert_eq!(
            settings.pending_acp_agent_connect.as_ref(),
            Some(&new_request)
        );
        assert_eq!(
            settings.acp_agent_connection_for("acp-1").generation,
            new_request.generation
        );
    }

    #[test]
    fn edited_config_reconnect_rejects_old_response_without_clearing_new_request() {
        let (mut state, expected, old_request) = state_with_pending_acp();
        state
            .editor_ui
            .agent_settings
            .invalidate_acp_agent_connection("acp-1");
        state.editor_ui.agent_settings.acp_agents[0].command = "new-agent".into();
        state.editor_ui.agent_settings.begin_acp_agent_connect(0);
        let new_request = state
            .editor_ui
            .agent_settings
            .pending_acp_agent_connect
            .clone()
            .expect("replacement request");
        let body = serde_json::json!({
            "id": &old_request.id,
            "generation": old_request.generation,
            "connected": true,
            "connectionInfo": "Old Agent"
        })
        .to_string();

        assert!(!apply_acp_agent_connect_response(
            &mut state,
            &expected,
            &old_request,
            &body
        ));

        let settings = &state.editor_ui.agent_settings;
        assert!(!settings.acp_agents[0].connected);
        assert_eq!(
            settings.pending_acp_agent_connect.as_ref(),
            Some(&new_request)
        );
        assert_eq!(
            settings.acp_agent_connection_for("acp-1").generation,
            new_request.generation
        );
    }
}
