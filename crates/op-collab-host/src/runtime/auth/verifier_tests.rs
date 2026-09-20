//! Verifier/renewal unit tests split out of `auth.rs` for the file cap.

use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

fn auth(subject: &str, expires_at_unix_ms: u64) -> VerifiedAuthMetadata {
    VerifiedAuthMetadata {
        issuer: op_auth_bridge::PRODUCTION_COLLAB_ISSUER.to_string(),
        subject: subject.to_string(),
        device_id: "device".to_string(),
        proof_binding: "binding".to_string(),
        expires_at_unix_ms,
        display_name: None,
        avatar_url: None,
    }
}

#[test]
fn verifier_uses_the_pinned_self_hosted_desktop_trust_root() {
    let config =
        CollabVerifierConfig::for_sso_origin("https://auth.self-hosted.example:9443").unwrap();
    let verifier = ProductionTicketVerifier::new_with_config(config).unwrap();
    assert_eq!(
        verifier.expected_issuer(),
        "https://auth.self-hosted.example:9443"
    );
    assert_eq!(
        verifier.inner.config().jwks_endpoint(),
        "https://auth.self-hosted.example:9443/api/v1/collab/jwks"
    );
}

#[test]
fn verifier_configuration_rejects_an_insecure_self_hosted_origin() {
    let error = CollabVerifierConfig::for_sso_origin("http://auth.self-hosted.example")
        .expect_err("HTTP trust roots must fail closed");
    assert_eq!(
        error,
        op_auth_bridge::CollabVerifierConfigError::InvalidIssuer
    );
}

#[test]
fn renewal_verification_error_classification_is_fail_closed() {
    let transient = [
        CollabVerifyError::Cancelled,
        CollabVerifyError::Jwks(CollabJwksError::InvalidBodySize { maximum: 1 }),
        CollabVerifyError::Jwks(CollabJwksError::MalformedJson),
        CollabVerifyError::Jwks(CollabJwksError::EmptyKeyset),
        CollabVerifyError::Jwks(CollabJwksError::TooManyKeys { maximum: 1 }),
        CollabVerifyError::Jwks(CollabJwksError::DuplicateKeyId),
        CollabVerifyError::Jwks(CollabJwksError::InvalidKey {
            index: 0,
            kind: op_auth_bridge::CollabJwkErrorKind::WrongAlgorithm,
        }),
        CollabVerifyError::Jwks(CollabJwksError::InvalidEtag { maximum: 1 }),
        CollabVerifyError::Jwks(CollabJwksError::RefreshThrottled),
        CollabVerifyError::Jwks(CollabJwksError::CacheUnavailable),
        CollabVerifyError::Jwks(CollabJwksError::NotModifiedWithoutCache),
        CollabVerifyError::Jwks(CollabJwksError::Policy(
            op_auth_bridge::CollabUnionPolicyError::InvalidSignature,
        )),
        CollabVerifyError::Jwks(CollabJwksError::Fetch(CollabJwksFetchError::Cancelled)),
        CollabVerifyError::Jwks(CollabJwksError::Fetch(CollabJwksFetchError::Unavailable)),
        CollabVerifyError::Jwks(CollabJwksError::Fetch(
            CollabJwksFetchError::RejectedResponse,
        )),
        CollabVerifyError::Jwks(CollabJwksError::Fetch(
            CollabJwksFetchError::ResponseTooLarge,
        )),
    ];
    for error in transient {
        assert_eq!(
            verification_failure(&error),
            CollabRuntimeFailure::AuthenticationUnavailable,
            "{error:?}"
        );
    }

    let rejected = [
        CollabVerifyError::MalformedCompactJws,
        CollabVerifyError::MalformedJson { part: "claims" },
        CollabVerifyError::InvalidSignature,
        CollabVerifyError::InvalidChannelBinding,
        CollabVerifyError::ChannelBindingMismatch,
        CollabVerifyError::Jwks(CollabJwksError::UnknownKey),
    ];
    for error in rejected {
        assert_eq!(
            verification_failure(&error),
            CollabRuntimeFailure::TicketRejected,
            "{error:?}"
        );
    }

    assert_eq!(
        verification_runtime_error(&CollabVerifyError::InvalidSignature, true).failure,
        CollabRuntimeFailure::AuthenticationUnavailable,
        "session retirement cancellation must suppress publication"
    );
}

#[test]
fn renewal_must_extend_the_same_noise_bound_principal() {
    let current = auth("account-a", 1_100);
    assert!(validate_renewed_binding(&current, &auth("account-a", 1_200)).is_ok());
    assert!(validate_renewed_binding(&current, &auth("account-b", 1_200)).is_err());
    assert!(validate_renewed_binding(&current, &auth("account-a", 1_100)).is_err());
}

