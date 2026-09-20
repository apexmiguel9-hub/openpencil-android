use super::*;
use crate::{
    CollabAvailability, CollabConnectionPathUi, CollabConnectionPhase, CollabNoticeKind,
    CollabParticipantUi, CollabRejectUiCode, CollabRelayRegion, CollabUiRole, RemotePresenceUi,
};

fn owner_session_state() -> CollabUiState {
    let mut ui = CollabUiState::default();
    ui.set_availability(CollabAvailability::Ready);
    ui.set_authenticated_session(
        CollabConnectionPhase::Active,
        crate::AuthenticatedCollabSession {
            session_name: "studio".into(),
            role: CollabUiRole::Owner,
            share_endpoint: CollabShareEndpoint::new("192.168.1.7:43120"),
        },
        vec![CollabParticipantUi::new(
            "peer-1",
            "Ada Lovelace",
            0x33aaffff,
            CollabUiRole::Owner,
            true,
        )],
    );
    ui.set_public_session(
        CollabInviteCode::new("ABCDEFGHJK"),
        CollabConnectionPathUi::Relay {
            home_region: CollabRelayRegion::Global,
        },
    );
    ui
}

#[test]
fn owner_projection_round_trips_through_json_into_an_equal_projection() {
    let ui = owner_session_state();
    let wire = CollabStateWire::from_ui(&ui, 7, 42);

    let json = serde_json::to_string(&wire).expect("encodes");
    let decoded: CollabStateWire = serde_json::from_str(&json).expect("decodes");
    assert_eq!(decoded, wire);

    let mut client = CollabUiState::default();
    decoded.apply_to(&mut client, 1_000);
    let reprojected = CollabStateWire::from_ui(&client, 7, 42);
    assert_eq!(reprojected, wire);
}

#[test]
fn projection_carries_the_wire_version_and_both_sequence_numbers() {
    let wire = CollabStateWire::from_ui(&owner_session_state(), 9, 300);
    assert_eq!(wire.wire_version, COLLAB_WIRE_VERSION);
    assert_eq!(wire.collab_seq, 9);
    assert_eq!(wire.document_revision, 300);
}

#[test]
fn unknown_fields_are_ignored_so_an_older_client_survives_an_added_field() {
    let wire = CollabStateWire::from_ui(&owner_session_state(), 1, 1);
    let mut value = serde_json::to_value(&wire).expect("encodes");
    value
        .as_object_mut()
        .expect("object")
        .insert("somethingAddedLater".into(), serde_json::json!({"a": 1}));
    let decoded: CollabStateWire = serde_json::from_value(value).expect("tolerates new fields");
    assert_eq!(decoded, wire);
}

#[test]
fn an_idle_projection_carries_no_session_participants_or_presence() {
    let mut ui = CollabUiState::default();
    ui.set_availability(CollabAvailability::SignInRequired);
    let wire = CollabStateWire::from_ui(&ui, 0, 0);

    assert_eq!(wire.phase, CollabPhaseWire::Idle);
    assert_eq!(wire.availability, CollabAvailabilityWire::SignInRequired);
    assert!(wire.session.is_none());
    assert!(wire.participants.is_empty());
    assert!(wire.presence.is_empty());
    assert!(wire.admissions.is_empty());
}

#[test]
fn applying_a_session_projection_installs_role_invite_and_share_endpoint() {
    let wire = CollabStateWire::from_ui(&owner_session_state(), 3, 3);
    let mut client = CollabUiState::default();
    wire.apply_to(&mut client, 500);

    let session = client
        .authenticated_session()
        .expect("session installed on an authenticated phase");
    assert_eq!(session.session_name, "studio");
    assert_eq!(session.role, CollabUiRole::Owner);
    assert_eq!(
        session
            .share_endpoint
            .as_ref()
            .map(CollabShareEndpoint::as_str),
        Some("192.168.1.7:43120")
    );
    let public = client.public_session().expect("public session installed");
    assert_eq!(
        public.invite().map(CollabInviteCode::as_str),
        Some("ABCDEFGHJK")
    );
    assert_eq!(
        public.connection(),
        Some(CollabConnectionPathUi::Relay {
            home_region: CollabRelayRegion::Global
        })
    );
}

