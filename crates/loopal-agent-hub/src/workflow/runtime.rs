use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, Notify};
use tokio::task::JoinHandle;

use crate::Hub;

use super::{WorkflowCoordinatorError, WorkflowCoordinatorHandle, WorkflowOwner};

#[path = "runtime_cleanup.rs"]
mod cleanup;
#[path = "runtime_construction.rs"]
mod construction;
#[path = "runtime_error.rs"]
mod error;
#[path = "runtime_ticker.rs"]
mod ticker;

use cleanup::{RuntimeCleanup, clear_exact};
pub use error::WorkflowRuntimeError;
use ticker::{Ticker, start_ticker};

const DROP_CLEANUP_TIMEOUT: Duration = Duration::from_secs(10);

/// Owns the production workflow actor and its periodic deadline driver.
///
/// Construction does not authorize workflow RPCs. Call [`Self::recover_and_admit`]
/// after the managed root session has been bound and before starting the root
/// agent. Call [`Self::shutdown`] before shutting down that root process.
#[must_use = "a workflow runtime must be shut down explicitly"]
pub struct WorkflowRuntime {
    hub: Arc<Mutex<Hub>>,
    shutdown_signal: Arc<Notify>,
    handle: WorkflowCoordinatorHandle,
    actor_task: Option<JoinHandle<()>>,
    ticker: Option<Ticker>,
    admitted: bool,
    owner: Option<WorkflowOwner>,
    #[cfg(test)]
    drop_cleanup_timeout: Duration,
    #[cfg(test)]
    drop_cleanup_probe: Option<tokio::sync::oneshot::Sender<cleanup::DropCleanupOutcome>>,
}

impl WorkflowRuntime {
    /// Recover the bound root owner, start deadline driving, then authorize
    /// workflow RPCs by installing this runtime's exact coordinator handle.
    pub async fn recover_and_admit(
        &mut self,
        owner: WorkflowOwner,
    ) -> Result<usize, WorkflowRuntimeError> {
        if self.admitted {
            return Err(WorkflowRuntimeError::AlreadyAdmitted);
        }
        if self.actor_task.as_ref().is_none_or(JoinHandle::is_finished) {
            return Err(WorkflowRuntimeError::Coordinator(
                WorkflowCoordinatorError::Unavailable,
            ));
        }

        let recovered = self
            .handle
            .recover(owner.clone())
            .await
            .map_err(WorkflowRuntimeError::Coordinator)?;

        let mut hub = self.hub.lock().await;
        if let Some(current) = hub.workflow_coordinator()
            && !current.same_channel(&self.handle)
        {
            return Err(WorkflowRuntimeError::AdmissionOccupied);
        }
        let ticker = start_ticker(self.hub.clone(), self.handle.clone());
        hub.install_workflow_coordinator(self.handle.clone());
        drop(hub);

        self.ticker = Some(ticker);
        self.admitted = true;
        self.owner = Some(owner);
        Ok(recovered)
    }

    pub async fn activate_terminal_deliveries(&self) -> Result<(), WorkflowRuntimeError> {
        if !self.admitted {
            return Err(WorkflowRuntimeError::Coordinator(
                WorkflowCoordinatorError::RecoveryRequired,
            ));
        }
        let owner = self.owner.clone().ok_or(WorkflowRuntimeError::Coordinator(
            WorkflowCoordinatorError::RecoveryRequired,
        ))?;
        self.handle
            .activate_terminal_deliveries(owner.clone())
            .await
            .map_err(WorkflowRuntimeError::Coordinator)?;
        self.handle
            .resume(owner)
            .await
            .map_err(WorkflowRuntimeError::Coordinator)
    }

    /// Stop admitting new workflow RPCs while retaining ownership of the
    /// actor and ticker for a later ordered shutdown.
    pub async fn clear_admission(&mut self) {
        if self.admitted {
            clear_exact(&self.hub, &self.handle).await;
            self.admitted = false;
            self.owner = None;
        }
    }

    /// Close admission, stop the tick driver, drain coordinator workers, and
    /// join both owned tasks. Cleanup is attempted fully before an error is
    /// returned.
    pub async fn shutdown(mut self) -> Result<(), WorkflowRuntimeError> {
        let cleanup = self
            .take_cleanup()
            .expect("a live workflow runtime owns its coordinator task");
        cleanup.shutdown().await
    }

    fn take_cleanup(&mut self) -> Option<RuntimeCleanup> {
        if self.actor_task.is_none() && self.ticker.is_none() && !self.admitted {
            return None;
        }
        let cleanup = RuntimeCleanup::new(
            self.hub.clone(),
            self.shutdown_signal.clone(),
            self.handle.clone(),
            self.actor_task.take(),
            self.ticker.take(),
            self.admitted,
        );
        self.admitted = false;
        self.owner = None;
        Some(cleanup)
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "runtime_drop_tests.rs"]
mod drop_tests;

#[cfg(test)]
#[path = "runtime_admission_tests.rs"]
mod admission_tests;