#[test]
fn unavailable_jwks_renewal_backs_off_while_the_old_ticket_remains_valid() {
    let start = Instant::now();
    let expires_at = start + Duration::from_secs(180);
    let shutdown_at = terminal_shutdown_at(start, expires_at);
    let binding = auth("account-a", 180_000);
    let (sender, receiver) = mpsc::channel();
    let unavailable =
        CollabVerifyError::Jwks(CollabJwksError::Fetch(CollabJwksFetchError::Unavailable));
    sender
        .send(Err(verification_runtime_error(&unavailable, false)))
        .unwrap();
    drop(sender);
    let config = CollabVerifierConfig::for_sso_origin("https://auth.self-hosted.example").unwrap();
    let mut renewer = LocalTicketRenewer {
        local_static: [0x42; 32],
        verifier: Arc::new(ProductionTicketVerifier::new_with_config(config).unwrap()),
        binding: binding.clone(),
        renew_at: start,
        shutdown_at,
        expires_at,
        retry_backoff: INITIAL_RENEWAL_RETRY_BACKOFF,
        pending: Some(PendingRenewal {
            receiver: Some(receiver),
            cancelled: Arc::new(AtomicBool::new(false)),
            worker: None,
        }),
    };

    assert!(matches!(renewer.poll(start), Ok(None)));
    assert!(renewer.pending.is_none());
    assert_eq!(
        renewer.renew_at.duration_since(start),
        INITIAL_RENEWAL_RETRY_BACKOFF
    );
    assert_eq!(renewer.retry_backoff, Duration::from_secs(2));
    assert_eq!(renewer.expires_at, expires_at);
    assert_eq!(renewer.shutdown_at, shutdown_at);
    assert_eq!(renewer.binding, binding);
}

#[test]
fn transient_renewal_failures_back_off_then_allow_success_before_expiry() {
    let start = Instant::now();
    let expires_at = start + Duration::from_secs(180);
    let (first_retry, second_backoff) =
        transient_retry_schedule(start, expires_at, INITIAL_RENEWAL_RETRY_BACKOFF).unwrap();
    assert_eq!(
        first_retry.duration_since(start),
        INITIAL_RENEWAL_RETRY_BACKOFF
    );
    let (second_retry, third_backoff) =
        transient_retry_schedule(first_retry, expires_at, second_backoff).unwrap();
    assert_eq!(
        second_retry.duration_since(first_retry),
        Duration::from_secs(2)
    );
    assert_eq!(third_backoff, Duration::from_secs(4));

    let (renew_at, renewed_expiry) = renewal_schedule(10_000, 910_000, second_retry).unwrap();
    assert!(renew_at < renewed_expiry);
    assert!(second_retry < renew_at);
}

#[test]
fn persistent_transient_renewal_failure_becomes_fatal_at_old_expiry() {
    let start = Instant::now();
    let expires_at = start + Duration::from_secs(3);
    let (retry_at, _) =
        transient_retry_schedule(start, expires_at, Duration::from_secs(30)).unwrap();
    assert_eq!(retry_at, expires_at);
    let error = transient_retry_schedule(retry_at, expires_at, Duration::from_secs(30))
        .expect_err("old ticket expiry is fail closed");
    assert_eq!(error.failure, CollabRuntimeFailure::TicketRejected);
}

#[test]
fn pending_fetch_is_cancelled_and_joined_before_terminal_hard_expiry() {
    let live = Arc::new(AtomicUsize::new(0));
    let worker_live = Arc::clone(&live);
    let published = Arc::new(AtomicUsize::new(0));
    let worker_published = Arc::clone(&published);
    let dropped = Arc::new(AtomicBool::new(false));
    let worker_dropped = Arc::clone(&dropped);
    let (sender, receiver) = mpsc::channel();
    let cancelled = Arc::new(AtomicBool::new(false));
    let worker_cancelled = Arc::clone(&cancelled);
    let (ready, started) = mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        worker_live.fetch_add(1, Ordering::SeqCst);
        let _result = crate::blocking::block_on(crate::jwks::await_or_cancel(
            async move {
                struct DropMarker(Arc<AtomicBool>);

                impl Drop for DropMarker {
                    fn drop(&mut self) {
                        self.0.store(true, Ordering::Release);
                    }
                }

                let _drop_marker = DropMarker(worker_dropped);
                ready.send(()).unwrap();
                std::future::pending::<()>().await;
            },
            &|| worker_cancelled.load(Ordering::Acquire),
        ));
        if sender.send(Err(authentication_unavailable())).is_ok() {
            worker_published.fetch_add(1, Ordering::SeqCst);
        }
        worker_live.fetch_sub(1, Ordering::SeqCst);
    });
    started.recv().unwrap();
    assert_eq!(live.load(Ordering::SeqCst), 1);

    let start = Instant::now();
    drop(PendingRenewal {
        receiver: Some(receiver),
        cancelled,
        worker: Some(worker),
    });
    assert!(
        Instant::now() < start + TERMINAL_FLUSH_TIMEOUT,
        "worker cancellation and join must fit inside the reserved terminal window"
    );
    assert_eq!(live.load(Ordering::SeqCst), 0);
    assert_eq!(published.load(Ordering::SeqCst), 0);
    assert!(dropped.load(Ordering::Acquire));

    let restart_started = Instant::now();
    let replacement = std::thread::spawn(|| {
        crate::blocking::block_on(crate::jwks::await_or_cancel(async { 42 }, &never_cancelled))
    });
    assert_eq!(replacement.join().unwrap(), Ok(42));
    assert!(restart_started.elapsed() < Duration::from_secs(1));
}
