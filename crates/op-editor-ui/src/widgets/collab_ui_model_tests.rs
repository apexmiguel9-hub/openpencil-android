//! Presentation-model tests for the collaboration panel and top bar.

use super::*;
use op_editor_core::{
    AuthenticatedCollabSession, CollabAdmissionRequestKey, CollabPanelView, CollabParticipantUi,
    CollabShareEndpoint, Locale,
};

fn participant(key: &str, name: &str) -> CollabParticipantUi {
    CollabParticipantUi::new(key, name, 0x3366ffff, CollabUiRole::Editor, false)
}

#[test]
fn pre_auth_panel_never_contains_session_or_participant_profiles() {
    let mut ui = EditorUiState::default();
    ui.collab.availability = CollabAvailability::Ready;
    ui.collab.phase = CollabConnectionPhase::Authenticating;
    let model = CollabPanelModel::for_editor_ui(&ui);
    assert!(matches!(model.screen, CollabPanelScreen::Progress { .. }));
    assert!(!format!("{model:?}").contains("participant"));
}

#[test]
fn authenticated_topbar_caps_avatar_stack_and_reports_overflow() {
    let mut ui = EditorUiState::default();
    ui.collab.availability = CollabAvailability::Ready;
    ui.collab.set_authenticated_session(
        CollabConnectionPhase::Active,
        AuthenticatedCollabSession {
            session_name: "Landing page".to_string(),
            role: CollabUiRole::Editor,
            share_endpoint: None,
        },
        vec![
            participant("p1", "Ada"),
            participant("p2", "Grace"),
            participant("p3", "Linus"),
            participant("p4", "Margaret"),
        ],
    );
    let model = CollabTopBarModel::for_editor_ui(&ui);
    assert_eq!(model.avatars.len(), 3);
    assert_eq!(model.participant_overflow, 1);
    assert_eq!(model.tone, CollabTopBarTone::Connected);
}

#[test]
fn participant_models_project_to_both_surfaces_without_profile_urls() {
    let mut ui = EditorUiState::default();
    ui.collab.availability = CollabAvailability::Ready;
    ui.collab.set_authenticated_session(
        CollabConnectionPhase::Active,
        AuthenticatedCollabSession {
            session_name: "Design".to_string(),
            role: CollabUiRole::Editor,
            share_endpoint: None,
        },
        vec![
            CollabParticipantUi::new("owner", "Owner", 0x3366ffff, CollabUiRole::Owner, false),
            CollabParticipantUi::new("guest", "Guest", 0x6633ffff, CollabUiRole::Editor, true),
        ],
    );

    let topbar = CollabTopBarModel::for_editor_ui(&ui);
    assert_eq!(
        topbar
            .avatars
            .iter()
            .map(|avatar| avatar.participant_key.as_str())
            .collect::<Vec<_>>(),
        vec!["owner", "guest"]
    );
    let panel = CollabPanelModel::for_editor_ui(&ui);
    let CollabPanelScreen::Session { participants, .. } = panel.screen else {
        panic!("expected session model");
    };
    assert_eq!(participants, topbar.avatars);
}

#[test]
fn join_model_exposes_only_discovery_endpoint_data() {
    let mut ui = EditorUiState::default();
    ui.collab.availability = CollabAvailability::Ready;
    ui.collab.panel.view = CollabPanelView::Join;
    ui.collab.panel.join_input.set_text("10.0.0.2:43120");
    ui.collab.panel.discovered = std::sync::Arc::new(vec![DiscoveredCollabEndpoint {
        discovery_id: "opaque-1".to_string(),
        endpoint: "10.0.0.3:43120".to_string(),
        compatible: true,
    }]);
    let model = CollabPanelModel::for_editor_ui(&ui);
    let CollabPanelScreen::Join { discovered, .. } = model.screen else {
        panic!("expected join model");
    };
    assert_eq!(discovered[0].endpoint, "10.0.0.3:43120");
}

