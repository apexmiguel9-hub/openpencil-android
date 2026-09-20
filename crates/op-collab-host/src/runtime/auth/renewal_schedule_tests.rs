use super::*;

#[test]
fn short_ticket_renews_at_half_life() {
    let start = Instant::now();
    let (due, _) = renewal_schedule(1_000, 1_100, start).expect("renewal deadline");
    assert_eq!(due.duration_since(start), Duration::from_millis(50));
}

#[test]
fn local_auth_shutdown_reserves_up_to_the_terminal_flush_window() {
    let start = Instant::now();
    let expires_at = start + Duration::from_secs(30);
    assert_eq!(
        expires_at.duration_since(terminal_shutdown_at(start, expires_at)),
        TERMINAL_FLUSH_TIMEOUT
    );
    assert_eq!(
        terminal_shutdown_at(start, start + Duration::from_millis(100)),
        start + Duration::from_millis(75)
    );
}

#[test]
fn five_ten_and_forty_second_tickets_poll_for_renewal_before_shutdown() {
    let start = Instant::now();
    for ttl in [5_u64, 10, 40] {
        let ttl_ms = ttl * 1_000;
        let (renew_at, expires_at) = renewal_schedule(10_000, 10_000 + ttl_ms, start).unwrap();
        let shutdown_at = terminal_shutdown_at(start, expires_at);
        assert_eq!(
            renew_at.duration_since(start),
            Duration::from_millis(ttl_ms / 2),
            "{ttl}-second ticket renews at half-life"
        );
        assert!(
            renew_at < shutdown_at,
            "{ttl}-second ticket must have a live renewal window"
        );
        assert!(
            matches!(
                renewal_due_before_shutdown(renew_at, renew_at, shutdown_at),
                Ok(true)
            ),
            "{ttl}-second ticket poll starts renewal instead of shutting down"
        );
        assert_eq!(
            renewal_due_before_shutdown(shutdown_at, renew_at, shutdown_at)
                .unwrap_err()
                .failure,
            CollabRuntimeFailure::TicketRejected
        );
    }
}

#[test]
fn fast_provider_result_extends_a_five_second_ticket_before_shutdown() {
    let now_unix_ms = unix_time_ms().unwrap();
    let start = Instant::now();
    let current = VerifiedAuthMetadata {
        issuer: op_auth_bridge::PRODUCTION_COLLAB_ISSUER.to_string(),
        subject: "account-a".to_string(),
        device_id: "device-a".to_string(),
        proof_binding: "binding-a".to_string(),
        expires_at_unix_ms: now_unix_ms + 5_000,
        display_name: None,
        avatar_url: None,
    };
    let renewed = VerifiedAuthMetadata {
        expires_at_unix_ms: now_unix_ms + 60_000,
        ..current.clone()
    };
    let (renew_at, expires_at) =
        renewal_schedule(now_unix_ms, current.expires_at_unix_ms, start).unwrap();
    let shutdown_at = terminal_shutdown_at(start, expires_at);
    let admission = LocalAdmission {
        ticket: OpaqueCollabTicket::new(b"header.payload.signature".to_vec()).unwrap(),
        relay_bearer: super::RelayBearerCredentialSource::MinimizedRelayToken(
            op_auth_bridge::OpaqueCollabRelayToken::new(b"relay.token.signature".to_vec()).unwrap(),
        ),
        auth: renewed.clone(),
    };
    let (sender, receiver) = mpsc::channel();
    assert!(sender.send(Ok(admission)).is_ok());
    drop(sender);
    let config = CollabVerifierConfig::for_sso_origin("https://auth.self-hosted.example").unwrap();
    let mut renewer = LocalTicketRenewer {
        local_static: [0x42; 32],
        verifier: Arc::new(ProductionTicketVerifier::new_with_config(config).unwrap()),
        binding: current,
        renew_at,
        shutdown_at,
        expires_at,
        retry_backoff: INITIAL_RENEWAL_RETRY_BACKOFF,
        pending: Some(PendingRenewal {
            receiver: Some(receiver),
            cancelled: Arc::new(AtomicBool::new(false)),
            worker: None,
        }),
    };

    let applied = renewer
        .poll(renew_at)
        .expect("fast renewal remains inside the short-ticket window")
        .expect("completed provider result is applied");
    assert_eq!(applied.auth(), &renewed);
    assert_eq!(renewer.binding, renewed);
    assert!(renewer.expires_at > expires_at);
    assert!(renewer.shutdown_at > shutdown_at);
}
