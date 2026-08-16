use std::time::Duration;

use loopal_protocol::WorkflowPermissionCausation;
use tokio::sync::oneshot;

use super::scheduler_support::{SpawnerEffect, TestSpawner};
use crate::types::AgentExecutionRef;
use crate::workflow::scheduler::{
    WorkflowActivationFailure, WorkflowCleanupStatus, WorkflowPreparedWorker,
    WorkflowRecoveryAdoptionError, WorkflowRecoveryAdoptionRequest, WorkflowSpawnFailure,
    WorkflowSpawnRequest, WorkflowSpawner, WorkflowStopStatus,
};

#[async_trait::async_trait]
impl WorkflowSpawner for TestSpawner {
    async fn prepare(
        &self,
        request: WorkflowSpawnRequest,
    ) -> Result<WorkflowPreparedWorker, WorkflowSpawnFailure> {
        let (response, receiver) = oneshot::channel();
        self.effects
            .send(SpawnerEffect::Prepare { request, response })
            .map_err(|_| unavailable_failure())?;
        receiver.await.map_err(|_| unavailable_failure())?
    }

    async fn abort_prepare_and_wait(
        &self,
        causation: &WorkflowPermissionCausation,
        _timeout: Duration,
    ) -> WorkflowCleanupStatus {
        let (response, receiver) = oneshot::channel();
        if self
            .effects
            .send(SpawnerEffect::AbortPrepare {
                causation: causation.clone(),
                response,
            })
            .is_err()
        {
            return WorkflowCleanupStatus::TimedOut;
        }
        receiver.await.unwrap_or(WorkflowCleanupStatus::TimedOut)
    }

    async fn adopt_recovered(
        &self,
        request: WorkflowRecoveryAdoptionRequest,
    ) -> Result<WorkflowPreparedWorker, WorkflowRecoveryAdoptionError> {
        let (response, receiver) = oneshot::channel();
        if self
            .effects
            .send(SpawnerEffect::AdoptRecovered { request, response })
            .is_err()
        {
            return Err(WorkflowRecoveryAdoptionError::MissingCustody);
        }
        receiver
            .await
            .unwrap_or(Err(WorkflowRecoveryAdoptionError::MissingCustody))
    }

    async fn activate(
        &self,
        execution: &AgentExecutionRef,
    ) -> Result<(), WorkflowActivationFailure> {
        let (response, receiver) = oneshot::channel();
        self.effects
            .send(SpawnerEffect::Activate {
                execution: execution.clone(),
                response,
            })
            .map_err(|_| WorkflowActivationFailure::Stopped(unavailable_failure()))?;
        receiver
            .await
            .unwrap_or_else(|_| Err(WorkflowActivationFailure::Stopped(unavailable_failure())))
    }

    async fn interrupt(&self, execution: &AgentExecutionRef) -> WorkflowStopStatus {
        let (response, receiver) = oneshot::channel();
        if self
            .effects
            .send(SpawnerEffect::Interrupt {
                execution: execution.clone(),
                response,
            })
            .is_err()
        {
            return WorkflowStopStatus::Stopped;
        }
        receiver.await.unwrap_or(WorkflowStopStatus::Stopped)
    }

    async fn shutdown_and_wait(
        &self,
        execution: &AgentExecutionRef,
        _timeout: Duration,
    ) -> WorkflowCleanupStatus {
        let (response, receiver) = oneshot::channel();
        if self
            .effects
            .send(SpawnerEffect::Shutdown {
                execution: execution.clone(),
                response,
            })
            .is_err()
        {
            return WorkflowCleanupStatus::TimedOut;
        }
        receiver.await.unwrap_or(WorkflowCleanupStatus::TimedOut)
    }
}

fn unavailable_failure() -> WorkflowSpawnFailure {
    WorkflowSpawnFailure {
        completion: loopal_protocol::AgentCompletion::new("test_spawner_unavailable", None),
        failure: loopal_protocol::WorkflowAttemptFailure {
            class: loopal_protocol::WorkflowFailureClass::Permanent,
            reason: "test workflow spawner observer is unavailable".into(),
        },
    }
}
