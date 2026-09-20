use std::{
    future::Future,
    num::NonZeroU64,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures_util::{SinkExt, StreamExt};
use op_collab_relay_protocol::{
    CallerDeviceDhPublic, RelayClientHello, RelayLocatorV1, RelayReauthChallengeV1,
    RelayReauthResponseV1, RelayRole, RouteMapKey,
};
use tokio::{
    sync::{mpsc, Semaphore},
    time::{self, Instant},
};
use tokio_tungstenite::tungstenite::Message;

use crate::{
    auth::{AuthenticatedRoute, RelayAuthenticator, RelayBearerCredential, RelayUpgradeChallenge},
    connection::{WebSocketSink, WebSocketSource},
    registry::QueuedPayload,
};

const MAX_REAUTH_LEAD: Duration = Duration::from_secs(30);
const SERVER_REAUTH_REMAINING_NUMERATOR: u32 = 1;
const SERVER_REAUTH_REMAINING_DENOMINATOR: u32 = 5;

#[derive(Clone, Copy)]
pub(crate) struct RelayAuthState {
    expires_at_unix: NonZeroU64,
    deadline: Instant,
}

impl RelayAuthState {
    pub(crate) fn new(expires_at_unix: NonZeroU64) -> Option<Self> {
        Some(Self {
            expires_at_unix,
            deadline: authentication_deadline(expires_at_unix)?,
        })
    }

    pub(crate) const fn deadline(self) -> Instant {
        self.deadline
    }

    pub(crate) fn effective_deadline(self, configured_deadline: Instant) -> Instant {
        self.deadline.min(configured_deadline)
    }

    pub(crate) fn reauth_at(
        self,
        now: Instant,
        configured_deadline: Instant,
        renewal_cap_unix: u64,
    ) -> Option<Instant> {
        if self.deadline >= configured_deadline || self.expires_at_unix.get() >= renewal_cap_unix {
            return None;
        }
        let remaining = self.deadline.saturating_duration_since(now);
        // Desktop renews halfway through its observed remaining TTL. Challenge
        // after 80%, leaving a bounded 20% response window and ample time for
        // both long and short admissions to publish their fresh ticket.
        let lead = (remaining * SERVER_REAUTH_REMAINING_NUMERATOR
            / SERVER_REAUTH_REMAINING_DENOMINATOR)
            .min(MAX_REAUTH_LEAD);
        if lead.is_zero() {
            return None;
        }
        self.deadline.checked_sub(lead).or(Some(now))
    }

    fn extend(self, expires_at_unix: NonZeroU64) -> Option<Self> {
        if expires_at_unix <= self.expires_at_unix {
            return None;
        }
        let next = Self::new(expires_at_unix)?;
        (next.deadline > self.deadline).then_some(next)
    }
}

pub(crate) struct RelaySessionIdentity {
    route: RouteMapKey,
    role: RelayRole,
    caller_device_dh: CallerDeviceDhPublic,
    locator: RelayLocatorV1,
}

impl RelaySessionIdentity {
    pub(crate) fn new(
        route: RouteMapKey,
        role: RelayRole,
        hello: &RelayClientHello,
    ) -> Option<Self> {
        if hello.role() != role {
            return None;
        }
        Some(Self {
            route,
            role,
            caller_device_dh: *hello.auth_extension().caller_device_dh_pub_x25519(),
            locator: hello.locator().clone(),
        })
    }

    pub(crate) fn matches_hello(&self, hello: &RelayClientHello) -> bool {
        hello.role() == self.role
            && hello.auth_extension().caller_device_dh_pub_x25519() == &self.caller_device_dh
            && hello.locator() == &self.locator
    }

    pub(crate) fn accepted_expiry(&self, authenticated: &AuthenticatedRoute) -> Option<NonZeroU64> {
        if authenticated.route != self.route || authenticated.role != self.role {
            return None;
        }
        NonZeroU64::new(
            authenticated
                .expires_at_unix
                .get()
                .min(self.locator.claims().expires_at_unix()),
        )
    }

    pub(crate) fn locator_expiry_unix(&self) -> u64 {
        self.locator.claims().expires_at_unix()
    }
}

pub(crate) struct ReauthRelayTraffic<'a> {
    pub(crate) relay_rx: &'a mut mpsc::Receiver<QueuedPayload>,
    pub(crate) peer_tx: &'a mpsc::Sender<QueuedPayload>,
    pub(crate) route_budget: &'a Arc<Semaphore>,
    pub(crate) queued_bytes: &'a Arc<Semaphore>,
    pub(crate) max_message_bytes: usize,
}

