//! Bounded host/widget handoff for verified profile avatars.
//!
//! URLs arrive from authenticated account/collaboration profiles and stay in
//! this ephemeral process-local registry; encoded bytes stay in its LRU. The
//! desktop host owns SSRF-safe HTTPS and the existing image decode workers own
//! rasterization.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

pub const MAX_AVATAR_ENCODED_BYTES: usize = 512 * 1024;
pub const MAX_AVATAR_SOURCE_EDGE_PX: u32 = 1_024;
pub const MAX_AVATAR_SOURCE_PIXELS: u64 = 1_048_576;
pub const AVATAR_DECODE_EDGE_PX: u32 = 64;

const MAX_AVATAR_URL_BYTES: usize = 2_048;
const MAX_PARTICIPANT_KEY_BYTES: usize = 256;
const MAX_ACCOUNT_REVISION_BYTES: usize = 128;
const COLLAB_KEY_PREFIX: &str = "collab:";
const MAX_REGISTRY_KEY_BYTES: usize = MAX_PARTICIPANT_KEY_BYTES + COLLAB_KEY_PREFIX.len();
const ACCOUNT_AVATAR_KEY: &str = "account:current";
const AVATAR_CACHE_BYTE_BUDGET: usize = 8 * 1024 * 1024;
const AVATAR_CACHE_MAX_ENTRIES: usize = 64;
const AVATAR_PENDING_CAP: usize = 32;
const ACCOUNT_AVATAR_MAX_RETRIES: u8 = 6;
const ACCOUNT_AVATAR_MAX_RETRY_DELAY: Duration = Duration::from_secs(30);
const IMAGE_ID_PREFIX: u64 = 0xa710_0000_0000_0000;

/// Opaque request token. Its custom `Debug` deliberately omits the URL and
/// participant identity.
#[derive(Clone)]
pub struct CollabAvatarFetchRequest {
    session_generation: u64,
    participant_key: String,
    profile_generation: u64,
    image_id: u64,
    url: String,
}

impl CollabAvatarFetchRequest {
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The participant this avatar belongs to.
    ///
    /// Safe to expose even though `Debug` redacts identity: the value is the
    /// exact string a host has to put in the daemon proxy request body, and it
    /// is a `collab:`-namespaced session token, not the account's own key. The
    /// web host cannot reach `qlogo.cn` directly under CSP, so it posts this to
    /// `POST /api/collab/avatar` instead of using [`url`](Self::url).
    pub fn participant_key(&self) -> &str {
        &self.participant_key
    }

    /// Whether this request belongs to the locally authenticated account.
    ///
    /// Remote collaboration participants always occupy the separate
    /// `collab:` namespace and therefore cannot opt into account-only host
    /// fetch policy.
    pub fn is_current_account(&self) -> bool {
        self.participant_key == ACCOUNT_AVATAR_KEY
    }
}

impl fmt::Debug for CollabAvatarFetchRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CollabAvatarFetchRequest")
            .field("session_generation", &self.session_generation)
            .field("profile_generation", &self.profile_generation)
            .field("image_id", &self.image_id)
            .field("url", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone)]
pub struct CollabAvatarImage {
    pub image_id: u64,
    pub encoded: Arc<[u8]>,
}

impl fmt::Debug for CollabAvatarImage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CollabAvatarImage")
            .field("image_id", &self.image_id)
            .field("encoded_bytes", &self.encoded.len())
            .finish()
    }
}

enum SlotState {
    Waiting,
    Queued,
    InFlight,
    Ready,
    Failed,
}

struct AvatarSlot {
    url: String,
    profile_generation: u64,
    image_id: u64,
    state: SlotState,
    last_used: u64,
    failure_count: u8,
    retry_at: Option<Instant>,
}

struct AvatarRegistry {
    slots: HashMap<String, AvatarSlot>,
    /// Encoded cache keyed only by a process-local opaque id. Participant
    /// names and raw URLs are never cache keys.
    encoded: HashMap<u64, Arc<[u8]>>,
    pending: VecDeque<CollabAvatarFetchRequest>,
    cached_bytes: usize,
    session_generation: u64,
    tick: u64,
    next_image_id: u64,
    byte_budget: usize,
    max_entries: usize,
    pending_cap: usize,
}

impl AvatarRegistry {
    fn new() -> Self {
        Self::with_limits(
            AVATAR_CACHE_BYTE_BUDGET,
            AVATAR_CACHE_MAX_ENTRIES,
            AVATAR_PENDING_CAP,
        )
    }

