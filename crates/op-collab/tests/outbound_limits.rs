use jian_ops_schema::PenDocument;
use op_collab::{
    canonical_document_hash, guest_to_owner_envelope_limit, AdmissionGrant, ClientOpId,
    CollabMessage, CommitSeq, ConnectionKey, ConnectionPrincipal, Epoch, FrameEnvelope,
    InboundFrameDirection, OwnerSessionConfig, OwnerSessionCore, ParticipantId, PeerId,
    PeerNamespace, Presence, ProtocolError, Role, SessionError, SessionId, Snapshot, UndoRequest,
    UndoRequestId, VerifiedAuthMetadata, WireLimits, MAX_ENVELOPE_BYTES,
    MAX_GUEST_TO_OWNER_ENVELOPE_BYTES, MAX_TXN_BYTES,
};

fn connection(raw: u64) -> ConnectionKey {
    ConnectionKey::new(raw).unwrap()
}

fn repeated_id(byte: char) -> String {
    byte.to_string().repeat(1_024)
}

fn grant(role: Role, participant: char, peer: char, namespace: &str) -> AdmissionGrant {
    AdmissionGrant::new(
        ConnectionPrincipal::from_verified(
            VerifiedAuthMetadata {
                issuer: "test-issuer".into(),
                subject: format!("subject-{peer}"),
                device_id: format!("device-{peer}"),
                proof_binding: format!("binding-{peer}"),
                expires_at_unix_ms: 10_000,
                display_name: None,
                avatar_url: None,
            },
            ParticipantId::from(repeated_id(participant)),
            PeerId::from(repeated_id(peer)),
            role,
        ),
        PeerNamespace::try_from(namespace).unwrap(),
    )
}

fn large_presence() -> Presence {
    Presence {
        cursor: None,
        selection: vec!["s".repeat(1_000); 20],
        viewport: None,
        editing_node: None,
    }
}

fn short_grant(role: Role, participant: &str, peer: &str, namespace: &str) -> AdmissionGrant {
    AdmissionGrant::new(
        ConnectionPrincipal::from_verified(
            VerifiedAuthMetadata {
                issuer: "test-issuer".into(),
                subject: format!("subject-{peer}"),
                device_id: format!("device-{peer}"),
                proof_binding: format!("binding-{peer}"),
                expires_at_unix_ms: 10_000,
                display_name: None,
                avatar_url: None,
            },
            ParticipantId::from(participant),
            PeerId::from(peer),
            role,
        ),
        PeerNamespace::try_from(namespace).unwrap(),
    )
}

#[test]
fn guest_inbound_ceiling_is_derived_from_the_per_message_caps() {
    assert_eq!(
        guest_to_owner_envelope_limit(WireLimits::default()),
        MAX_GUEST_TO_OWNER_ENVELOPE_BYTES as usize
    );
    const {
        assert!(MAX_GUEST_TO_OWNER_ENVELOPE_BYTES > MAX_TXN_BYTES);
        assert!(MAX_GUEST_TO_OWNER_ENVELOPE_BYTES < MAX_ENVELOPE_BYTES);
    }
    let tight = WireLimits {
        max_envelope_bytes: 4_096,
        ..WireLimits::default()
    };
    assert_eq!(guest_to_owner_envelope_limit(tight), 4_096);
}

#[test]
fn oversized_guest_frame_is_rejected_before_the_generic_decode() {
    let padding = "a".repeat(MAX_GUEST_TO_OWNER_ENVELOPE_BYTES as usize);
    let bytes = serde_json::to_vec(&serde_json::json!({
        "protocolVersion": 1,
        "sessionId": "session",
        "epoch": 1,
        "body": {
            "type": "submit",
            "payload": {
                "clientOpId": {"peerId": padding, "localCounter": 1},
                "baseSeq": 0,
                "txn": {"ops": []},
            },
        },
    }))
    .unwrap();
    assert!(bytes.len() > guest_to_owner_envelope_limit(WireLimits::default()));
    assert!(bytes.len() <= MAX_ENVELOPE_BYTES as usize);

    assert!(matches!(
        FrameEnvelope::from_json_slice(&bytes),
        Err(ProtocolError::GuestEnvelopeTooLarge { .. })
    ));
}

