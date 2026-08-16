mod adopt;
mod control;
mod lifecycle_audit;
mod monitor;
mod outcome;
mod pre_abort;
mod preparation_owner;
mod prepare;
mod spawn_spec;
mod worker_prompt;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use loopal_protocol::{WorkflowAttemptId, WorkflowPermissionCausation};
use tokio::sync::{Mutex, Notify};

use crate::hub::Hub;
use crate::spawn_manager::spawn::{PreparedControl, WorkflowProcessOwner};
use crate::types::AgentExecutionRef;
use crate::workflow::scheduler::{
    WorkflowActivationFailure, WorkflowCleanupStatus, WorkflowPreparedWorker,
    WorkflowRecoveryAdoptionError, WorkflowRecoveryAdoptionRequest, WorkflowSpawnFailure,
    WorkflowSpawnRequest, WorkflowSpawner, WorkflowStopStatus,
};
use preparation_owner::PreparationOwner;

#[derive(Clone)]
pub(crate) struct ProductionWorkflowSpawner {
    hub: Arc<Mutex<Hub>>,
    shutdown_signal: Arc<Notify>,
    attempts: Arc<Mutex<AttemptOwners>>,
    changed: Arc<Notify>,
}

#[derive(Default)]
struct AttemptOwners {
    pre_aborted: HashMap<WorkflowAttemptId, Vec<WorkflowPermissionCausation>>,
    preparing: HashMap<WorkflowAttemptId, Arc<PreparationOwner>>,
    by_attempt: HashMap<WorkflowAttemptId, AttemptOwner>,
    by_execution: HashMap<AgentExecutionRef, WorkflowAttemptId>,
    recovery_adopted: HashSet<WorkflowAttemptId>,
}

struct AttemptOwner {
    owner: crate::workflow::WorkflowOwner,
    causation: WorkflowPermissionCausation,
    execution: AgentExecutionRef,
    control: Arc<PreparedControl>,
    process: Option<WorkflowProcessOwner>,
    process_shutdown: Option<tokio::sync::watch::Receiver<bool>>,
    cleanup_registered: bool,
    operation: Arc<Mutex<()>>,
    phase: AttemptPhase,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AttemptPhase {
    Prepared,
    Activating,
    Running,
    Stopping,
}

impl ProductionWorkflowSpawner {
    pub(crate) fn new(hub: Arc<Mutex<Hub>>, shutdown_signal: Arc<Notify>) -> Arc<Self> {
        Arc::new(Self {
            hub,
            shutdown_signal,
            attempts: Arc::new(Mutex::new(AttemptOwners::default())),
            changed: Arc::new(Notify::new()),
        })
    }

    async fn finish_owner(&self, execution: &AgentExecutionRef) {
        let mut attempts = self.attempts.lock().await;
        if !remove_exact_owner(&mut attempts, execution) {
            return;
        }
        drop(attempts);
        self.changed.notify_waiters();
    }

    pub(super) async fn cleanup_orphaned_preparation(
        &self,
        causation: &WorkflowPermissionCausation,
    ) -> bool {
        {
            let owners = self.attempts.lock().await;
            if owners
                .preparing
                .get(&causation.attempt_id)
                .is_some_and(|owner| owner.causation == *causation)
                || owners
                    .by_attempt
                    .get(&causation.attempt_id)
                    .is_some_and(|owner| owner.causation == *causation)
            {
                return true;
            }
        }
        let execution = {
            let hub = self.hub.lock().await;
            hub.registry.workflow_execution(causation)
        };
        let Some(execution) = execution else {
            return true;
        };
        let (connection, mcp) = {
            let mut hub = self.hub.lock().await;
            let connection = hub.registry.exact_connection(&execution);
            hub.clear_permission_grants(&execution);
            hub.spawn_registry.unregister_exact(&execution);
            hub.registry.unregister_exact(&execution);
            (connection, hub.mcp_service.clone())
        };
        mcp.on_agent_detach(&execution).await;
        if let Some(connection) = connection {
            let _ = tokio::time::timeout(Duration::from_secs(2), connection.close()).await;
        }
        let mut owners = self.attempts.lock().await;
        if let Some(tombstones) = owners.pre_aborted.get_mut(&causation.attempt_id) {
            tombstones.retain(|current| current != causation);
            if tombstones.is_empty() {
                owners.pre_aborted.remove(&causation.attempt_id);
            }
        }
        drop(owners);
        self.changed.notify_waiters();
        false
    }
}

fn remove_exact_owner(owners: &mut AttemptOwners, execution: &AgentExecutionRef) -> bool {
    let Some(attempt) = owners.by_execution.remove(execution) else {
        return false;
    };
    let exact = owners
        .by_attempt
        .get(&attempt)
        .is_some_and(|owner| owner.execution == *execution);
    if exact {
        owners.by_attempt.remove(&attempt);
        owners.recovery_adopted.remove(&attempt);
    }
    true
}

#[async_trait::async_trait]
impl WorkflowSpawner for ProductionWorkflowSpawner {
    async fn prepare(
        &self,
        request: WorkflowSpawnRequest,
    ) -> Result<WorkflowPreparedWorker, WorkflowSpawnFailure> {
        prepare::run(self, request).await
    }

    async fn abort_prepare_and_wait(
        &self,
        causation: &WorkflowPermissionCausation,
        timeout: Duration,
    ) -> WorkflowCleanupStatus {
        control::abort_prepare(self, causation, timeout).await
    }

    async fn adopt_recovered(
        &self,
        request: WorkflowRecoveryAdoptionRequest,
    ) -> Result<WorkflowPreparedWorker, WorkflowRecoveryAdoptionError> {
        adopt::run(self, request).await
    }

    async fn activate(
        &self,
        execution: &AgentExecutionRef,
    ) -> Result<(), WorkflowActivationFailure> {
        control::activate(self, execution).await
    }

    async fn interrupt(&self, execution: &AgentExecutionRef) -> WorkflowStopStatus {
        control::interrupt(self, execution).await
    }

    async fn shutdown_and_wait(
        &self,
        execution: &AgentExecutionRef,
        timeout: Duration,
    ) -> WorkflowCleanupStatus {
        control::shutdown(self, execution, timeout).await
    }
}

#[cfg(test)]
#[path = "workflow_tests.rs"]
mod tests;