#[test]
fn gate_reason_uses_the_active_locale() {
    let ui = EditorUiState {
        locale: Locale::ZhCn,
        ..Default::default()
    };
    assert_eq!(
        gate_reason_text(&ui, CollabGateReason::OwnerOnlySave),
        "只有所有者可以保存共享源文件。"
    );
}

#[test]
fn action_queue_is_single_flight() {
    let mut ui = EditorUiState::default();
    assert!(request_action(&mut ui, CollabUiAction::Start));
    assert!(!request_action(&mut ui, CollabUiAction::Leave));
    assert_eq!(ui.collab.take_pending_action(), Some(CollabUiAction::Start));
}

#[test]
fn owner_admission_model_has_three_decisions_without_identity_data() {
    let mut ui = EditorUiState::default();
    ui.collab.availability = CollabAvailability::Ready;
    ui.collab.set_authenticated_session(
        CollabConnectionPhase::Active,
        AuthenticatedCollabSession {
            session_name: "Design".to_string(),
            role: CollabUiRole::Owner,
            share_endpoint: None,
        },
        Vec::new(),
    );
    let request_key = CollabAdmissionRequestKey::new("opaque-request-7").unwrap();
    assert!(ui
        .collab
        .publish_pending_admission(request_key.clone(), None));

    let model = CollabPanelModel::for_editor_ui(&ui);
    let CollabPanelScreen::Session {
        admission_request: Some(request),
        ..
    } = &model.screen
    else {
        panic!("owner must see the oldest pending admission");
    };
    assert_eq!(request.actions.len(), 3);
    assert!(request.actions.iter().any(|action| {
        action.action
            == CollabUiAction::ApproveAdmissionEditor {
                request_key: request_key.clone(),
            }
    }));
    assert!(request.actions.iter().any(|action| {
        action.action
            == CollabUiAction::ApproveAdmissionViewer {
                request_key: request_key.clone(),
            }
    }));
    assert!(request.actions.iter().any(|action| {
        action.action
            == CollabUiAction::RejectAdmission {
                request_key: request_key.clone(),
            }
    }));
    let debug = format!("{model:?}");
    assert!(!debug.contains(request_key.as_str()));
    assert!(!debug.contains("subject"));
    assert!(!debug.contains("device"));
}

#[test]
fn viewer_panel_cannot_project_owner_admission_controls() {
    let mut ui = EditorUiState::default();
    ui.collab.availability = CollabAvailability::Ready;
    ui.collab.set_authenticated_session(
        CollabConnectionPhase::Active,
        AuthenticatedCollabSession {
            session_name: "Design".to_string(),
            role: CollabUiRole::Viewer,
            share_endpoint: None,
        },
        Vec::new(),
    );
    let model = CollabPanelModel::for_editor_ui(&ui);
    let CollabPanelScreen::Session {
        admission_request, ..
    } = model.screen
    else {
        panic!("expected session model");
    };
    assert!(admission_request.is_none());
}

#[test]
fn manual_share_endpoint_is_projected_only_for_the_owner() {
    let raw_endpoint = "192.168.1.8:43120";
    let mut owner_ui = EditorUiState::default();
    owner_ui.collab.set_authenticated_session(
        CollabConnectionPhase::Active,
        AuthenticatedCollabSession {
            session_name: "Design".to_string(),
            role: CollabUiRole::Owner,
            share_endpoint: CollabShareEndpoint::new(raw_endpoint),
        },
        Vec::new(),
    );
    let owner_model = CollabPanelModel::for_editor_ui(&owner_ui);
    let CollabPanelScreen::Session { share_endpoint, .. } = &owner_model.screen else {
        panic!("expected owner session model");
    };
    assert_eq!(
        share_endpoint.as_ref().map(CollabShareEndpoint::as_str),
        Some(raw_endpoint)
    );
    assert!(!format!("{owner_model:?}").contains(raw_endpoint));

    for role in [CollabUiRole::Editor, CollabUiRole::Viewer] {
        let mut guest_ui = EditorUiState::default();
        guest_ui.collab.set_authenticated_session(
            CollabConnectionPhase::Active,
            AuthenticatedCollabSession {
                session_name: "Design".to_string(),
                role,
                share_endpoint: CollabShareEndpoint::new(raw_endpoint),
            },
            Vec::new(),
        );
        let guest_model = CollabPanelModel::for_editor_ui(&guest_ui);
        let CollabPanelScreen::Session { share_endpoint, .. } = guest_model.screen else {
            panic!("expected guest session model");
        };
        assert!(share_endpoint.is_none());
    }
}

