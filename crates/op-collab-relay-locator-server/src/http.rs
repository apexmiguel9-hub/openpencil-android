use std::{
    future::Future,
    net::IpAddr,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use axum::{
    body::Bytes,
    extract::{Extension, OriginalUri, State},
    http::{
        header::{
            ACCEPT, AUTHORIZATION, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE, RETRY_AFTER,
            TRANSFER_ENCODING, WWW_AUTHENTICATE,
        },
        HeaderMap, HeaderValue, StatusCode,
    },
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use hyper::server::conn::http1;
use hyper_util::{
    rt::{TokioIo, TokioTimer},
    service::TowerToHyperService,
};
use op_auth_bridge::CollabJwksFetcher;
use op_collab_relay_control_plane::{
    PairingCodeStore, RelayLocatorPublishService, RelayLocatorPublishServiceError,
    RelayLocatorSigner, RelayOwnerPublishPolicy, RelayPairingService, RelayPairingServiceError,
    SignedLocatorResponse, MAX_PAIRING_PUBLISH_REQUEST_BYTES, MAX_PUBLISH_AUTHORIZATION_BYTES,
    MIN_PAIRING_PUBLISH_REQUEST_BYTES, OWNER_PUBLISH_CONTENT_TYPE, OWNER_PUBLISH_REQUEST_BYTES,
    PAIRING_CLAIM_CONTENT_TYPE, PAIRING_CLAIM_PATH, PAIRING_CLAIM_REQUEST_BYTES,
    PAIRING_PUBLISH_CONTENT_TYPE, PAIRING_PUBLISH_PATH, RELAY_LOCATOR_PUBLISH_PATH,
    SEALED_INVITE_CONTENT_TYPE, SIGNED_LOCATOR_CONTENT_TYPE,
};
use tokio::{
    net::TcpListener,
    sync::{watch, OwnedSemaphorePermit, Semaphore},
    task::JoinSet,
    time,
};
use tower_http::{limit::RequestBodyLimitLayer, timeout::RequestBodyTimeoutLayer};
use zeroize::Zeroizing;

use crate::LocatorServerConfig;

pub(crate) mod rate_limit;

use rate_limit::RateLimiter;

pub const MAX_HTTP_HEADERS: usize = 32;
pub const MAX_HTTP_HEADER_BYTES: usize = 64 * 1024;

pub trait LocatorPublisher: Send + Sync + 'static {
    fn publish(
        &self,
        request_body: &[u8],
        opaque_ticket: &[u8],
    ) -> Result<SignedLocatorResponse, RelayLocatorPublishServiceError>;
}

impl<F, S, P> LocatorPublisher for RelayLocatorPublishService<F, S, P>
where
    F: CollabJwksFetcher + 'static,
    S: RelayLocatorSigner + 'static,
    P: RelayOwnerPublishPolicy + 'static,
{
    fn publish(
        &self,
        request_body: &[u8],
        opaque_ticket: &[u8],
    ) -> Result<SignedLocatorResponse, RelayLocatorPublishServiceError> {
        RelayLocatorPublishService::publish(self, request_body, opaque_ticket)
    }
}

pub trait PairingEndpoints: Send + Sync + 'static {
    fn publish(
        &self,
        request_body: &[u8],
        opaque_ticket: &[u8],
    ) -> Result<(), RelayPairingServiceError>;

    fn claim(
        &self,
        request_body: &[u8],
        opaque_ticket: &[u8],
    ) -> Result<Vec<u8>, RelayPairingServiceError>;
}

impl<F, S> PairingEndpoints for RelayPairingService<F, S>
where
    F: CollabJwksFetcher + 'static,
    S: PairingCodeStore + 'static,
{
    fn publish(
        &self,
        request_body: &[u8],
        opaque_ticket: &[u8],
    ) -> Result<(), RelayPairingServiceError> {
        let now_unix = unix_now().ok_or(RelayPairingServiceError::Unavailable)?;
        self.publish_at(request_body, opaque_ticket, now_unix, Instant::now())
    }

    fn claim(
        &self,
        request_body: &[u8],
        opaque_ticket: &[u8],
    ) -> Result<Vec<u8>, RelayPairingServiceError> {
        let now_unix = unix_now().ok_or(RelayPairingServiceError::Unavailable)?;
        self.claim_at(request_body, opaque_ticket, now_unix, Instant::now())
    }
}

