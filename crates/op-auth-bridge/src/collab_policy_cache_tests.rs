use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use serde::Deserialize;

use super::*;

const ISSUER: &str = "https://collab.example.com";
const ENDPOINT: &str = "https://cn.example/api/v1/collab/policy";
const NOW: u64 = 1_800_000_000;
const ACTIVE_KEY_X: &str = "ebVWLo_mVPlAeLES6KmLp5AfhTrmlb7X4OORC60ElmQ";
const GO_V2_FIXTURE: &str = include_str!("../tests/fixtures/zseven-sso-go-union-policy-v2.json");

#[derive(Deserialize)]
struct GoUnionPolicyFixture {
    policy_json: String,
}

#[derive(Default)]
struct FetchState {
    requests: Vec<(String, Option<String>)>,
    responses: VecDeque<Result<CollabJwksFetchResponse, CollabJwksFetchError>>,
}

#[derive(Clone)]
struct PolicyFetcher(Arc<Mutex<FetchState>>);

impl CollabJwksFetcher for PolicyFetcher {
    fn fetch(
        &self,
        request: CollabJwksFetchRequest<'_>,
    ) -> Result<CollabJwksFetchResponse, CollabJwksFetchError> {
        let mut state = self.0.lock().unwrap();
        state
            .requests
            .push((request.endpoint.to_owned(), request.etag.map(str::to_owned)));
        state
            .responses
            .pop_front()
            .unwrap_or(Err(CollabJwksFetchError::Unavailable))
    }
}

fn policy_body() -> Vec<u8> {
    serde_json::from_str::<GoUnionPolicyFixture>(GO_V2_FIXTURE)
        .unwrap()
        .policy_json
        .into_bytes()
}

fn active_key() -> [u8; 32] {
    URL_SAFE_NO_PAD
        .decode(ACTIVE_KEY_X)
        .unwrap()
        .try_into()
        .unwrap()
}

fn modified(body: Vec<u8>) -> Result<CollabJwksFetchResponse, CollabJwksFetchError> {
    Ok(CollabJwksFetchResponse::Modified {
        body,
        etag: Some("\"policy-v7\"".to_owned()),
        max_age_seconds: 300,
    })
}

#[test]
fn signed_policy_cache_uses_only_eligible_keys_and_preserves_etag_refresh() {
    let state = Arc::new(Mutex::new(FetchState {
        responses: VecDeque::from([
            modified(policy_body()),
            Ok(CollabJwksFetchResponse::NotModified {
                etag: None,
                max_age_seconds: 300,
            }),
        ]),
        ..FetchState::default()
    }));
    let cache = CollabJwksCache::new_signed_policy(
        ENDPOINT,
        ISSUER,
        PolicyFetcher(Arc::clone(&state)),
        CollabJwksCacheLimits::default(),
    )
    .unwrap();
    let cache_now = Instant::now();

    assert_eq!(
        cache
            .policy_verification_key("cn-active", cache_now, NOW)
            .unwrap(),
        active_key()
    );
    assert_eq!(cache.cached_key_count().unwrap(), 4);
    assert_eq!(
        cache.policy_verification_key("cn-next", cache_now, NOW),
        Err(CollabJwksError::UnknownKey)
    );
    assert_eq!(state.lock().unwrap().requests.len(), 1);

    assert_eq!(
        cache.policy_verification_key("unknown", cache_now + Duration::from_secs(31), NOW + 31),
        Err(CollabJwksError::UnknownKey)
    );
    let requests = &state.lock().unwrap().requests;
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0], (ENDPOINT.to_owned(), None));
    assert_eq!(
        requests[1],
        (ENDPOINT.to_owned(), Some("\"policy-v7\"".to_owned()))
    );
}

#[test]
fn cached_policy_expiry_forces_refresh_and_still_fails_closed_on_304() {
    let state = Arc::new(Mutex::new(FetchState {
        responses: VecDeque::from([
            modified(policy_body()),
            Ok(CollabJwksFetchResponse::NotModified {
                etag: None,
                max_age_seconds: 300,
            }),
        ]),
        ..FetchState::default()
    }));
    let cache = CollabJwksCache::new_signed_policy(
        ENDPOINT,
        ISSUER,
        PolicyFetcher(Arc::clone(&state)),
        CollabJwksCacheLimits::default(),
    )
    .unwrap();
    let cache_now = Instant::now();
    cache
        .policy_verification_key("cn-active", cache_now, NOW)
        .unwrap();

    assert_eq!(
        cache.policy_verification_key(
            "cn-active",
            cache_now + Duration::from_secs(1),
            1_800_500_000
        ),
        Err(CollabJwksError::Policy(CollabUnionPolicyError::Inactive))
    );
    assert_eq!(state.lock().unwrap().requests.len(), 2);
}
