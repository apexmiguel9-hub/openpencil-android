//! Per-source-address admission accounting for the pre-pairing phase.
//!
//! A relay connection performs its most expensive work — the WebSocket
//! upgrade, the hello decode, and ticket verification — before the client has
//! proven anything. The global `max_pending` and `max_auth_in_flight` ceilings
//! bound that work in aggregate but not per source, so a single host can hold
//! every slot by opening connections and then going silent until the handshake
//! deadline expires. Charging those slots to the connecting address caps how
//! much of the shared budget one source can occupy.

use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex},
};

pub(crate) struct PeerQuota {
    limit: usize,
    in_flight: Mutex<HashMap<IpAddr, usize>>,
}

impl PeerQuota {
    pub(crate) fn new(limit: usize) -> Arc<Self> {
        Arc::new(Self {
            limit: limit.max(1),
            in_flight: Mutex::new(HashMap::new()),
        })
    }

    /// Reserve one pre-pairing slot for `address`, or return `None` when that
    /// source already holds `limit` of them.
    ///
    /// The map only ever holds addresses with a live reservation and entries
    /// are removed as their last permit drops, so its size is bounded by the
    /// global pending ceiling rather than by how many addresses connect.
    pub(crate) fn try_acquire(self: &Arc<Self>, address: IpAddr) -> Option<PeerQuotaPermit> {
        let mut in_flight = self.in_flight.lock().ok()?;
        let slot = in_flight.entry(address).or_insert(0);
        if *slot >= self.limit {
            return None;
        }
        *slot += 1;
        drop(in_flight);
        Some(PeerQuotaPermit {
            quota: Arc::clone(self),
            address,
        })
    }

    #[cfg(test)]
    pub(crate) fn tracked_addresses(&self) -> usize {
        self.in_flight.lock().map_or(0, |in_flight| in_flight.len())
    }
}

pub(crate) struct PeerQuotaPermit {
    quota: Arc<PeerQuota>,
    address: IpAddr,
}

impl Drop for PeerQuotaPermit {
    fn drop(&mut self) {
        let Ok(mut in_flight) = self.quota.in_flight.lock() else {
            return;
        };
        let Some(slot) = in_flight.get_mut(&self.address) else {
            return;
        };
        *slot = slot.saturating_sub(1);
        if *slot == 0 {
            in_flight.remove(&self.address);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr};

    use super::*;

    fn v4(last: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(203, 0, 113, last))
    }

    #[test]
    fn quota_caps_slots_per_source_address() {
        let quota = PeerQuota::new(2);
        let first = quota.try_acquire(v4(1)).expect("first slot");
        let second = quota.try_acquire(v4(1)).expect("second slot");
        assert!(quota.try_acquire(v4(1)).is_none());
        drop(first);
        drop(second);
    }

    #[test]
    fn exhausting_one_source_leaves_others_admissible() {
        let quota = PeerQuota::new(1);
        let _held = quota.try_acquire(v4(1)).expect("first source");
        assert!(quota.try_acquire(v4(1)).is_none());
        assert!(quota.try_acquire(v4(2)).is_some());
        assert!(quota.try_acquire(IpAddr::V6(Ipv6Addr::LOCALHOST)).is_some());
    }

    #[test]
    fn released_permits_free_the_slot_and_drop_the_entry() {
        let quota = PeerQuota::new(1);
        let permit = quota.try_acquire(v4(1)).expect("slot");
        assert_eq!(quota.tracked_addresses(), 1);
        drop(permit);
        assert_eq!(quota.tracked_addresses(), 0);
        assert!(quota.try_acquire(v4(1)).is_some());
    }

    #[test]
    fn a_rejected_acquire_does_not_retain_an_entry() {
        let quota = PeerQuota::new(1);
        let permit = quota.try_acquire(v4(1)).expect("slot");
        assert!(quota.try_acquire(v4(1)).is_none());
        drop(permit);
        assert_eq!(quota.tracked_addresses(), 0);
    }
}