/// Address of the peer that opened the connection a request arrived on.
///
/// Attached per connection by the accept loop, because the manual
/// `hyper`/`TowerToHyperService` wiring gives the handler no other view of the
/// socket. Requests are rate limited against this address.
#[derive(Clone, Copy, Debug)]
struct ClientAddress(IpAddr);

struct AppState {
    publisher: Arc<dyn LocatorPublisher>,
    pairing: Arc<dyn PairingEndpoints>,
    in_flight: Arc<Semaphore>,
    auth_in_flight: Arc<Semaphore>,
    rate_limiter: Arc<RateLimiter>,
    auth_timeout: Duration,
}

impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            publisher: Arc::clone(&self.publisher),
            pairing: Arc::clone(&self.pairing),
            in_flight: Arc::clone(&self.in_flight),
            auth_in_flight: Arc::clone(&self.auth_in_flight),
            rate_limiter: Arc::clone(&self.rate_limiter),
            auth_timeout: self.auth_timeout,
        }
    }
}

struct SensitiveBearer(Zeroizing<Vec<u8>>);

impl SensitiveBearer {
    fn parse(headers: &HeaderMap) -> Result<Self, StatusCode> {
        let mut values = headers.get_all(AUTHORIZATION).iter();
        let value = values.next().ok_or(StatusCode::UNAUTHORIZED)?;
        if values.next().is_some() || value.as_bytes().len() > MAX_PUBLISH_AUTHORIZATION_BYTES {
            return Err(StatusCode::UNAUTHORIZED);
        }
        let bearer = value
            .as_bytes()
            .strip_prefix(b"Bearer ")
            .ok_or(StatusCode::UNAUTHORIZED)?;
        if bearer.is_empty() || !valid_b64token(bearer) {
            return Err(StatusCode::UNAUTHORIZED);
        }
        Ok(Self(Zeroizing::new(bearer.to_vec())))
    }

    fn expose(&self) -> &[u8] {
        &self.0
    }
}

impl std::fmt::Debug for SensitiveBearer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SensitiveBearer([REDACTED])")
    }
}

pub async fn serve_listener_until<F>(
    listener: TcpListener,
    config: LocatorServerConfig,
    publisher: Arc<dyn LocatorPublisher>,
    pairing: Arc<dyn PairingEndpoints>,
    shutdown: F,
) -> Result<(), LocatorServerError>
where
    F: Future<Output = ()>,
{
    config.limits.validate()?;
    let local_address = listener.local_addr().map_err(LocatorServerError::Accept)?;
    tracing::info!(address = %local_address, "relay locator service listening");

    let state = AppState {
        publisher,
        pairing,
        in_flight: Arc::new(Semaphore::new(config.limits.max_in_flight.get())),
        auth_in_flight: Arc::new(Semaphore::new(config.limits.max_auth_in_flight.get())),
        rate_limiter: Arc::new(RateLimiter::new(
            config.limits.max_requests_per_second,
            config.limits.max_client_requests_per_second,
            config.limits.max_connections,
        )),
        auth_timeout: config.limits.auth_timeout,
    };
    // Body ceilings are per-route so the pairing publish path (the only one
    // needing a larger body) does not loosen the transport-level bound on
    // the locator route or the unauthenticated health probe.
    let router = Router::new()
        .route(
            RELAY_LOCATOR_PUBLISH_PATH,
            post(publish_locator).layer(RequestBodyLimitLayer::new(OWNER_PUBLISH_REQUEST_BYTES)),
        )
        .route(
            PAIRING_PUBLISH_PATH,
            post(publish_pairing_code).layer(RequestBodyLimitLayer::new(
                MAX_PAIRING_PUBLISH_REQUEST_BYTES,
            )),
        )
        .route(
            PAIRING_CLAIM_PATH,
            post(claim_pairing_code).layer(RequestBodyLimitLayer::new(PAIRING_CLAIM_REQUEST_BYTES)),
        )
        .route("/healthz", get(health).layer(RequestBodyLimitLayer::new(0)))
        .layer(RequestBodyTimeoutLayer::new(config.limits.body_timeout))
        .with_state(state);
    let connection_capacity = Arc::new(Semaphore::new(config.limits.max_connections.get()));
    let (shutdown_sender, shutdown_receiver) = watch::channel(false);
    let mut connections = JoinSet::new();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            biased;
            () = &mut shutdown => break,
            accepted = listener.accept() => {
                let (stream, peer) = accepted.map_err(LocatorServerError::Accept)?;
                let Ok(connection_permit) =
                    Arc::clone(&connection_capacity).try_acquire_owned()
                else {
                    continue;
                };
                // The router is shared, so the connecting peer is attached per
                // connection: it is the only place the socket address is known.
                let service = TowerToHyperService::new(
                    router.clone().layer(Extension(ClientAddress(peer.ip()))),
                );
                let connection_shutdown = shutdown_receiver.clone();
                let limits = config.limits.clone();
                connections.spawn(async move {
                    serve_connection(
                        stream,
                        service,
                        limits.header_timeout,
                        limits.shutdown_grace,
                        connection_shutdown,
                        connection_permit,
                    )
                    .await;
                });
            }
            Some(result) = connections.join_next(), if !connections.is_empty() => {
                if result.is_err() {
                    tracing::warn!("locator connection task failed");
                }
            }
        }
    }

    tracing::info!("relay locator shutdown requested");
    let _ = shutdown_sender.send(true);
    let drain = async { while connections.join_next().await.is_some() {} };
    if time::timeout(config.limits.shutdown_grace, drain)
        .await
        .is_err()
    {
        connections.shutdown().await;
    }
    Ok(())
}

