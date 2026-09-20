mod guest;
mod owner;
mod protocol;

#[cfg(test)]
mod tests;

pub use guest::RelayGuestBridge;
pub use owner::RelayOwnerBridge;
pub use protocol::RelayHandshake;

use std::fmt;

use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::error::{RelayBridgeStatus, RelayStopError};
use crate::limits::RelayLimits;

pub(crate) struct BridgeHandle {
    cancel: watch::Sender<bool>,
    status: watch::Receiver<RelayBridgeStatus>,
    task: Option<JoinHandle<()>>,
    stop_timeout: std::time::Duration,
}

impl BridgeHandle {
    pub(crate) fn new(
        cancel: watch::Sender<bool>,
        status: watch::Receiver<RelayBridgeStatus>,
        task: JoinHandle<()>,
        limits: RelayLimits,
    ) -> Self {
        Self {
            cancel,
            status,
            task: Some(task),
            stop_timeout: limits.stop,
        }
    }

    pub(crate) fn status(&self) -> RelayBridgeStatus {
        *self.status.borrow()
    }

    pub(crate) fn subscribe(&self) -> watch::Receiver<RelayBridgeStatus> {
        self.status.clone()
    }

    pub(crate) async fn stop(mut self) -> Result<(), RelayStopError> {
        let _ = self.cancel.send(true);
        let Some(mut task) = self.task.take() else {
            return Ok(());
        };
        match tokio::time::timeout(self.stop_timeout, &mut task).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(_)) => Err(RelayStopError::TaskFailed),
            Err(_) => {
                task.abort();
                let _ = task.await;
                Err(RelayStopError::Timeout)
            }
        }
    }
}

impl Drop for BridgeHandle {
    fn drop(&mut self) {
        let _ = self.cancel.send(true);
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

impl fmt::Debug for BridgeHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BridgeHandle")
            .field("status", &self.status())
            .field("task_running", &self.task.is_some())
            .finish()
    }
}
