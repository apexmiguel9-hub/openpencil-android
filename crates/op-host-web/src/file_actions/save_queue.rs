//! Small destination-agnostic state machine for serialized Web saves.
//!
//! The browser daemon owns one bound file path. Letting multiple XHR writes
//! race can therefore allow an older response to land after a newer one. This
//! queue keeps one active launch plus only the latest pending launch.

pub struct LatestSaveQueue<T> {
    active: bool,
    pending: Option<T>,
}

impl<T> Default for LatestSaveQueue<T> {
    fn default() -> Self {
        Self {
            active: false,
            pending: None,
        }
    }
}

impl<T> LatestSaveQueue<T> {
    /// Start immediately when idle; otherwise replace the pending launch.
    pub fn enqueue(&mut self, launch: T) -> Option<T> {
        if self.active {
            self.pending = Some(launch);
            None
        } else {
            self.active = true;
            Some(launch)
        }
    }

    /// Complete the active launch and return the latest queued launch, if any.
    pub fn finish(&mut self) -> Option<T> {
        match self.pending.take() {
            Some(next) => Some(next),
            None => {
                self.active = false;
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LatestSaveQueue;

    #[test]
    fn serializes_launches_and_keeps_only_the_latest_pending_save() {
        let mut queue = LatestSaveQueue::default();

        assert_eq!(queue.enqueue("first"), Some("first"));
        assert_eq!(queue.enqueue("superseded"), None);
        assert_eq!(queue.enqueue("latest"), None);
        assert_eq!(queue.finish(), Some("latest"));
        assert_eq!(queue.finish(), None);
        assert_eq!(queue.enqueue("after-idle"), Some("after-idle"));
    }
}