async fn serve_connection<S>(
    stream: tokio::net::TcpStream,
    service: TowerToHyperService<S>,
    header_timeout: Duration,
    shutdown_grace: Duration,
    mut shutdown: watch::Receiver<bool>,
    _connection_permit: OwnedSemaphorePermit,
) where
    S: tower_service::Service<
            hyper::Request<hyper::body::Incoming>,
            Response = axum::response::Response,
            Error = std::convert::Infallible,
        > + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
{
    let mut builder = http1::Builder::new();
    builder
        .timer(TokioTimer::new())
        .header_read_timeout(header_timeout)
        .max_headers(MAX_HTTP_HEADERS)
        .max_buf_size(MAX_HTTP_HEADER_BYTES);
    let connection = builder.serve_connection(TokioIo::new(stream), service);
    tokio::pin!(connection);
    tokio::select! {
        result = &mut connection => {
            if result.is_err() {
                tracing::debug!("locator HTTP connection closed with protocol error");
            }
        }
        changed = shutdown.changed() => {
            if changed.is_ok() {
                connection.as_mut().graceful_shutdown();
                let _ = time::timeout(shutdown_grace, &mut connection).await;
            }
        }
    }
}

async fn health() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn publish_locator(
    State(state): State<AppState>,
    Extension(client): Extension<ClientAddress>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if uri.path() != RELAY_LOCATOR_PUBLISH_PATH || uri.query().is_some() {
        return empty_response(StatusCode::NOT_FOUND);
    }
    if !valid_publish_headers(&headers) || body.len() != OWNER_PUBLISH_REQUEST_BYTES {
        return empty_response(StatusCode::BAD_REQUEST);
    }
    let bearer = match SensitiveBearer::parse(&headers) {
        Ok(value) => value,
        Err(status) => {
            return response_with_header(
                status,
                WWW_AUTHENTICATE,
                HeaderValue::from_static("Bearer"),
            );
        }
    };
    if !state.rate_limiter.allow(Instant::now(), client.0) {
        return response_with_header(
            StatusCode::TOO_MANY_REQUESTS,
            RETRY_AFTER,
            HeaderValue::from_static("1"),
        );
    }
    let Ok(_request_permit) = Arc::clone(&state.in_flight).try_acquire_owned() else {
        return empty_response(StatusCode::SERVICE_UNAVAILABLE);
    };
    let Ok(auth_permit) = Arc::clone(&state.auth_in_flight).try_acquire_owned() else {
        return empty_response(StatusCode::SERVICE_UNAVAILABLE);
    };
    let publisher = Arc::clone(&state.publisher);
    let request = body.to_vec();
    let task = tokio::task::spawn_blocking(move || {
        let _auth_permit = auth_permit;
        publisher.publish(&request, bearer.expose())
    });
    let result = match time::timeout(state.auth_timeout, task).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) | Err(_) => {
            return empty_response(StatusCode::SERVICE_UNAVAILABLE);
        }
    };
    match result {
        Ok(locator) => {
            let encoded = locator.encode();
            (
                StatusCode::OK,
                [(CONTENT_TYPE, SIGNED_LOCATOR_CONTENT_TYPE)],
                encoded,
            )
                .into_response()
        }
        Err(RelayLocatorPublishServiceError::RequestRejected) => {
            empty_response(StatusCode::BAD_REQUEST)
        }
        Err(RelayLocatorPublishServiceError::AuthenticationFailed) => response_with_header(
            StatusCode::UNAUTHORIZED,
            WWW_AUTHENTICATE,
            HeaderValue::from_static("Bearer"),
        ),
        Err(RelayLocatorPublishServiceError::Unavailable) => {
            empty_response(StatusCode::SERVICE_UNAVAILABLE)
        }
    }
}