    fn with_limits(byte_budget: usize, max_entries: usize, pending_cap: usize) -> Self {
        Self {
            slots: HashMap::new(),
            encoded: HashMap::new(),
            pending: VecDeque::new(),
            cached_bytes: 0,
            session_generation: 0,
            tick: 0,
            next_image_id: 1,
            byte_budget,
            max_entries,
            pending_cap,
        }
    }

    fn lookup(&mut self, participant_key: &str, url: &str) -> Option<CollabAvatarImage> {
        if !valid_lookup(participant_key, url) || self.max_entries == 0 {
            return None;
        }
        self.tick = self.tick.wrapping_add(1);
        let tick = self.tick;
        let changed = self
            .slots
            .get(participant_key)
            .is_some_and(|slot| slot.url != url);
        let profile_generation = if changed {
            self.slots
                .get(participant_key)
                .map(|slot| slot.profile_generation.wrapping_add(1).max(1))
                .unwrap_or(1)
        } else {
            1
        };
        if changed {
            self.remove_slot(participant_key);
        }
        if !self.slots.contains_key(participant_key) {
            self.evict_to_entry_limit(self.max_entries.saturating_sub(1));
            let image_id = IMAGE_ID_PREFIX | self.next_image_id;
            self.next_image_id = self.next_image_id.wrapping_add(1).max(1);
            self.slots.insert(
                participant_key.to_string(),
                AvatarSlot {
                    url: url.to_string(),
                    profile_generation,
                    image_id,
                    state: SlotState::Waiting,
                    last_used: tick,
                    failure_count: 0,
                    retry_at: None,
                },
            );
        }

        let needs_queue = self
            .slots
            .get(participant_key)
            .is_some_and(|slot| matches!(slot.state, SlotState::Waiting));
        if needs_queue && self.pending.len() < self.pending_cap {
            let slot = self
                .slots
                .get_mut(participant_key)
                .expect("slot was inserted above");
            slot.state = SlotState::Queued;
            self.pending.push_back(CollabAvatarFetchRequest {
                session_generation: self.session_generation,
                participant_key: participant_key.to_string(),
                profile_generation: slot.profile_generation,
                image_id: slot.image_id,
                url: slot.url.clone(),
            });
        }
        let slot = self
            .slots
            .get_mut(participant_key)
            .expect("slot was inserted above");
        slot.last_used = tick;
        match &slot.state {
            SlotState::Ready => self
                .encoded
                .get(&slot.image_id)
                .map(|encoded| CollabAvatarImage {
                    image_id: slot.image_id,
                    encoded: Arc::clone(encoded),
                }),
            _ => None,
        }
    }

    fn ready(&mut self, participant_key: &str) -> Option<CollabAvatarImage> {
        if participant_key.is_empty()
            || participant_key.len() > MAX_REGISTRY_KEY_BYTES
            || participant_key.chars().any(char::is_control)
        {
            return None;
        }
        if participant_key == ACCOUNT_AVATAR_KEY {
            let retry_url = self.slots.get_mut(participant_key).and_then(|slot| {
                let retry_due = matches!(slot.state, SlotState::Failed)
                    && slot
                        .retry_at
                        .is_some_and(|deadline| Instant::now() >= deadline);
                if matches!(slot.state, SlotState::Waiting) || retry_due {
                    slot.state = SlotState::Waiting;
                    slot.retry_at = None;
                    Some(slot.url.clone())
                } else {
                    None
                }
            });
            if let Some(url) = retry_url {
                let _ = self.lookup(participant_key, &url);
            }
        }
        self.tick = self.tick.wrapping_add(1);
        let tick = self.tick;
        let slot = self.slots.get_mut(participant_key)?;
        slot.last_used = tick;
        matches!(slot.state, SlotState::Ready)
            .then(|| {
                self.encoded
                    .get(&slot.image_id)
                    .map(|encoded| CollabAvatarImage {
                        image_id: slot.image_id,
                        encoded: Arc::clone(encoded),
                    })
            })
            .flatten()
    }

    fn take_requests(&mut self, max: usize) -> Vec<CollabAvatarFetchRequest> {
        let mut output = Vec::with_capacity(max.min(self.pending.len()));
        while output.len() < max {
            let Some(request) = self.pending.pop_front() else {
                break;
            };
            let valid = self
                .slots
                .get_mut(&request.participant_key)
                .is_some_and(|slot| {
                    if request.session_generation == self.session_generation
                        && slot.profile_generation == request.profile_generation
                        && slot.image_id == request.image_id
                        && slot.url == request.url
                        && matches!(slot.state, SlotState::Queued)
                    {
                        slot.state = SlotState::InFlight;
                        true
                    } else {
                        false
                    }
                });
            if valid {
                output.push(request);
            }
        }
        output
    }