#[test]
fn oversized_snapshot_kind_cannot_raise_the_owner_inbound_ceiling() {
    let content = "a".repeat(MAX_GUEST_TO_OWNER_ENVELOPE_BYTES as usize);
    let document: PenDocument = serde_json::from_value(serde_json::json!({
        "version": "1.0",
        "children": [{"type": "text", "id": "c_ns_1", "content": content}],
    }))
    .unwrap();
    let snapshot = FrameEnvelope::new(
        SessionId::from("session"),
        Epoch(1),
        CollabMessage::Snapshot(Box::new(Snapshot {
            seq: CommitSeq(0),
            doc_hash: canonical_document_hash(&document).unwrap(),
            document,
        })),
    );

    let encoded = snapshot.to_json_vec().unwrap();
    assert!(encoded.len() > guest_to_owner_envelope_limit(WireLimits::default()));
    assert_eq!(
        FrameEnvelope::from_json_slice_with_limits_for_direction(
            &encoded,
            WireLimits::default(),
            InboundFrameDirection::OwnerToGuest,
        )
        .unwrap(),
        snapshot
    );

    assert!(matches!(
        FrameEnvelope::from_json_slice_with_limits_for_direction(
            &encoded,
            WireLimits::default(),
            InboundFrameDirection::GuestToOwner,
        ),
        Err(ProtocolError::GuestEnvelopeTooLarge { .. })
    ));
}

#[test]
fn presence_payload_limit_applies_to_encode_and_decode() {
    let frame = FrameEnvelope::new(
        SessionId::from("session"),
        Epoch(1),
        CollabMessage::PresenceUpdate(large_presence()),
    );
    let encoded = frame.to_json_vec().unwrap();
    let limits = WireLimits {
        max_presence_bytes: 32,
        ..WireLimits::default()
    };

    assert!(matches!(
        frame.to_json_vec_with_limits(limits),
        Err(ProtocolError::PresenceTooLarge { .. })
    ));
    assert!(matches!(
        FrameEnvelope::from_json_slice_with_limits(&encoded, limits),
        Err(ProtocolError::PresenceTooLarge { .. })
    ));
}

#[test]
fn identity_injection_cannot_emit_an_oversized_presence_broadcast() {
    let document: PenDocument = serde_json::from_str(r#"{"version":"1.0","children":[]}"#).unwrap();
    let inbound = FrameEnvelope::new(
        SessionId::from("s"),
        Epoch(1),
        CollabMessage::PresenceUpdate(large_presence()),
    );
    let inbound_size = inbound.to_json_vec().unwrap().len();
    let mut config = OwnerSessionConfig::default();
    config.wire_limits.max_envelope_bytes = u32::try_from(inbound_size).unwrap();
    let mut core = OwnerSessionCore::new(
        SessionId::from("s"),
        Epoch(1),
        CommitSeq(0),
        connection(1),
        grant(Role::Owner, 'a', 'o', "owner"),
        &document,
        config,
    )
    .unwrap();
    core.activate_peer(
        connection(2),
        grant(Role::Editor, 'b', 'e', "editor"),
        &document,
    )
    .unwrap();

    assert!(matches!(
        core.accept_frame(connection(2), inbound, &document),
        Err(SessionError::InvalidFrame(
            ProtocolError::EnvelopeTooLarge { .. }
        ))
    ));
}

#[test]
fn viewer_undo_rejection_is_checked_after_adding_owner_fields() {
    let document: PenDocument = serde_json::from_str(r#"{"version":"1.0","children":[]}"#).unwrap();
    let inbound = FrameEnvelope::new(
        SessionId::from("s"),
        Epoch(1),
        CollabMessage::UndoRequest(UndoRequest {
            request_id: UndoRequestId {
                peer_id: PeerId::from("viewer"),
                local_counter: 1,
            },
            target_client_op_id: ClientOpId {
                peer_id: PeerId::from("t".repeat(1_024)),
                local_counter: 1,
            },
        }),
    );
    let inbound_size = inbound.to_json_vec().unwrap().len();
    let mut config = OwnerSessionConfig::default();
    config.wire_limits.max_envelope_bytes = u32::try_from(inbound_size).unwrap();
    let mut core = OwnerSessionCore::new(
        SessionId::from("s"),
        Epoch(1),
        CommitSeq(0),
        connection(1),
        short_grant(Role::Owner, "owner-participant", "owner", "owner"),
        &document,
        config,
    )
    .unwrap();
    core.activate_peer(
        connection(2),
        short_grant(Role::Viewer, "viewer-participant", "viewer", "viewer"),
        &document,
    )
    .unwrap();

    assert!(matches!(
        core.accept_frame(connection(2), inbound, &document),
        Err(SessionError::InvalidFrame(
            ProtocolError::EnvelopeTooLarge { .. }
        ))
    ));
}
