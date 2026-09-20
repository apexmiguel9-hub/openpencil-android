use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::{ConfigError, ConnectionLimits, TimeoutConfig, TransportConfig};

#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionLimitError {
    #[error("the global pending-handshake limit is full")]
    PendingHandshakesFull,
    #[error("the per-address pending-handshake limit is full")]
    PendingHandshakesPerIpFull,
    #[error("the active-connection limit is full")]
    ActiveConnectionsFull,
    #[error("the pending-handshake guard no longer owns a global seat")]
    PendingHandshakeSeatLost,
    #[error("the connection limiter is unavailable")]
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionCounts {
    /// Global pending seats in use. Every live pending guard owns one.
    pub pending_handshakes: usize,
    /// Live pending guards. Kept separate for observability even though the
    /// fail-closed accounting requires it to equal `pending_handshakes`.
    pub pending_guards: usize,
    pub active_connections: usize,
}

#[derive(Debug)]
struct PendingSlot {
    peer_ip: IpAddr,
}

#[derive(Debug)]
struct LimiterState {
    slots: HashMap<u64, PendingSlot>,
    next_slot: u64,
    pending_by_ip: HashMap<IpAddr, usize>,
    active: usize,
}

#[derive(Debug)]
struct LimiterInner {
    limits: ConnectionLimits,
    state: Mutex<LimiterState>,
}

/// Shared admission gate for an accept loop.
///
/// A host acquires a pending guard before starting any Noise work, then turns
/// it into an active guard only after ticket admission. Dropping either guard
/// releases its count, including on early-return error paths.
///
/// Every live guard continuously owns both its global and per-address seats.
/// The owner-side guarded TCP accept enforces `handshake_first_message` as a
/// real socket read deadline; a silent connection exits and drops its guard
/// before either seat is released. Accounting is never reclaimed while the
/// socket, worker, or cryptographic handshake remains alive.
#[derive(Debug, Clone)]
pub struct ConnectionLimiter {
    inner: Arc<LimiterInner>,
}

impl ConnectionLimiter {
    pub fn new(limits: ConnectionLimits) -> Result<Self, ConfigError> {
        Self::with_timeouts(limits, TimeoutConfig::default())
    }

    /// Builds a limiter after validating the connection and handshake limits
    /// that its guarded accept path will enforce.
    pub fn with_timeouts(
        limits: ConnectionLimits,
        timeouts: TimeoutConfig,
    ) -> Result<Self, ConfigError> {
        TransportConfig {
            connections: limits,
            timeouts,
            ..TransportConfig::default()
        }
        .validate()?;
        Ok(Self {
            inner: Arc::new(LimiterInner {
                limits,
                state: Mutex::new(LimiterState {
                    slots: HashMap::new(),
                    next_slot: 1,
                    pending_by_ip: HashMap::new(),
                    active: 0,
                }),
            }),
        })
    }

    pub fn try_begin_handshake(
        &self,
        peer_ip: IpAddr,
    ) -> Result<PendingHandshakeGuard, ConnectionLimitError> {
        self.try_begin_handshake_at(peer_ip, Instant::now())
    }

    /// Monotonic-clock variant of [`Self::try_begin_handshake`].
    pub fn try_begin_handshake_at(
        &self,
        peer_ip: IpAddr,
        _now: Instant,
    ) -> Result<PendingHandshakeGuard, ConnectionLimitError> {
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| ConnectionLimitError::Unavailable)?;
        if state.slots.len() >= self.inner.limits.max_pending_handshakes {
            return Err(ConnectionLimitError::PendingHandshakesFull);
        }
        let per_ip = state.pending_by_ip.get(&peer_ip).copied().unwrap_or(0);
        if per_ip >= self.inner.limits.max_pending_handshakes_per_ip {
            return Err(ConnectionLimitError::PendingHandshakesPerIpFull);
        }
        let slot = state.next_slot;
        state.next_slot = slot
            .checked_add(1)
            .ok_or(ConnectionLimitError::Unavailable)?;
        state.slots.insert(slot, PendingSlot { peer_ip });
        state.pending_by_ip.insert(peer_ip, per_ip + 1);
        drop(state);
        Ok(PendingHandshakeGuard {
            inner: Arc::clone(&self.inner),
            slot,
            peer_ip,
            held: true,
        })
    }

    pub fn counts(&self) -> Result<ConnectionCounts, ConnectionLimitError> {
        self.counts_at(Instant::now())
    }

    /// Monotonic-clock variant of [`Self::counts`]. Time never changes a live
    /// guard's accounting; only dropping or activating it releases the seat.
    pub fn counts_at(&self, _now: Instant) -> Result<ConnectionCounts, ConnectionLimitError> {
        let state = self
            .inner
            .state
            .lock()
            .map_err(|_| ConnectionLimitError::Unavailable)?;
        Ok(ConnectionCounts {
            pending_handshakes: state.slots.len(),
            pending_guards: state.slots.len(),
            active_connections: state.active,
        })
    }
}