async fn publish_pairing_code(
    State(state): State<AppState>,
    Extension(client): Extension<ClientAddress>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if uri.path() != PAIRING_PUBLISH_PATH || uri.query().is_some() {
        return empty_response(StatusCode::NOT_FOUND);
    }
    if !valid_pairing_publish_headers(&headers, body.len())
        || !(MIN_PAIRING_PUBLISH_REQUEST_BYTES..=MAX_PAIRING_PUBLISH_REQUEST_BYTES)
            .contains(&body.len())
    {
        return empty_response(StatusCode::BAD_REQUEST);
    }
    let bearer = match SensitiveBearer::parse(&headers) {
        Ok(value) => value,
        Err(status) => {
            return response_with_header(
                status,
                WWW_AUTHENTICATE,
                HeaderValue::from_static("Bearer"),
            );
        }
    };
    if !state.rate_limiter.allow(Instant::now(), client.0) {
        return response_with_header(
            StatusCode::TOO_MANY_REQUESTS,
            RETRY_AFTER,
            HeaderValue::from_static("1"),
        );
    }
    let Ok(_request_permit) = Arc::clone(&state.in_flight).try_acquire_owned() else {
        return empty_response(StatusCode::SERVICE_UNAVAILABLE);
    };
    let Ok(auth_permit) = Arc::clone(&state.auth_in_flight).try_acquire_owned() else {
        return empty_response(StatusCode::SERVICE_UNAVAILABLE);
    };
    let pairing = Arc::clone(&state.pairing);
    let request = body.to_vec();
    let task = tokio::task::spawn_blocking(move || {
        let _auth_permit = auth_permit;
        pairing.publish(&request, bearer.expose())
    });
    let result = match time::timeout(state.auth_timeout, task).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) | Err(_) => {
            return empty_response(StatusCode::SERVICE_UNAVAILABLE);
        }
    };
    match result {
        Ok(()) => empty_response(StatusCode::NO_CONTENT),
        Err(RelayPairingServiceError::RequestRejected) => empty_response(StatusCode::BAD_REQUEST),
        Err(RelayPairingServiceError::AuthenticationFailed) => response_with_header(
            StatusCode::UNAUTHORIZED,
            WWW_AUTHENTICATE,
            HeaderValue::from_static("Bearer"),
        ),
        Err(RelayPairingServiceError::NotFound) => empty_response(StatusCode::NOT_FOUND),
        Err(RelayPairingServiceError::Unavailable) => {
            empty_response(StatusCode::SERVICE_UNAVAILABLE)
        }
    }
}

