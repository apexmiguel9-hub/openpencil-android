//! Fixed-window rate limiting for the locator publish endpoint.
//!
//! Two ceilings are enforced over the same one-second window: a per-client
//! budget keyed by the connecting peer's IP address, so one unauthenticated
//! source cannot spend everyone else's publish capacity, and a global budget
//! that still bounds total capacity when the load is spread across many
//! sources.

use std::{
    collections::{hash_map::Entry, HashMap},
    net::IpAddr,
    num::{NonZeroU32, NonZeroUsize},
    sync::Mutex,
    time::{Duration, Instant},
};

/// Width of the fixed rate-limiting window.
const WINDOW: Duration = Duration::from_secs(1);
/// Tracked client windows allowed per configured connection slot.
const TRACKED_CLIENTS_PER_CONNECTION: usize = 4;
/// Floor for the tracked-client cap, so tiny connection budgets still admit a
/// realistic number of distinct peers.
const MIN_TRACKED_CLIENTS: usize = 64;
/// Ceiling for the tracked-client cap, so a large connection budget cannot turn
/// into an unbounded map.
const MAX_TRACKED_CLIENTS: usize = 65_536;

pub(crate) struct RateLimiter {
    global_maximum: NonZeroU32,
    client_maximum: NonZeroU32,
    max_tracked_clients: usize,
    state: Mutex<RateLimiterState>,
}

struct RateLimiterState {
    global: Window,
    clients: HashMap<IpAddr, Window>,
}

#[derive(Clone, Copy)]
struct Window {
    started: Instant,
    accepted: u32,
}

impl RateLimiter {
    pub(crate) fn new(
        global_maximum: NonZeroU32,
        client_maximum: NonZeroU32,
        max_connections: NonZeroUsize,
    ) -> Self {
        Self {
            global_maximum,
            client_maximum,
            max_tracked_clients: max_tracked_clients(max_connections),
            state: Mutex::new(RateLimiterState {
                global: Window::started_at(Instant::now()),
                clients: HashMap::new(),
            }),
        }
    }

    /// Accounts one request from `client` and reports whether it is admitted.
    ///
    /// Budget is consumed only when both ceilings admit the request, so a
    /// rejected request never eats capacity from the other gate.
    pub(crate) fn allow(&self, now: Instant, client: IpAddr) -> bool {
        let Ok(mut guard) = self.state.lock() else {
            return false;
        };
        let state = &mut *guard;
        // A window older than the fixed window carries no budget, so dropping
        // it keeps the map proportional to the clients active right now.
        state.clients.retain(|_, window| !window.expired(now));
        if state.global.expired(now) {
            state.global = Window::started_at(now);
        }
        if state.global.accepted >= self.global_maximum.get() {
            return false;
        }
        // The key space is remote input: every distinct source address would
        // otherwise add an entry that nothing removes, so the map is hard-capped
        // in addition to the pruning above. The cap derives from
        // `max_connections` because only that many peers can be connected at
        // once; still being at the cap after pruning already means a flood, so
        // the request is rejected instead of growing the map.
        let saturated = state.clients.len() >= self.max_tracked_clients;
        let window = match state.clients.entry(client) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                if saturated {
                    return false;
                }
                entry.insert(Window::started_at(now))
            }
        };
        // Pruning above guarantees every surviving window is inside the current
        // window, so no per-client reset is needed here.
        if window.accepted >= self.client_maximum.get() {
            return false;
        }
        window.accepted += 1;
        state.global.accepted += 1;
        true
    }

    #[cfg(test)]
    pub(crate) fn tracked_clients(&self) -> usize {
        self.state.lock().expect("rate limiter state").clients.len()
    }

    #[cfg(test)]
    pub(crate) fn max_tracked_clients(&self) -> usize {
        self.max_tracked_clients
    }
}

impl Window {
    fn started_at(now: Instant) -> Self {
        Self {
            started: now,
            accepted: 0,
        }
    }

    fn expired(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.started) >= WINDOW
    }
}

fn max_tracked_clients(max_connections: NonZeroUsize) -> usize {
    max_connections
        .get()
        .saturating_mul(TRACKED_CLIENTS_PER_CONNECTION)
        .clamp(MIN_TRACKED_CLIENTS, MAX_TRACKED_CLIENTS)
}