#[derive(Debug)]
pub struct PendingHandshakeGuard {
    inner: Arc<LimiterInner>,
    slot: u64,
    peer_ip: IpAddr,
    held: bool,
}

impl PendingHandshakeGuard {
    pub const fn peer_ip(&self) -> IpAddr {
        self.peer_ip
    }

    /// Records that the peer produced its first valid handshake message.
    ///
    /// The guard already owns its seat continuously. This check lets the TCP
    /// accept path fail immediately if that invariant is ever broken.
    pub fn note_handshake_progress(&self) -> Result<(), ConnectionLimitError> {
        self.note_handshake_progress_at(Instant::now())
    }

    /// Monotonic-clock variant of [`Self::note_handshake_progress`].
    pub fn note_handshake_progress_at(&self, _now: Instant) -> Result<(), ConnectionLimitError> {
        let state = self
            .inner
            .state
            .lock()
            .map_err(|_| ConnectionLimitError::Unavailable)?;
        if !self.held || !state.slots.contains_key(&self.slot) {
            return Err(ConnectionLimitError::PendingHandshakeSeatLost);
        }
        Ok(())
    }

    /// Reports whether this connection currently occupies a global seat.
    pub fn holds_global_seat(&self) -> bool {
        self.holds_global_seat_at(Instant::now())
    }

    /// Monotonic-clock variant of [`Self::holds_global_seat`].
    pub fn holds_global_seat_at(&self, _now: Instant) -> bool {
        let Ok(state) = self.inner.state.lock() else {
            return false;
        };
        self.held && state.slots.contains_key(&self.slot)
    }

    pub fn activate(mut self) -> Result<ActiveConnectionGuard, ConnectionLimitError> {
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| ConnectionLimitError::Unavailable)?;
        if !self.held || !state.slots.contains_key(&self.slot) {
            return Err(ConnectionLimitError::PendingHandshakeSeatLost);
        }
        if state.active >= self.inner.limits.max_active_connections {
            return Err(ConnectionLimitError::ActiveConnectionsFull);
        }
        release_pending(&mut state, self.slot);
        state.active += 1;
        self.held = false;
        drop(state);
        Ok(ActiveConnectionGuard {
            inner: Arc::clone(&self.inner),
            peer_ip: self.peer_ip,
            held: true,
        })
    }
}

impl Drop for PendingHandshakeGuard {
    fn drop(&mut self) {
        if !self.held {
            return;
        }
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        release_pending(&mut state, self.slot);
        self.held = false;
    }
}

#[derive(Debug)]
pub struct ActiveConnectionGuard {
    inner: Arc<LimiterInner>,
    peer_ip: IpAddr,
    held: bool,
}

impl ActiveConnectionGuard {
    pub const fn peer_ip(&self) -> IpAddr {
        self.peer_ip
    }
}

impl Drop for ActiveConnectionGuard {
    fn drop(&mut self) {
        if !self.held {
            return;
        }
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.active = state.active.saturating_sub(1);
        self.held = false;
    }
}

fn release_pending(state: &mut LimiterState, slot: u64) {
    let Some(slot) = state.slots.remove(&slot) else {
        return;
    };
    if let Some(per_ip) = state.pending_by_ip.get_mut(&slot.peer_ip) {
        *per_ip = per_ip.saturating_sub(1);
        if *per_ip == 0 {
            state.pending_by_ip.remove(&slot.peer_ip);
        }
    }
}

#[cfg(test)]
#[path = "connection_limit_tests.rs"]
mod tests;
