use loopal_protocol::WorkflowEventPayload;

use super::super::super::WorkflowCoordinator;
use super::super::commit;
use super::unique_lease;
use crate::workflow::command::WorkflowCommand;
use crate::workflow::scheduler::{
    ActiveAttempt, ActiveAttemptPhase, AttemptKey, WorkflowPreparedWorker,
};
use crate::workflow::{WorkflowCoordinatorError, WorkflowOwner};

pub(super) async fn bind_and_activate(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    key: AttemptKey,
    worker: WorkflowPreparedWorker,
    run: loopal_protocol::WorkflowRunSnapshot,
    deadline_unix_ms: u64,
) -> Result<(), WorkflowCoordinatorError> {
    if !unique_lease(coordinator, &worker.execution) {
        coordinator.contain_execution(worker.execution);
        coordinator.poison_owner(owner);
        return Err(WorkflowCoordinatorError::InvalidExecutionLease);
    }
    let execution = worker.execution;
    if let Err(error) = commit::payload(
        coordinator,
        &owner,
        &run,
        WorkflowEventPayload::AttemptBound {
            node_id: key.node_id.clone(),
            attempt_id: key.attempt_id.clone(),
            agent: execution.address.clone(),
        },
        coordinator.clock.now_unix_ms(),
    )
    .await
    {
        coordinator.contain_execution(execution.clone());
        return Err(error);
    }
    coordinator.active.insert(
        key.attempt_id.clone(),
        ActiveAttempt {
            owner: owner.clone(),
            key: key.clone(),
            execution: execution.clone(),
            outcome: Some(worker.outcome),
            outcome_waiter: None,
            shutdown_waiter: None,
            deadline_unix_ms,
            shutdown_after_unix_ms: None,
            phase: ActiveAttemptPhase::Activating,
            stop: None,
        },
    );
    let spawner = coordinator.spawner.clone();
    let callbacks = coordinator.callbacks.clone();
    tokio::spawn(async move {
        let result = spawner.activate(&execution).await;
        let Some(callbacks) = callbacks.upgrade() else {
            return;
        };
        let _ = callbacks
            .send(WorkflowCommand::WorkerActivated {
                owner,
                key,
                execution,
                result,
            })
            .await;
    });
    Ok(())
}
