use jian_ops_schema::PenDocument;
use op_collab::{
    AdmissionGrant, CollabMessage, CommitSeq, ConnectionKey, ConnectionPrincipal, Epoch,
    FrameEnvelope, OwnerSessionConfig, OwnerSessionCore, Participant, ParticipantId, PeerId,
    PeerNamespace, ProtocolError, Role, SessionError, SessionId, VerifiedAuthMetadata,
    MAX_COLLAB_PROFILE_AVATAR_URL_BYTES, MAX_COLLAB_PROFILE_DISPLAY_NAME_BYTES,
    MAX_COLLAB_PROFILE_DISPLAY_NAME_CHARS,
};

const SESSION: &str = "profile-session";
const EPOCH: u64 = 5;

fn connection(value: u64) -> ConnectionKey {
    ConnectionKey::new(value).unwrap()
}

fn document() -> PenDocument {
    serde_json::from_str(r#"{"version":"1.0","children":[]}"#).unwrap()
}

fn auth(
    peer: &str,
    expiry: u64,
    display_name: Option<&str>,
    avatar_url: Option<&str>,
) -> VerifiedAuthMetadata {
    VerifiedAuthMetadata {
        issuer: "https://issuer.example".into(),
        subject: format!("subject-{peer}"),
        device_id: format!("device-{peer}"),
        proof_binding: format!("proof-{peer}"),
        expires_at_unix_ms: expiry,
        display_name: display_name.map(str::to_owned),
        avatar_url: avatar_url.map(str::to_owned),
    }
}

fn principal(
    peer: &str,
    role: Role,
    expiry: u64,
    display_name: Option<&str>,
    avatar_url: Option<&str>,
) -> ConnectionPrincipal {
    ConnectionPrincipal::from_verified(
        auth(peer, expiry, display_name, avatar_url),
        ParticipantId::from(format!("participant-{peer}")),
        PeerId::from(peer),
        role,
    )
}

fn grant(
    peer: &str,
    role: Role,
    expiry: u64,
    display_name: Option<&str>,
    avatar_url: Option<&str>,
) -> AdmissionGrant {
    AdmissionGrant::new(
        principal(peer, role, expiry, display_name, avatar_url),
        PeerNamespace::try_from(format!("{peer}-ns")).unwrap(),
    )
}

fn frame(participant: Participant) -> FrameEnvelope {
    FrameEnvelope::new(
        SessionId::from(SESSION),
        Epoch(EPOCH),
        CollabMessage::ParticipantJoined(participant),
    )
}

#[test]
fn optional_profile_fields_round_trip_without_identity_leaks() {
    let principal = principal(
        "guest",
        Role::Editor,
        100,
        Some("Kay 沈"),
        Some("https://profiles.example/avatar.png?size=80&sig=public"),
    );
    assert_eq!(principal.display_name(), Some("Kay 沈"));
    assert_eq!(
        principal.avatar_url(),
        Some("https://profiles.example/avatar.png?size=80&sig=public")
    );

    let participant = principal.roster_participant();
    let encoded = frame(participant.clone()).to_json_vec().unwrap();
    let json: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(json["body"]["payload"]["displayName"], "Kay 沈");
    assert_eq!(
        json["body"]["payload"]["avatarUrl"],
        "https://profiles.example/avatar.png?size=80&sig=public"
    );
    let text = String::from_utf8(encoded.clone()).unwrap();
    assert!(!text.contains("subject-guest"));
    assert!(!text.contains("device-guest"));
    assert_eq!(
        FrameEnvelope::from_json_slice(&encoded).unwrap(),
        frame(participant.clone())
    );

    let mut legacy = json;
    let payload = legacy["body"]["payload"].as_object_mut().unwrap();
    payload.remove("displayName");
    payload.remove("avatarUrl");
    let decoded = FrameEnvelope::from_json_slice(&serde_json::to_vec(&legacy).unwrap()).unwrap();
    let CollabMessage::ParticipantJoined(decoded) = decoded.into_body() else {
        panic!("fixture remains a participant_joined frame");
    };
    assert_eq!(decoded.display_name, None);
    assert_eq!(decoded.avatar_url, None);

    let absent = frame(Participant {
        participant_id: ParticipantId::from("participant-anonymous"),
        peer_id: PeerId::from("anonymous"),
        role: Role::Viewer,
        display_name: None,
        avatar_url: None,
    });
    let absent: serde_json::Value = serde_json::from_slice(&absent.to_json_vec().unwrap()).unwrap();
    let payload = absent["body"]["payload"].as_object().unwrap();
    assert!(!payload.contains_key("displayName"));
    assert!(!payload.contains_key("avatarUrl"));

    let principal_debug = format!("{principal:?}");
    let participant_debug = format!("{participant:?}");
    for secret in [
        "subject-guest",
        "device-guest",
        "proof-guest",
        "Kay 沈",
        "profiles.example",
    ] {
        assert!(!principal_debug.contains(secret));
        assert!(!participant_debug.contains(secret));
    }
}

#[test]
fn participant_wire_rejects_unverified_identity_fields() {
    let participant = principal("guest", Role::Editor, 100, None, None).roster_participant();
    let mut json: serde_json::Value =
        serde_json::from_slice(&frame(participant).to_json_vec().unwrap()).unwrap();
    json["body"]["payload"]["subject"] = serde_json::json!("must-not-cross-wire");
    assert!(matches!(
        FrameEnvelope::from_json_slice(&serde_json::to_vec(&json).unwrap()),
        Err(ProtocolError::Decode(_))
    ));
}

#[test]
fn fixed_profile_bounds_and_https_rules_apply_on_encode_and_decode() {
    assert_eq!(MAX_COLLAB_PROFILE_DISPLAY_NAME_BYTES, 320);
    assert_eq!(MAX_COLLAB_PROFILE_DISPLAY_NAME_CHARS, 80);
    assert_eq!(MAX_COLLAB_PROFILE_AVATAR_URL_BYTES, 2_048);

    for invalid in [
        String::new(),
        " leading".into(),
        "trailing ".into(),
        "line\nbreak".into(),
        "界".repeat(MAX_COLLAB_PROFILE_DISPLAY_NAME_CHARS + 1),
        "a".repeat(MAX_COLLAB_PROFILE_DISPLAY_NAME_BYTES + 1),
    ] {
        assert_invalid_profile(Some(invalid), None, "participant.display_name", true);
    }
    for invalid in [
        String::new(),
        "http://profiles.example/avatar.png".into(),
        "https://user@profiles.example/avatar.png".into(),
        "https://profiles.example/avatar.png#fragment".into(),
        "https://profiles.example/a vatar.png".into(),
        "https://profiles.example:0/avatar.png".into(),
        "https://profiles.example/头像.png".into(),
        format!(
            "https://profiles.example/{}",
            "a".repeat(MAX_COLLAB_PROFILE_AVATAR_URL_BYTES)
        ),
    ] {
        assert_invalid_profile(None, Some(invalid), "participant.avatar_url", true);
    }

    let valid = frame(Participant {
        participant_id: ParticipantId::from("participant-valid"),
        peer_id: PeerId::from("valid"),
        role: Role::Editor,
        display_name: Some("Kay 沈".into()),
        avatar_url: Some("https://[2606:4700:4700::1111]:8443/avatar.png?size=80".into()),
    });
    assert!(valid.to_json_vec().is_ok());
}

#[test]
fn invisible_and_bidirectional_display_names_are_rejected() {
    for spoofed in [
        // Renders identically to an existing roster entry named "alice".
        "alice\u{200b}",
        "ali\u{200d}ce",
        "alice\u{feff}",
        "alice\u{00ad}bob",
        "alice\u{2060}",
        // Reorders the glyphs shown to every other participant.
        "\u{200e}alice",
        "\u{200f}alice",
        "\u{202e}alice",
        "\u{2066}alice\u{2069}",
        // Other non-graphic code points implied by the same contract.
        "alice\u{2028}bob",
        "alice\u{061c}",
        "alice\u{e0041}",
    ] {
        assert_invalid_profile(Some(spoofed.into()), None, "participant.display_name", true);
    }
}

#[test]
fn legitimate_unicode_display_names_still_pass() {
    for accepted in [
        "Kay 沈",
        "沈凯 (设计)",
        "Renée Dupont",
        "김민준",
        "Иван Петров",
        "أحمد",
        "Ada 😀",
        "नमस्ते",
    ] {
        let valid = frame(Participant {
            participant_id: ParticipantId::from("participant-valid"),
            peer_id: PeerId::from("valid"),
            role: Role::Editor,
            display_name: Some(accepted.into()),
            avatar_url: None,
        });
        let encoded = valid
            .to_json_vec()
            .unwrap_or_else(|error| panic!("`{accepted}` must remain a valid name: {error}"));
        assert_eq!(FrameEnvelope::from_json_slice(&encoded).unwrap(), valid);
    }
}

#[test]
fn avatar_urls_naming_non_public_addresses_are_rejected() {
    for invalid in [
        // Cloud instance metadata, and the private/loopback blocks.
        "https://169.254.169.254/latest/meta-data/",
        "https://10.0.0.1/avatar.png",
        "https://172.16.0.1/avatar.png",
        "https://192.168.1.1/avatar.png",
        "https://127.0.0.1:8443/avatar.png",
        "https://0.0.0.0/avatar.png",
        "https://100.64.0.1/avatar.png",
        "https://255.255.255.255/avatar.png",
        "https://239.0.0.1/avatar.png",
        // The IPv6 equivalents.
        "https://[::1]/avatar.png",
        "https://[::]/avatar.png",
        "https://[fe80::1]/avatar.png",
        "https://[fd00::1]/avatar.png",
        "https://[ff02::1]/avatar.png",
        // IPv4-mapped, IPv4-compatible, and NAT64-embedded aliases.
        "https://[::ffff:169.254.169.254]/avatar.png",
        "https://[::ffff:10.0.0.1]:8443/avatar.png",
        "https://[::10.0.0.1]/avatar.png",
        "https://[64:ff9b::a00:1]/avatar.png",
    ] {
        assert_invalid_profile(None, Some(invalid.into()), "participant.avatar_url", true);
    }
}

#[test]
fn public_ip_literal_and_dns_avatar_urls_still_pass() {
    for accepted in [
        "https://1.1.1.1/avatar.png",
        "https://93.184.216.34:8443/avatar.png?size=80",
        "https://[2606:4700:4700::1111]/avatar.png",
        "https://[64:ff9b::101:101]/avatar.png",
        "https://profiles.example/avatar.png",
        // A DNS name may still resolve to a private address; refusing that is
        // the fetch layer's job, so host names keep their existing behaviour.
        "https://internal.corp.example/avatar.png",
    ] {
        let valid = frame(Participant {
            participant_id: ParticipantId::from("participant-valid"),
            peer_id: PeerId::from("valid"),
            role: Role::Editor,
            display_name: None,
            avatar_url: Some(accepted.into()),
        });
        let encoded = valid
            .to_json_vec()
            .unwrap_or_else(|error| panic!("`{accepted}` must remain a valid avatar: {error}"));
        assert_eq!(FrameEnvelope::from_json_slice(&encoded).unwrap(), valid);
    }
}

#[test]
fn admission_and_resume_publish_latest_verified_profile() {
    let document = document();
    let mut owner = OwnerSessionCore::new(
        SessionId::from(SESSION),
        Epoch(EPOCH),
        CommitSeq(0),
        connection(1),
        grant(
            "owner",
            Role::Owner,
            100,
            Some("Owner"),
            Some("https://profiles.example/owner.png"),
        ),
        &document,
        OwnerSessionConfig::default(),
    )
    .unwrap();

    let activated = owner
        .activate_peer(
            connection(2),
            grant(
                "guest",
                Role::Editor,
                100,
                Some("Guest Old"),
                Some("https://profiles.example/guest-old.png"),
            ),
            &document,
        )
        .unwrap();
    assert_eq!(activated.joined.display_name.as_deref(), Some("Guest Old"));
    assert_eq!(
        activated.joined.avatar_url.as_deref(),
        Some("https://profiles.example/guest-old.png")
    );
    assert_eq!(
        activated
            .welcome
            .participants
            .iter()
            .find(|participant| participant.peer_id.as_ref() == "guest")
            .and_then(|participant| participant.display_name.as_deref()),
        Some("Guest Old")
    );

    owner.disconnect(connection(2)).unwrap();
    let resumed = owner
        .resume_peer(
            connection(3),
            grant(
                "guest",
                Role::Editor,
                200,
                Some("Guest New"),
                Some("https://profiles.example/guest-new.png"),
            ),
        )
        .unwrap();
    assert_eq!(resumed.joined.display_name.as_deref(), Some("Guest New"));
    assert_eq!(
        resumed
            .welcome
            .participants
            .iter()
            .find(|participant| participant.peer_id.as_ref() == "guest")
            .and_then(|participant| participant.display_name.as_deref()),
        Some("Guest New")
    );

    owner
        .complete_renewal(
            connection(3),
            auth(
                "guest",
                300,
                Some("Guest Renewed"),
                Some("https://profiles.example/guest-renewed.png"),
            ),
        )
        .unwrap();
    let renewed = owner
        .active_participants()
        .into_iter()
        .find(|participant| participant.peer_id.as_ref() == "guest")
        .unwrap();
    assert_eq!(renewed.display_name.as_deref(), Some("Guest New"));
    assert_eq!(
        renewed.avatar_url.as_deref(),
        Some("https://profiles.example/guest-new.png")
    );
}

#[test]
fn admission_and_renewal_reject_invalid_verified_profiles() {
    let document = document();
    let invalid_owner = OwnerSessionCore::new(
        SessionId::from(SESSION),
        Epoch(EPOCH),
        CommitSeq(0),
        connection(1),
        grant("owner", Role::Owner, 100, Some(" leading"), None),
        &document,
        OwnerSessionConfig::default(),
    );
    assert!(matches!(
        invalid_owner,
        Err(SessionError::InvalidAdmissionProfile {
            field: "display_name"
        })
    ));

    let mut owner = OwnerSessionCore::new(
        SessionId::from(SESSION),
        Epoch(EPOCH),
        CommitSeq(0),
        connection(1),
        grant("owner", Role::Owner, 100, Some("Owner"), None),
        &document,
        OwnerSessionConfig::default(),
    )
    .unwrap();
    owner
        .activate_peer(
            connection(2),
            grant("guest", Role::Editor, 100, Some("Guest"), None),
            &document,
        )
        .unwrap();
    assert!(matches!(
        owner.complete_renewal(
            connection(2),
            auth(
                "guest",
                200,
                Some("Guest"),
                Some("http://profiles.example/avatar.png"),
            ),
        ),
        Err(SessionError::InvalidAdmissionProfile {
            field: "avatar_url"
        })
    ));
    let guest = owner
        .active_participants()
        .into_iter()
        .find(|participant| participant.peer_id.as_ref() == "guest")
        .unwrap();
    assert_eq!(guest.display_name.as_deref(), Some("Guest"));
    assert_eq!(guest.avatar_url, None);
}

fn assert_invalid_profile(
    display_name: Option<String>,
    avatar_url: Option<String>,
    field: &'static str,
    check_decode: bool,
) {
    let participant = Participant {
        participant_id: ParticipantId::from("participant-invalid"),
        peer_id: PeerId::from("invalid"),
        role: Role::Editor,
        display_name,
        avatar_url,
    };
    let raw_payload = serde_json::to_value(&participant).unwrap();
    let invalid = frame(participant);
    assert!(matches!(
        invalid.to_json_vec(),
        Err(ProtocolError::InvalidParticipantProfile { field: actual }) if actual == field
    ));
    if check_decode {
        let raw = serde_json::to_vec(&serde_json::json!({
            "protocolVersion": 1,
            "sessionId": SESSION,
            "epoch": EPOCH,
            "body": {
                "type": "participant_joined",
                "payload": raw_payload,
            }
        }))
        .unwrap();
        assert!(matches!(
            FrameEnvelope::from_json_slice(&raw),
            Err(ProtocolError::InvalidParticipantProfile { field: actual }) if actual == field
        ));
    }
}