async fn claim_pairing_code(
    State(state): State<AppState>,
    Extension(client): Extension<ClientAddress>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if uri.path() != PAIRING_CLAIM_PATH || uri.query().is_some() {
        return empty_response(StatusCode::NOT_FOUND);
    }
    if !valid_pairing_claim_headers(&headers, body.len())
        || body.len() != PAIRING_CLAIM_REQUEST_BYTES
    {
        return empty_response(StatusCode::BAD_REQUEST);
    }
    let bearer = match SensitiveBearer::parse(&headers) {
        Ok(value) => value,
        Err(status) => {
            return response_with_header(
                status,
                WWW_AUTHENTICATE,
                HeaderValue::from_static("Bearer"),
            );
        }
    };
    if !state.rate_limiter.allow(Instant::now(), client.0) {
        return response_with_header(
            StatusCode::TOO_MANY_REQUESTS,
            RETRY_AFTER,
            HeaderValue::from_static("1"),
        );
    }
    let Ok(_request_permit) = Arc::clone(&state.in_flight).try_acquire_owned() else {
        return empty_response(StatusCode::SERVICE_UNAVAILABLE);
    };
    let Ok(auth_permit) = Arc::clone(&state.auth_in_flight).try_acquire_owned() else {
        return empty_response(StatusCode::SERVICE_UNAVAILABLE);
    };
    let pairing = Arc::clone(&state.pairing);
    let request = body.to_vec();
    let task = tokio::task::spawn_blocking(move || {
        let _auth_permit = auth_permit;
        pairing.claim(&request, bearer.expose())
    });
    let result = match time::timeout(state.auth_timeout, task).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) | Err(_) => {
            return empty_response(StatusCode::SERVICE_UNAVAILABLE);
        }
    };
    match result {
        Ok(sealed) => (
            StatusCode::OK,
            [(CONTENT_TYPE, SEALED_INVITE_CONTENT_TYPE)],
            sealed,
        )
            .into_response(),
        Err(RelayPairingServiceError::RequestRejected) => empty_response(StatusCode::BAD_REQUEST),
        Err(RelayPairingServiceError::AuthenticationFailed) => response_with_header(
            StatusCode::UNAUTHORIZED,
            WWW_AUTHENTICATE,
            HeaderValue::from_static("Bearer"),
        ),
        Err(RelayPairingServiceError::NotFound) => empty_response(StatusCode::NOT_FOUND),
        Err(RelayPairingServiceError::Unavailable) => {
            empty_response(StatusCode::SERVICE_UNAVAILABLE)
        }
    }
}

fn valid_publish_headers(headers: &HeaderMap) -> bool {
    exactly_one_header(headers, CONTENT_TYPE, OWNER_PUBLISH_CONTENT_TYPE)
        && exactly_one_header(headers, ACCEPT, SIGNED_LOCATOR_CONTENT_TYPE)
        && exactly_one_header(
            headers,
            CONTENT_LENGTH,
            &OWNER_PUBLISH_REQUEST_BYTES.to_string(),
        )
        && !headers.contains_key(TRANSFER_ENCODING)
        && !headers.contains_key(CONTENT_ENCODING)
}

fn valid_pairing_publish_headers(headers: &HeaderMap, body_len: usize) -> bool {
    exactly_one_header(headers, CONTENT_TYPE, PAIRING_PUBLISH_CONTENT_TYPE)
        && exactly_one_header(headers, CONTENT_LENGTH, &body_len.to_string())
        && !headers.contains_key(TRANSFER_ENCODING)
        && !headers.contains_key(CONTENT_ENCODING)
}

fn valid_pairing_claim_headers(headers: &HeaderMap, body_len: usize) -> bool {
    exactly_one_header(headers, CONTENT_TYPE, PAIRING_CLAIM_CONTENT_TYPE)
        && exactly_one_header(headers, ACCEPT, SEALED_INVITE_CONTENT_TYPE)
        && exactly_one_header(headers, CONTENT_LENGTH, &body_len.to_string())
        && !headers.contains_key(TRANSFER_ENCODING)
        && !headers.contains_key(CONTENT_ENCODING)
}

fn exactly_one_header(
    headers: &HeaderMap,
    name: axum::http::header::HeaderName,
    expected: &str,
) -> bool {
    let mut values = headers.get_all(name).iter();
    values
        .next()
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == expected)
        && values.next().is_none()
}

fn valid_b64token(value: &[u8]) -> bool {
    let mut padding = false;
    for byte in value {
        if *byte == b'=' {
            padding = true;
        } else if padding
            || !(byte.is_ascii_alphanumeric()
                || matches!(*byte, b'-' | b'.' | b'_' | b'~' | b'+' | b'/'))
        {
            return false;
        }
    }
    value.first() != Some(&b'=')
}

fn unix_now() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
        .filter(|now| *now != 0)
}

fn empty_response(status: StatusCode) -> Response {
    status.into_response()
}

fn response_with_header(
    status: StatusCode,
    name: axum::http::header::HeaderName,
    value: HeaderValue,
) -> Response {
    (status, [(name, value)]).into_response()
}

#[derive(Debug, thiserror::Error)]
pub enum LocatorServerError {
    #[error(transparent)]
    Config(#[from] crate::LocatorServerConfigError),
    #[error("locator server failed to accept a connection")]
    Accept(#[source] std::io::Error),
}