pub(crate) enum ReauthOutcome {
    Extended(RelayAuthState),
    StillCurrent,
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn perform_reauthentication(
    sink: &mut WebSocketSink,
    source: &mut WebSocketSource,
    authenticator: Arc<dyn RelayAuthenticator>,
    reauth_in_flight: Arc<Semaphore>,
    identity: &RelaySessionIdentity,
    current: RelayAuthState,
    configured_deadline: Instant,
    response_timeout: Duration,
    traffic: Option<&mut ReauthRelayTraffic<'_>>,
) -> Result<ReauthOutcome, ()> {
    let mut traffic = traffic;
    let now = Instant::now();
    let response_deadline = now
        .checked_add(response_timeout)
        .unwrap_or(current.deadline())
        .min(current.deadline())
        .min(configured_deadline);
    if response_deadline <= now {
        return Err(());
    }

    let auth_permit = await_paired_phase(
        sink,
        source,
        response_deadline,
        reauth_in_flight.acquire_owned(),
        traffic.as_deref_mut(),
    )
    .await?
    .map_err(|_| ())?;
    let challenge_authenticator = Arc::clone(&authenticator);
    let challenge_key = tokio::task::spawn_blocking(move || {
        (challenge_authenticator.challenge_key_id(), auth_permit)
    });
    let (key_id, auth_permit) = match await_paired_phase(
        sink,
        source,
        response_deadline,
        challenge_key,
        traffic.as_deref_mut(),
    )
    .await?
    {
        Ok((Ok(Some(key_id)), permit)) => (key_id, permit),
        Ok((Ok(None) | Err(_), _)) | Err(_) => return Err(()),
    };
    let challenge =
        RelayUpgradeChallenge::generate(key_id, response_deadline.into_std()).map_err(|_| ())?;
    let challenge_text = RelayReauthChallengeV1::new(challenge.challenge().clone()).encode_text();
    time::timeout_at(response_deadline, sink.send(Message::Text(challenge_text)))
        .await
        .map_err(|_| ())?
        .map_err(|_| ())?;

    let response = match traffic.as_deref_mut() {
        Some(traffic) => receive_paired_response(sink, source, response_deadline, traffic).await?,
        None => receive_waiting_response(sink, source, response_deadline).await?,
    };
    if response.challenge() != challenge.challenge() || !identity.matches_hello(response.hello()) {
        return Err(());
    }
    let credential = RelayBearerCredential::new(response.bearer().to_vec());
    let hello = response.hello().clone();
    drop(response);

    let authentication = tokio::task::spawn_blocking(move || {
        let _auth_permit = auth_permit;
        authenticator.authenticate(&hello, Some(&credential), Some(challenge))
    });
    let authenticated =
        await_paired_phase(sink, source, response_deadline, authentication, traffic)
            .await?
            .map_err(|_| ())?
            .map_err(|_| ())?;
    let accepted_expiry = identity.accepted_expiry(&authenticated).ok_or(())?;
    let Some(extended) = current.extend(accepted_expiry) else {
        return Ok(ReauthOutcome::StillCurrent);
    };
    if extended.effective_deadline(configured_deadline)
        <= current.effective_deadline(configured_deadline)
    {
        return Err(());
    }
    Ok(ReauthOutcome::Extended(extended))
}

async fn await_paired_phase<F, T>(
    sink: &mut WebSocketSink,
    source: &mut WebSocketSource,
    deadline: Instant,
    phase: F,
    traffic: Option<&mut ReauthRelayTraffic<'_>>,
) -> Result<T, ()>
where
    F: Future<Output = T>,
{
    tokio::pin!(phase);
    let Some(traffic) = traffic else {
        return time::timeout_at(deadline, phase).await.map_err(|_| ());
    };
    loop {
        tokio::select! {
            biased;
            _ = time::sleep_until(deadline) => return Err(()),
            result = &mut phase => return Ok(result),
            message = source.next() => {
                match message {
                    Some(Ok(Message::Binary(payload))) => {
                        forward_to_peer(payload, traffic)?;
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        time::timeout_at(deadline, sink.send(Message::Pong(payload)))
                            .await
                            .map_err(|_| ())?
                            .map_err(|_| ())?;
                    }
                    Some(Ok(Message::Pong(_))) => {}
                    Some(Ok(Message::Text(_) | Message::Close(_) | Message::Frame(_)))
                    | Some(Err(_))
                    | None => return Err(()),
                }
            }
            queued = traffic.relay_rx.recv() => {
                let Some(queued) = queued else {
                    return Err(());
                };
                let (payload, _global_budget, _route_budget) = queued.into_parts();
                time::timeout_at(deadline, sink.send(Message::Binary(payload)))
                    .await
                    .map_err(|_| ())?
                    .map_err(|_| ())?;
            }
        }
    }
}

async fn receive_paired_response(
    sink: &mut WebSocketSink,
    source: &mut WebSocketSource,
    deadline: Instant,
    traffic: &mut ReauthRelayTraffic<'_>,
) -> Result<RelayReauthResponseV1, ()> {
    loop {
        tokio::select! {
            biased;
            _ = time::sleep_until(deadline) => return Err(()),
            message = source.next() => {
                match message {
                    Some(Ok(Message::Text(text))) => {
                        return RelayReauthResponseV1::decode_text(&text).map_err(|_| ());
                    }
                    Some(Ok(Message::Binary(payload))) => {
                        forward_to_peer(payload, traffic)?;
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        time::timeout_at(deadline, sink.send(Message::Pong(payload)))
                            .await
                            .map_err(|_| ())?
                            .map_err(|_| ())?;
                    }
                    Some(Ok(Message::Pong(_))) => {}
                    Some(Ok(Message::Close(_) | Message::Frame(_)))
                    | Some(Err(_))
                    | None => return Err(()),
                }
            }
            queued = traffic.relay_rx.recv() => {
                let Some(queued) = queued else {
                    return Err(());
                };
                let (payload, _global_budget, _route_budget) = queued.into_parts();
                time::timeout_at(deadline, sink.send(Message::Binary(payload)))
                    .await
                    .map_err(|_| ())?
                    .map_err(|_| ())?;
            }
        }
    }
}

async fn receive_waiting_response(
    sink: &mut WebSocketSink,
    source: &mut WebSocketSource,
    deadline: Instant,
) -> Result<RelayReauthResponseV1, ()> {
    loop {
        let message = time::timeout_at(deadline, source.next())
            .await
            .map_err(|_| ())?;
        match message {
            Some(Ok(Message::Text(text))) => {
                return RelayReauthResponseV1::decode_text(&text).map_err(|_| ());
            }
            Some(Ok(Message::Ping(payload))) => {
                time::timeout_at(deadline, sink.send(Message::Pong(payload)))
                    .await
                    .map_err(|_| ())?
                    .map_err(|_| ())?;
            }
            Some(Ok(Message::Pong(_))) => {}
            Some(Ok(Message::Binary(_) | Message::Close(_) | Message::Frame(_)))
            | Some(Err(_))
            | None => return Err(()),
        }
    }
}

fn forward_to_peer(payload: Vec<u8>, traffic: &ReauthRelayTraffic<'_>) -> Result<(), ()> {
    if payload.len() > traffic.max_message_bytes {
        return Err(());
    }
    let permits = u32::try_from(payload.len()).map_err(|_| ())?;
    let global_budget = Arc::clone(traffic.queued_bytes)
        .try_acquire_many_owned(permits)
        .map_err(|_| ())?;
    let route_budget = Arc::clone(traffic.route_budget)
        .try_acquire_many_owned(permits)
        .map_err(|_| ())?;
    traffic
        .peer_tx
        .try_send(QueuedPayload::new(payload, global_budget, route_budget))
        .map_err(|_| ())
}

fn authentication_deadline(expires_at_unix: NonZeroU64) -> Option<Instant> {
    let expiry = UNIX_EPOCH.checked_add(Duration::from_secs(expires_at_unix.get()))?;
    let remaining = expiry.duration_since(SystemTime::now()).ok()?;
    (!remaining.is_zero())
        .then(|| Instant::now().checked_add(remaining))
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_state_never_extends_for_same_or_older_ticket() {
        let current = RelayAuthState {
            expires_at_unix: NonZeroU64::new(200).unwrap(),
            deadline: Instant::now() + Duration::from_secs(100),
        };
        assert!(current.extend(NonZeroU64::new(199).unwrap()).is_none());
        assert!(current.extend(NonZeroU64::new(200).unwrap()).is_none());
    }

    #[test]
    fn configured_tunnel_deadline_remains_an_absolute_cap() {
        let now = Instant::now();
        let current = RelayAuthState {
            expires_at_unix: NonZeroU64::new(100).unwrap(),
            deadline: now + Duration::from_secs(10),
        };
        let configured = now + Duration::from_secs(5);
        assert_eq!(current.effective_deadline(configured), configured);
        assert_eq!(current.reauth_at(now, configured, u64::MAX), None);
    }

    #[test]
    fn reauth_is_scheduled_before_current_authentication_expiry() {
        let now = Instant::now();
        let current = RelayAuthState {
            expires_at_unix: NonZeroU64::new(100).unwrap(),
            deadline: now + Duration::from_secs(100),
        };
        let scheduled = current
            .reauth_at(now, now + Duration::from_secs(200), u64::MAX)
            .unwrap();
        assert_eq!(scheduled, current.deadline - Duration::from_secs(20));
        assert!(scheduled < current.deadline);
    }

    #[test]
    fn locator_expiry_cap_does_not_schedule_a_guaranteed_failed_reauth() {
        let now = Instant::now();
        let current = RelayAuthState {
            expires_at_unix: NonZeroU64::new(500).unwrap(),
            deadline: now + Duration::from_secs(30),
        };
        assert_eq!(
            current.reauth_at(
                now,
                now + Duration::from_secs(60),
                current.expires_at_unix.get(),
            ),
            None
        );
    }

    #[test]
    fn reauth_schedule_follows_client_half_life_renewal_contract() {
        let now = Instant::now();
        for (ttl, desktop_renewal, server_challenge) in [(5, 2.5, 4), (10, 5.0, 8), (40, 20.0, 32)]
        {
            let current = RelayAuthState {
                expires_at_unix: NonZeroU64::new(600).unwrap(),
                deadline: now + Duration::from_secs(ttl),
            };
            let scheduled = current
                .reauth_at(now, now + Duration::from_secs(120), u64::MAX)
                .unwrap();
            assert_eq!(scheduled, now + Duration::from_secs(server_challenge));
            assert!(
                scheduled > now + Duration::from_secs_f64(desktop_renewal),
                "{ttl}-second server challenge follows desktop renewal"
            );
        }
    }
}