#[test]
fn guest_confirmation_screen_preempts_progress_and_separates_the_claimed_name() {
    let mut ui = EditorUiState::default();
    ui.collab.availability = CollabAvailability::Ready;
    ui.collab.set_phase(CollabConnectionPhase::Authenticating);
    // Without a pending decision the join still reads as in progress.
    assert!(matches!(
        CollabPanelModel::for_editor_ui(&ui).screen,
        CollabPanelScreen::Progress { .. }
    ));

    let request_key = CollabAdmissionRequestKey::new("owner-confirm-42").unwrap();
    let identity = op_editor_core::CollabOwnerIdentityUi::from_verified(
        "11111111-1111-1111-1111-111111111111",
        "22222222-2222-2222-2222-222222222222",
        // A chosen name shaped to read like somebody else's account line: the
        // literal subject of another account, nothing else.
        Some("99999999-9999-9999-9999-999999999999"),
        None,
    )
    .unwrap();
    assert!(ui
        .collab
        .publish_owner_confirmation(request_key.clone(), identity));

    let model = CollabPanelModel::for_editor_ui(&ui);
    let CollabPanelScreen::ConfirmOwner(confirm) = model.screen else {
        panic!("a pending decision must preempt the progress screen");
    };
    assert_eq!(confirm.request_key, request_key);
    // The authoritative rows carry the verified identifiers only; the chosen
    // name never occupies one of them.
    assert_eq!(
        confirm
            .authoritative
            .iter()
            .map(|row| row.value.as_str())
            .collect::<Vec<_>>(),
        vec![
            "11111111-1111-1111-1111-111111111111",
            "22222222-2222-2222-2222-222222222222"
        ]
    );
    let claimed = confirm.claimed_name.clone().expect("claimed name row");
    assert_eq!(claimed.value, "99999999-9999-9999-9999-999999999999");
    // The forged value stays in the claim row and never becomes one of the
    // rows the decision is actually about.
    assert!(!confirm
        .authoritative
        .iter()
        .any(|row| row.value == claimed.value));
    // Its label says whose claim it is, so it cannot read as verification.
    assert_ne!(claimed.label, confirm.authoritative[0].label);

    // Exactly two decisions, and the model is redacted in debug output.
    assert_eq!(
        model
            .actions
            .iter()
            .map(|action| action.action.clone())
            .collect::<Vec<_>>(),
        vec![
            CollabUiAction::ConfirmOwnerIdentity {
                request_key: request_key.clone()
            },
            CollabUiAction::RejectOwnerIdentity { request_key }
        ]
    );
    let debug = format!("{confirm:?}");
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains("11111111-1111-1111-1111-111111111111"));
}

#[test]
fn an_active_session_can_never_project_the_guest_confirmation_screen() {
    let mut ui = EditorUiState::default();
    ui.collab.availability = CollabAvailability::Ready;
    ui.collab.set_phase(CollabConnectionPhase::Authenticating);
    let identity = op_editor_core::CollabOwnerIdentityUi::from_verified(
        "11111111-1111-1111-1111-111111111111",
        "22222222-2222-2222-2222-222222222222",
        None,
        None,
    )
    .unwrap();
    assert!(ui.collab.publish_owner_confirmation(
        CollabAdmissionRequestKey::new("owner-confirm-43").unwrap(),
        identity
    ));

    ui.collab.set_authenticated_session(
        CollabConnectionPhase::Active,
        AuthenticatedCollabSession {
            session_name: "Design".to_string(),
            role: CollabUiRole::Editor,
            share_endpoint: None,
        },
        Vec::new(),
    );
    assert!(matches!(
        CollabPanelModel::for_editor_ui(&ui).screen,
        CollabPanelScreen::Session { .. }
    ));
}