    fn complete(&mut self, request: &CollabAvatarFetchRequest, bytes: Option<Vec<u8>>) -> bool {
        let valid = self
            .slots
            .get(&request.participant_key)
            .is_some_and(|slot| {
                request.session_generation == self.session_generation
                    && slot.profile_generation == request.profile_generation
                    && slot.image_id == request.image_id
                    && slot.url == request.url
                    && matches!(slot.state, SlotState::InFlight)
            });
        if !valid {
            return false;
        }
        let encoded = bytes.and_then(validate_encoded_avatar);
        self.tick = self.tick.wrapping_add(1);
        let slot = self
            .slots
            .get_mut(&request.participant_key)
            .expect("validated slot still exists");
        slot.last_used = self.tick;
        match encoded {
            Some(encoded) => {
                slot.failure_count = 0;
                slot.retry_at = None;
                self.cached_bytes = self.cached_bytes.saturating_add(encoded.len());
                self.encoded.insert(slot.image_id, encoded);
                slot.state = SlotState::Ready;
                self.evict_over_budget();
                self.slots
                    .get(&request.participant_key)
                    .is_some_and(|slot| matches!(slot.state, SlotState::Ready))
            }
            None => {
                slot.state = SlotState::Failed;
                if request.participant_key == ACCOUNT_AVATAR_KEY
                    && slot.failure_count < ACCOUNT_AVATAR_MAX_RETRIES
                {
                    slot.failure_count += 1;
                    slot.retry_at = Some(
                        Instant::now()
                            + account_avatar_retry_delay(slot.failure_count)
                                .min(ACCOUNT_AVATAR_MAX_RETRY_DELAY),
                    );
                } else {
                    slot.retry_at = None;
                }
                true
            }
        }
    }

    fn bytes_for(&self, image_id: u64) -> Option<Arc<[u8]>> {
        self.encoded.get(&image_id).map(Arc::clone)
    }

    fn has_background_work(&self) -> bool {
        !self.pending.is_empty()
            || self.slots.get(ACCOUNT_AVATAR_KEY).is_some_and(|slot| {
                matches!(
                    slot.state,
                    SlotState::Waiting | SlotState::Queued | SlotState::InFlight
                ) || (matches!(slot.state, SlotState::Failed) && slot.retry_at.is_some())
            })
    }

    fn install_ready(
        &mut self,
        participant_key: &str,
        source_identity: &str,
        bytes: Vec<u8>,
    ) -> bool {
        let Some(encoded) = validate_encoded_avatar(bytes) else {
            self.remove_slot(participant_key);
            return false;
        };
        let changed = self
            .slots
            .get(participant_key)
            .is_some_and(|slot| slot.url != source_identity);
        if changed {
            self.remove_slot(participant_key);
        }
        self.tick = self.tick.wrapping_add(1);
        let tick = self.tick;
        if !self.slots.contains_key(participant_key) {
            if self.max_entries == 0 {
                return false;
            }
            self.evict_to_entry_limit(self.max_entries.saturating_sub(1));
            let image_id = IMAGE_ID_PREFIX | self.next_image_id;
            self.next_image_id = self.next_image_id.wrapping_add(1).max(1);
            self.slots.insert(
                participant_key.to_string(),
                AvatarSlot {
                    url: source_identity.to_string(),
                    profile_generation: 1,
                    image_id,
                    state: SlotState::Ready,
                    last_used: tick,
                    failure_count: 0,
                    retry_at: None,
                },
            );
        }
        let slot = self
            .slots
            .get_mut(participant_key)
            .expect("ready slot was inserted above");
        slot.last_used = tick;
        slot.state = SlotState::Ready;
        slot.failure_count = 0;
        slot.retry_at = None;
        if let Some(previous) = self.encoded.insert(slot.image_id, encoded.clone()) {
            self.cached_bytes = self.cached_bytes.saturating_sub(previous.len());
        }
        self.cached_bytes = self.cached_bytes.saturating_add(encoded.len());
        self.evict_over_budget();
        self.slots
            .get(participant_key)
            .is_some_and(|slot| matches!(slot.state, SlotState::Ready))
    }

