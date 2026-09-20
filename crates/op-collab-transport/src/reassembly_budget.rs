use std::sync::{Arc, Mutex, OnceLock};

use crate::{QueueError, DEFAULT_GLOBAL_INBOUND_REASSEMBLY_BYTES};

#[derive(Debug)]
struct ReassemblyBudgetState {
    maximum: usize,
    used: usize,
}

/// An aggregate byte budget for inbound reassembly buffers.
///
/// A transfer allocates its peer-declared total on chunk 0, so per-connection
/// class caps bound only one connection at a time. This is the inbound twin of
/// [`crate::SharedQueueBudget`]: the declared total is reserved before the
/// buffer is allocated and the reservation is released by RAII only after the
/// completed bytes finish decode/drop, or when a transfer aborts or times out.
#[derive(Debug, Clone)]
pub struct SharedReassemblyBudget {
    state: Arc<Mutex<ReassemblyBudgetState>>,
}

/// An RAII reservation against a [`SharedReassemblyBudget`].
#[derive(Debug)]
#[must_use = "dropping the reservation immediately releases its byte budget"]
pub struct SharedReassemblyReservation {
    budget: SharedReassemblyBudget,
    amount: usize,
}

impl SharedReassemblyReservation {
    pub const fn amount(&self) -> usize {
        self.amount
    }
}

impl Drop for SharedReassemblyReservation {
    fn drop(&mut self) {
        self.budget.release(self.amount);
    }
}

impl SharedReassemblyBudget {
    pub fn new(maximum: usize) -> Result<Self, QueueError> {
        if maximum == 0 {
            return Err(QueueError::ByteBudget);
        }
        Ok(Self {
            state: Arc::new(Mutex::new(ReassemblyBudgetState { maximum, used: 0 })),
        })
    }

    /// The process-wide budget every connection is charged against unless its
    /// host installs a narrower one.
    ///
    /// Inbound reassembly is started deep inside the record loop, far from any
    /// host-owned session object, so the aggregate has to be reachable from the
    /// default constructors. A host that wants a tighter per-session bound
    /// passes its own budget to
    /// [`crate::SecureConnection::with_inbound_budget`].
    pub fn process_default() -> Self {
        static DEFAULT: OnceLock<SharedReassemblyBudget> = OnceLock::new();
        DEFAULT
            .get_or_init(|| {
                Self::new(DEFAULT_GLOBAL_INBOUND_REASSEMBLY_BYTES)
                    .expect("the default inbound reassembly budget is a non-zero constant")
            })
            .clone()
    }

    pub fn used(&self) -> Result<usize, QueueError> {
        self.state
            .lock()
            .map(|state| state.used)
            .map_err(|_| QueueError::Unavailable)
    }

    pub fn maximum(&self) -> Result<usize, QueueError> {
        self.state
            .lock()
            .map(|state| state.maximum)
            .map_err(|_| QueueError::Unavailable)
    }

    pub fn reserve(&self, amount: usize) -> Result<SharedReassemblyReservation, QueueError> {
        let mut state = self.state.lock().map_err(|_| QueueError::Unavailable)?;
        let next = state
            .used
            .checked_add(amount)
            .ok_or(QueueError::ByteBudget)?;
        if next > state.maximum {
            return Err(QueueError::ByteBudget);
        }
        state.used = next;
        drop(state);
        Ok(SharedReassemblyReservation {
            budget: self.clone(),
            amount,
        })
    }

    fn release(&self, amount: usize) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.used = state.used.saturating_sub(amount);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reservations_are_exact_and_release_on_drop() {
        let budget = SharedReassemblyBudget::new(10).unwrap();
        assert_eq!(budget.maximum().unwrap(), 10);
        let reservation = budget.reserve(7).unwrap();
        assert_eq!(reservation.amount(), 7);
        assert_eq!(budget.used().unwrap(), 7);
        assert!(matches!(budget.reserve(4), Err(QueueError::ByteBudget)));
        drop(reservation);
        assert_eq!(budget.used().unwrap(), 0);
        assert_eq!(budget.reserve(10).unwrap().amount(), 10);
    }

    #[test]
    fn zero_and_overflowing_budgets_fail_closed() {
        assert!(matches!(
            SharedReassemblyBudget::new(0),
            Err(QueueError::ByteBudget)
        ));
        let budget = SharedReassemblyBudget::new(usize::MAX).unwrap();
        let held = budget.reserve(usize::MAX - 1).unwrap();
        assert!(matches!(budget.reserve(2), Err(QueueError::ByteBudget)));
        drop(held);
    }

    #[test]
    fn the_process_default_is_one_shared_aggregate() {
        let first = SharedReassemblyBudget::process_default();
        let second = SharedReassemblyBudget::process_default();
        assert_eq!(
            first.maximum().unwrap(),
            DEFAULT_GLOBAL_INBOUND_REASSEMBLY_BYTES
        );
        let reservation = first.reserve(4_096).unwrap();
        assert!(second.used().unwrap() >= 4_096);
        drop(reservation);
    }
}
