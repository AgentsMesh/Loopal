mod active;
mod cleanup;
mod delivery;
mod dependencies;
mod output;

use std::time::Duration;

use loopal_protocol::{
    AgentCompletion, WorkflowAttemptFailure, WorkflowOutput, WorkflowOutputContract,
    WorkflowPermissionCausation,
};
use tokio::sync::oneshot;

pub(in crate::workflow) use active::{
    ActiveAttempt, ActiveAttemptPhase, ActiveAttempts, AttemptKey, PendingAttempt, PendingAttempts,
    StopDisposition,
};
pub(crate) use cleanup::WorkflowCleanupStatus;
pub(in crate::workflow) use cleanup::{
    abort_local_preparation, bounded_abort_prepare, bounded_shutdown,
};
pub(in crate::workflow) use delivery::WorkflowPreparedDelivery;
pub(crate) use dependencies::WorkflowDependencyResult;
pub(in crate::workflow) use dependencies::resolve_dependency_results;
pub(in crate::workflow) use output::{prepare_outcome, prepare_spawn_failure};

use super::WorkflowOwner;
use super::worker_profile::ResolvedWorkflowWorkerProfile;
use crate::types::AgentExecutionRef;

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(crate) struct WorkflowSpawnRequest {
    pub(crate) owner: WorkflowOwner,
    pub(crate) causation: WorkflowPermissionCausation,
    pub(crate) run_goal: String,
    pub(crate) task: String,
    pub(crate) dependency_results: Vec<WorkflowDependencyResult>,
    pub(crate) worker_profile: ResolvedWorkflowWorkerProfile,
    pub(crate) output_contract: Option<WorkflowOutputContract>,
    pub(crate) completion_result_limit: u32,
    pub(crate) attempt_capability: loopal_protocol::WorkflowAttemptCapability,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WorkflowSpawnFailure {
    pub(crate) completion: AgentCompletion,
    pub(crate) failure: WorkflowAttemptFailure,
}

#[allow(dead_code)]
pub(crate) enum WorkflowActivationFailure {
    Stopped(WorkflowSpawnFailure),
    Uncertain(WorkflowAttemptFailure),
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(crate) enum WorkflowWorkerOutcome {
    Succeeded {
        completion: AgentCompletion,
        output: Option<WorkflowOutput>,
    },
    Failed(WorkflowSpawnFailure),
}

pub(crate) struct WorkflowPreparedWorker {
    pub(crate) execution: AgentExecutionRef,
    pub(crate) outcome: oneshot::Receiver<WorkflowWorkerOutcome>,
}

#[derive(Clone, Debug)]
pub(crate) struct WorkflowRecoveryAdoptionRequest {
    pub(crate) owner: WorkflowOwner,
    pub(crate) causation: WorkflowPermissionCausation,
    pub(crate) execution: AgentExecutionRef,
    pub(crate) output_contract: Option<WorkflowOutputContract>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorkflowRecoveryAdoptionError {
    MissingCustody,
    StaleExecution,
    ConflictingOwner,
    InvalidPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum WorkflowStopStatus {
    Requested,
    Stopped,
}

#[async_trait::async_trait]
pub(crate) trait WorkflowSpawner: Send + Sync + 'static {
    async fn prepare(
        &self,
        request: WorkflowSpawnRequest,
    ) -> Result<WorkflowPreparedWorker, WorkflowSpawnFailure>;

    async fn abort_prepare_and_wait(
        &self,
        causation: &WorkflowPermissionCausation,
        timeout: Duration,
    ) -> WorkflowCleanupStatus;

    async fn adopt_recovered(
        &self,
        request: WorkflowRecoveryAdoptionRequest,
    ) -> Result<WorkflowPreparedWorker, WorkflowRecoveryAdoptionError>;

    async fn activate(
        &self,
        execution: &AgentExecutionRef,
    ) -> Result<(), WorkflowActivationFailure>;

    async fn interrupt(&self, execution: &AgentExecutionRef) -> WorkflowStopStatus;

    async fn shutdown_and_wait(
        &self,
        execution: &AgentExecutionRef,
        timeout: Duration,
    ) -> WorkflowCleanupStatus;
}

pub(in crate::workflow) struct UnavailableWorkflowSpawner;

#[async_trait::async_trait]
impl WorkflowSpawner for UnavailableWorkflowSpawner {
    async fn prepare(
        &self,
        _request: WorkflowSpawnRequest,
    ) -> Result<WorkflowPreparedWorker, WorkflowSpawnFailure> {
        Err(WorkflowSpawnFailure {
            completion: AgentCompletion::new("workflow_spawner_unavailable", None),
            failure: WorkflowAttemptFailure {
                class: loopal_protocol::WorkflowFailureClass::Permanent,
                reason: "workflow spawner is unavailable".into(),
            },
        })
    }

    async fn abort_prepare_and_wait(
        &self,
        _causation: &WorkflowPermissionCausation,
        _timeout: Duration,
    ) -> WorkflowCleanupStatus {
        WorkflowCleanupStatus::Confirmed
    }

    async fn adopt_recovered(
        &self,
        _request: WorkflowRecoveryAdoptionRequest,
    ) -> Result<WorkflowPreparedWorker, WorkflowRecoveryAdoptionError> {
        Err(WorkflowRecoveryAdoptionError::MissingCustody)
    }

    async fn activate(
        &self,
        _execution: &AgentExecutionRef,
    ) -> Result<(), WorkflowActivationFailure> {
        unreachable!("unavailable workflow spawner cannot prepare a worker")
    }

    async fn interrupt(&self, _execution: &AgentExecutionRef) -> WorkflowStopStatus {
        WorkflowStopStatus::Stopped
    }

    async fn shutdown_and_wait(
        &self,
        _execution: &AgentExecutionRef,
        _timeout: Duration,
    ) -> WorkflowCleanupStatus {
        WorkflowCleanupStatus::Confirmed
    }
}

#[cfg(test)]
#[path = "availability_tests.rs"]
mod tests;