    fn begin_session_generation(&mut self, generation: u64) -> bool {
        let account_request_invalidated = self
            .slots
            .get(ACCOUNT_AVATAR_KEY)
            .is_some_and(|slot| matches!(slot.state, SlotState::Queued | SlotState::InFlight));
        let changed = self.session_generation != generation
            || !self.pending.is_empty()
            || self.slots.keys().any(|key| key != ACCOUNT_AVATAR_KEY)
            || account_request_invalidated;
        self.session_generation = generation;

        let stale_keys: Vec<String> = self
            .slots
            .keys()
            .filter(|key| key.as_str() != ACCOUNT_AVATAR_KEY)
            .cloned()
            .collect();
        for key in stale_keys {
            self.remove_slot(&key);
        }
        self.pending.clear();
        let account_retry_url = self.slots.get_mut(ACCOUNT_AVATAR_KEY).and_then(|account| {
            if matches!(account.state, SlotState::Queued | SlotState::InFlight) {
                account.state = SlotState::Waiting;
                Some(account.url.clone())
            } else {
                None
            }
        });
        if let Some(url) = account_retry_url {
            let _ = self.lookup(ACCOUNT_AVATAR_KEY, &url);
        }
        self.tick = self.tick.wrapping_add(1);
        // Keep `next_image_id` monotonic: a late decode result may still land
        // in the backend LRU, and a fresh generation must never reuse its id.
        changed
    }

    fn remove_slot(&mut self, participant_key: &str) {
        self.pending
            .retain(|request| request.participant_key != participant_key);
        if let Some(slot) = self.slots.remove(participant_key) {
            if let Some(bytes) = self.encoded.remove(&slot.image_id) {
                self.cached_bytes = self.cached_bytes.saturating_sub(bytes.len());
            }
        }
    }

    fn evict_to_entry_limit(&mut self, limit: usize) {
        while self.slots.len() > limit {
            let Some(oldest) = self
                .slots
                .iter()
                .filter(|(key, _)| key.as_str() != ACCOUNT_AVATAR_KEY)
                .min_by_key(|(_, slot)| slot.last_used)
                .map(|(key, _)| key.clone())
                .or_else(|| {
                    self.slots
                        .iter()
                        .min_by_key(|(_, slot)| slot.last_used)
                        .map(|(key, _)| key.clone())
                })
            else {
                break;
            };
            self.remove_slot(&oldest);
        }
    }

    fn evict_over_budget(&mut self) {
        while self.cached_bytes > self.byte_budget || self.slots.len() > self.max_entries {
            let Some(oldest) = self
                .slots
                .iter()
                .filter(|(key, _)| key.as_str() != ACCOUNT_AVATAR_KEY)
                .min_by_key(|(_, slot)| slot.last_used)
                .map(|(key, _)| key.clone())
                .or_else(|| {
                    self.slots
                        .iter()
                        .min_by_key(|(_, slot)| slot.last_used)
                        .map(|(key, _)| key.clone())
                })
            else {
                break;
            };
            self.remove_slot(&oldest);
        }
    }
}

static AVATARS: OnceLock<Mutex<AvatarRegistry>> = OnceLock::new();

fn avatars() -> &'static Mutex<AvatarRegistry> {
    AVATARS.get_or_init(|| Mutex::new(AvatarRegistry::new()))
}

/// Register or replace one URL obtained from the authenticated roster.
///
/// The URL never crosses into `EditorState`; widgets retain only the
/// epoch-local participant key. Invalid/absent profiles remove any stale
/// image for that participant.
pub fn register_collab_avatar_url(participant_key: &str, url: Option<&str>) -> bool {
    let Some(participant_key) = collab_registry_key(participant_key) else {
        return false;
    };
    let Ok(mut registry) = avatars().lock() else {
        return false;
    };
    let Some(url) = url else {
        registry.remove_slot(&participant_key);
        return true;
    };
    if !valid_lookup(&participant_key, url) {
        registry.remove_slot(&participant_key);
        return false;
    }
    let _ = registry.lookup(&participant_key, url);
    true
}

/// Resolve a ready avatar by opaque participant key only.
pub fn collab_avatar_image(participant_key: &str) -> Option<CollabAvatarImage> {
    let participant_key = collab_registry_key(participant_key)?;
    avatars().lock().ok()?.ready(&participant_key)
}

/// Register or clear the current authenticated account's profile image.
///
/// The account lives in a separate namespace from collaboration participant
/// keys, so an untrusted roster key cannot replace the signed-in user's image.
pub fn register_account_avatar_url(url: Option<&str>) -> bool {
    let Ok(mut registry) = avatars().lock() else {
        return false;
    };
    let Some(url) = url else {
        registry.remove_slot(ACCOUNT_AVATAR_KEY);
        return true;
    };
    if !valid_lookup(ACCOUNT_AVATAR_KEY, url) {
        registry.remove_slot(ACCOUNT_AVATAR_KEY);
        return false;
    }
    let _ = registry.lookup(ACCOUNT_AVATAR_KEY, url);
    true
}

