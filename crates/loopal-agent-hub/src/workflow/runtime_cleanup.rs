use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, Notify};
use tokio::task::JoinHandle;

use super::ticker::{self, Ticker};
use super::{WorkflowCoordinatorHandle, WorkflowRuntimeError};
use crate::Hub;

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DropCleanupOutcome {
    Graceful,
    Escalated,
}

#[cfg(not(test))]
type DropCleanupProbe = ();
#[cfg(test)]
type DropCleanupProbe = tokio::sync::oneshot::Sender<DropCleanupOutcome>;

pub(super) struct RuntimeCleanup {
    hub: Arc<Mutex<Hub>>,
    shutdown_signal: Arc<Notify>,
    handle: WorkflowCoordinatorHandle,
    actor_task: Option<JoinHandle<()>>,
    ticker: Option<Ticker>,
    admitted: bool,
    settled: bool,
}

impl RuntimeCleanup {
    pub(super) fn new(
        hub: Arc<Mutex<Hub>>,
        shutdown_signal: Arc<Notify>,
        handle: WorkflowCoordinatorHandle,
        actor_task: Option<JoinHandle<()>>,
        ticker: Option<Ticker>,
        admitted: bool,
    ) -> Self {
        Self {
            hub,
            shutdown_signal,
            handle,
            actor_task,
            ticker,
            admitted,
            settled: false,
        }
    }

    pub(super) async fn shutdown(mut self) -> Result<(), WorkflowRuntimeError> {
        self.graceful().await
    }

    pub(super) fn spawn_supervisor(self, timeout: Duration, probe: Option<DropCleanupProbe>) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            let mut cleanup = self;
            cleanup.escalate("Tokio runtime is unavailable during workflow runtime drop");
            report(probe, false);
            return;
        };
        runtime.spawn(async move {
            let mut cleanup = self;
            let outcome = match tokio::time::timeout(timeout, cleanup.graceful()).await {
                Ok(Ok(())) => true,
                Ok(Err(error)) => {
                    cleanup.escalate(&format!("graceful workflow cleanup failed: {error}"));
                    false
                }
                Err(_) => {
                    cleanup.escalate("graceful workflow cleanup timed out");
                    false
                }
            };
            report(probe, outcome);
        });
    }

    async fn graceful(&mut self) -> Result<(), WorkflowRuntimeError> {
        ticker::request_stop(&mut self.ticker);
        if self.admitted {
            clear_exact(&self.hub, &self.handle).await;
            self.admitted = false;
        }
        let ticker_result = ticker::join(&mut self.ticker).await;
        let coordinator_result = self
            .handle
            .shutdown()
            .await
            .map_err(WorkflowRuntimeError::Coordinator);
        let actor_result = join_actor(&mut self.actor_task).await;
        self.settled = true;

        actor_result?;
        ticker_result?;
        coordinator_result
    }

    fn escalate(&mut self, reason: &str) {
        ticker::abort(&mut self.ticker);
        if let Some(actor) = self.actor_task.take() {
            actor.abort();
        }
        self.clear_exact_now();
        self.shutdown_signal.notify_one();
        self.settled = true;
        tracing::error!(
            reason,
            "workflow runtime cleanup failed closed; Hub shutdown requested"
        );
    }

    fn clear_exact_now(&mut self) {
        if !self.admitted {
            return;
        }
        let Ok(mut hub) = self.hub.try_lock() else {
            return;
        };
        if hub
            .workflow_coordinator()
            .is_some_and(|current| current.same_channel(&self.handle))
        {
            hub.clear_workflow_coordinator();
        }
        self.admitted = false;
    }
}

impl Drop for super::WorkflowRuntime {
    fn drop(&mut self) {
        let Some(cleanup) = self.take_cleanup() else {
            return;
        };
        #[cfg(test)]
        let timeout = self.drop_cleanup_timeout;
        #[cfg(not(test))]
        let timeout = super::DROP_CLEANUP_TIMEOUT;
        #[cfg(test)]
        let probe = self.drop_cleanup_probe.take();
        #[cfg(not(test))]
        let probe = None;
        cleanup.spawn_supervisor(timeout, probe);
    }
}

impl Drop for RuntimeCleanup {
    fn drop(&mut self) {
        if !self.settled {
            self.escalate("workflow cleanup supervisor was cancelled before completion");
        }
    }
}

pub(super) async fn clear_exact(hub: &Arc<Mutex<Hub>>, handle: &WorkflowCoordinatorHandle) {
    let mut hub = hub.lock().await;
    if hub
        .workflow_coordinator()
        .is_some_and(|current| current.same_channel(handle))
    {
        hub.clear_workflow_coordinator();
    }
}

async fn join_actor(actor: &mut Option<JoinHandle<()>>) -> Result<(), WorkflowRuntimeError> {
    let result = match actor.as_mut() {
        Some(task) => task.await.map_err(|error| WorkflowRuntimeError::TaskJoin {
            task: "workflow coordinator",
            message: error.to_string(),
        }),
        None => Ok(()),
    };
    actor.take();
    result
}

#[cfg(not(test))]
fn report(_probe: Option<DropCleanupProbe>, _graceful: bool) {}

#[cfg(test)]
fn report(probe: Option<DropCleanupProbe>, graceful: bool) {
    if let Some(probe) = probe {
        let outcome = if graceful {
            DropCleanupOutcome::Graceful
        } else {
            DropCleanupOutcome::Escalated
        };
        let _ = probe.send(outcome);
    }
}