#[test]
fn presence_is_installed_only_for_peers_in_the_roster() {
    let mut ui = owner_session_state();
    ui.queue_presence_snapshot(vec![
        RemotePresenceUi::bounded(
            "peer-1",
            Some(crate::CollabCanvasPoint { x: 12.0, y: 34.0 }),
            ["node-a".to_string()],
            None,
            10,
        ),
        RemotePresenceUi::bounded("ghost", None, [], None, 11),
    ]);
    ui.flush_presence(100);

    let wire = CollabStateWire::from_ui(&ui, 1, 1);
    let keys: Vec<_> = wire
        .presence
        .iter()
        .map(|p| p.participant_key.as_str())
        .collect();
    assert_eq!(keys, ["peer-1"], "a non-roster peer must not be projected");

    let mut client = CollabUiState::default();
    wire.apply_to(&mut client, 1_000);
    assert_eq!(client.presence().len(), 1);
    assert_eq!(client.presence()[0].participant_key, "peer-1");
    assert_eq!(
        client.presence()[0].cursor,
        Some(crate::CollabCanvasPoint { x: 12.0, y: 34.0 })
    );
}

#[test]
fn admission_keys_survive_the_round_trip_as_opaque_strings() {
    let mut ui = owner_session_state();
    let key = CollabAdmissionRequestKey::new("req-abc_123").expect("valid key");
    assert!(ui.publish_pending_admission(key.clone(), Some(CollabUiRole::Editor)));

    let wire = CollabStateWire::from_ui(&ui, 1, 1);
    assert_eq!(wire.admissions.len(), 1);
    assert_eq!(wire.admissions[0].request_key, "req-abc_123");

    let mut client = CollabUiState::default();
    wire.apply_to(&mut client, 1_000);
    assert_eq!(client.pending_admissions().len(), 1);
    assert_eq!(client.pending_admissions()[0].request_key(), &key);
}

#[test]
fn a_malformed_admission_key_is_dropped_rather_than_installed() {
    let mut wire = CollabStateWire::from_ui(&owner_session_state(), 1, 1);
    wire.admissions.push(CollabAdmissionWire {
        request_key: "not a key".into(),
        resume_role: None,
    });

    let mut client = CollabUiState::default();
    wire.apply_to(&mut client, 1_000);
    assert!(client.pending_admissions().is_empty());
}

#[test]
fn notices_project_their_payload_and_timestamp() {
    let mut ui = owner_session_state();
    ui.set_notice(
        CollabNoticeKind::Reject(CollabRejectUiCode::StaleBase),
        4_242,
    );
    let wire = CollabStateWire::from_ui(&ui, 1, 1);
    let notice = wire.notice.clone().expect("notice projected");
    assert_eq!(notice.created_at_ms, 4_242);
    assert_eq!(
        notice.kind,
        CollabNoticeKindWire::Reject {
            code: CollabRejectCodeWire::StaleBase
        }
    );
}

#[test]
fn the_panel_projection_omits_local_only_ui() {
    let wire = CollabStateWire::from_ui(&owner_session_state(), 1, 1);
    let json = serde_json::to_value(&wire.panel).expect("encodes");
    let panel = json.as_object().expect("object");
    // Panel open/view/hover and the join-address draft belong to whichever
    // client is painting; echoing them would fight the user's own typing.
    for local_only in ["open", "view", "hover", "joinAddress", "joinInput"] {
        assert!(
            !panel.contains_key(local_only),
            "{local_only} must stay client-local"
        );
    }
    assert!(panel.contains_key("relayRegion"));
    assert!(panel.contains_key("discovered"));
}

#[test]
fn local_presence_wire_defaults_its_reserved_client_id() {
    let decoded: CollabLocalPresenceWire =
        serde_json::from_str(r#"{"cursor":{"x":1.5,"y":2.5}}"#).expect("decodes");
    assert_eq!(decoded.client_id, None);
    assert_eq!(decoded.cursor, Some(CollabPointWire { x: 1.5, y: 2.5 }));
    // Absent rather than null, so the field can gain meaning without a bump.
    let json = serde_json::to_string(&decoded).expect("encodes");
    assert!(!json.contains("clientId"), "{json}");
}

#[test]
fn the_version_probe_yields_the_collaboration_sequence_separately() {
    assert_eq!(
        super::parse_collab_seq_probe(r#"{"version":7,"collabSeq":42}"#),
        Some(42)
    );
}

#[test]
fn a_daemon_without_the_collaboration_field_reads_as_absent() {
    // An older daemon answers only `{"version":N}`; collaboration then simply
    // stays idle instead of the client mis-reading the document version as a
    // projection sequence.
    assert_eq!(super::parse_collab_seq_probe(r#"{"version":7}"#), None);
    assert_eq!(super::parse_collab_seq_probe("not json"), None);
}