/// Resolve the current account image after its host fetch has completed.
pub fn account_avatar_image() -> Option<CollabAvatarImage> {
    avatars().lock().ok()?.ready(ACCOUNT_AVATAR_KEY)
}

/// Install bytes returned by the authenticated serve-web avatar proxy.
///
/// `revision` is an opaque, URL-derived identity emitted by the daemon. The
/// browser never receives the underlying profile URL.
pub fn install_account_avatar_bytes(revision: &str, bytes: Vec<u8>) -> bool {
    if revision.is_empty()
        || revision.len() > MAX_ACCOUNT_REVISION_BYTES
        || !revision
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return false;
    }
    let source_identity = format!("proxy:{revision}");
    avatars()
        .lock()
        .map(|mut registry| registry.install_ready(ACCOUNT_AVATAR_KEY, &source_identity, bytes))
        .unwrap_or(false)
}

/// Begin a process-local avatar epoch for a collaboration runtime.
/// Collaboration entries are dropped on every call, including when a newly
/// constructed runtime reuses the same numeric generation. The independent
/// current-account slot survives; an in-flight account request is safely
/// requeued under the new generation.
pub fn begin_collab_avatar_generation(generation: u64) -> bool {
    avatars()
        .lock()
        .map(|mut registry| registry.begin_session_generation(generation))
        .unwrap_or(false)
}

pub fn take_collab_avatar_requests(max: usize) -> Vec<CollabAvatarFetchRequest> {
    avatars()
        .lock()
        .map(|mut registry| registry.take_requests(max))
        .unwrap_or_default()
}

/// Land one worker result if its participant generation is still current.
/// Invalid, oversized, or stale results are discarded without retaining raw
/// bytes.
pub fn complete_collab_avatar_request(
    request: &CollabAvatarFetchRequest,
    bytes: Option<Vec<u8>>,
) -> bool {
    avatars()
        .lock()
        .map(|mut registry| registry.complete(request, bytes))
        .unwrap_or(false)
}

pub fn cached_collab_avatar_bytes(image_id: u64) -> Option<Arc<[u8]>> {
    avatars().lock().ok()?.bytes_for(image_id)
}

pub fn has_pending_collab_avatar_requests() -> bool {
    avatars()
        .lock()
        .map(|registry| registry.has_background_work())
        .unwrap_or(false)
}

fn account_avatar_retry_delay(failure_count: u8) -> Duration {
    let exponent = u32::from(failure_count.saturating_sub(1).min(5));
    Duration::from_secs(1_u64 << exponent)
}

fn valid_lookup(participant_key: &str, url: &str) -> bool {
    !participant_key.is_empty()
        && participant_key.len() <= MAX_REGISTRY_KEY_BYTES
        && !participant_key.chars().any(char::is_control)
        && !url.is_empty()
        && url.len() <= MAX_AVATAR_URL_BYTES
        && url.starts_with("https://")
        && !url.chars().any(char::is_whitespace)
}

fn collab_registry_key(participant_key: &str) -> Option<String> {
    if participant_key.is_empty()
        || participant_key.len() > MAX_PARTICIPANT_KEY_BYTES
        || participant_key.chars().any(char::is_control)
    {
        return None;
    }
    Some(format!("{COLLAB_KEY_PREFIX}{participant_key}"))
}

fn validate_encoded_avatar(bytes: Vec<u8>) -> Option<Arc<[u8]>> {
    if bytes.is_empty() || bytes.len() > MAX_AVATAR_ENCODED_BYTES {
        return None;
    }
    let (width, height) = crate::image_runtime::encoded_image_dimensions(&bytes)?;
    if width > MAX_AVATAR_SOURCE_EDGE_PX
        || height > MAX_AVATAR_SOURCE_EDGE_PX
        || u64::from(width) * u64::from(height) > MAX_AVATAR_SOURCE_PIXELS
    {
        return None;
    }
    Some(Arc::from(bytes.into_boxed_slice()))
}

#[cfg(test)]
static TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
pub(crate) fn lock_collab_avatar_registry_for_tests() -> std::sync::MutexGuard<'static, ()> {
    let guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    if let Ok(mut registry) = avatars().lock() {
        *registry = AvatarRegistry::new();
    }
    guard
}

#[cfg(test)]
#[path = "collab_avatar_runtime_tests.rs"]
mod tests;
