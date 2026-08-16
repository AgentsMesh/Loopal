use loopal_protocol::{AgentCompletion, WorkflowEventPayload, WorkflowFailureClass};

use super::super::super::WorkflowCoordinator;
use super::super::{commit, dispatch};
use crate::types::AgentExecutionRef;
use crate::workflow::command::WorkflowCommand;
use crate::workflow::scheduler::{
    ActiveAttemptPhase, AttemptKey, StopDisposition, WorkflowCleanupStatus, WorkflowSpawnFailure,
    WorkflowStopStatus, bounded_shutdown, prepare_spawn_failure,
};
use crate::workflow::{WorkflowCoordinatorError, WorkflowOwner};

pub(super) fn contain(coordinator: &WorkflowCoordinator, execution: AgentExecutionRef) {
    let spawner = coordinator.spawner.clone();
    tokio::spawn(async move {
        bounded_shutdown(spawner, &execution).await;
    });
}

pub(super) fn contain_after(
    coordinator: &WorkflowCoordinator,
    execution: AgentExecutionRef,
    existing: tokio::task::JoinHandle<WorkflowCleanupStatus>,
) {
    let spawner = coordinator.spawner.clone();
    tokio::spawn(async move {
        // A poison path may race the callback from an already-running
        // supervisor. Wait for that supervisor before issuing the fallback
        // shutdown so exact-lease adapters never see concurrent requests.
        let _ = existing.await;
        bounded_shutdown(spawner, &execution).await;
    });
}

pub(super) fn interrupt(
    coordinator: &WorkflowCoordinator,
    owner: WorkflowOwner,
    key: AttemptKey,
    execution: AgentExecutionRef,
) {
    let spawner = coordinator.spawner.clone();
    let callbacks = coordinator.callbacks.clone();
    tokio::spawn(async move {
        if spawner.interrupt(&execution).await == WorkflowStopStatus::Stopped
            && let Some(callbacks) = callbacks.upgrade()
        {
            let _ = callbacks
                .send(WorkflowCommand::WorkerStopped {
                    owner,
                    key,
                    execution,
                    status: WorkflowCleanupStatus::Confirmed,
                })
                .await;
        }
    });
}

pub(super) fn escalate(coordinator: &mut WorkflowCoordinator, now: u64) {
    let attempts: Vec<_> = coordinator
        .active
        .values_mut()
        .filter(|active| {
            active.phase == ActiveAttemptPhase::Interrupting
                && active
                    .shutdown_after_unix_ms
                    .is_some_and(|deadline| now >= deadline)
        })
        .map(|active| {
            active.phase = ActiveAttemptPhase::ShuttingDown;
            (
                active.owner.clone(),
                active.key.clone(),
                active.execution.clone(),
            )
        })
        .collect();
    for (owner, key, execution) in attempts {
        let attempt_id = key.attempt_id.clone();
        let waiter = spawn_shutdown(coordinator, owner, key, execution);
        if let Some(active) = coordinator.active.get_mut(&attempt_id) {
            active.shutdown_waiter = Some(waiter);
        }
    }
}

pub(super) async fn terminalize(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    key: AttemptKey,
    execution: AgentExecutionRef,
    status: WorkflowCleanupStatus,
) -> Result<(), WorkflowCoordinatorError> {
    let Some(active) = coordinator.active.get(&key.attempt_id) else {
        return Ok(());
    };
    if active.owner != owner || active.key != key || active.execution != execution {
        return Ok(());
    }
    let mut stop = active
        .stop
        .clone()
        .unwrap_or_else(|| StopDisposition::Failed(lost_worker()));
    if status == WorkflowCleanupStatus::TimedOut {
        stop = StopDisposition::Failed(cleanup_timeout_failure(
            "workflow worker shutdown timed out",
        ));
    }
    let run = coordinator.scheduler_snapshot(&owner, &key.run_id)?;
    let payload = terminal_payload(&run, &key, stop, &coordinator.redaction_seed);
    commit::payload(
        coordinator,
        &owner,
        &run,
        payload,
        coordinator.clock.now_unix_ms(),
    )
    .await?;
    coordinator.active.remove(&key.attempt_id);
    dispatch::admit(coordinator, owner, key.run_id).await
}

fn spawn_shutdown(
    coordinator: &WorkflowCoordinator,
    owner: WorkflowOwner,
    key: AttemptKey,
    execution: AgentExecutionRef,
) -> tokio::task::JoinHandle<WorkflowCleanupStatus> {
    let spawner = coordinator.spawner.clone();
    let callbacks = coordinator.callbacks.clone();
    tokio::spawn(async move {
        let status = bounded_shutdown(spawner, &execution).await;
        if let Some(callbacks) = callbacks.upgrade() {
            let _ = callbacks
                .send(WorkflowCommand::WorkerStopped {
                    owner,
                    key,
                    execution,
                    status,
                })
                .await;
        }
        status
    })
}

pub(super) fn terminal_payload(
    run: &loopal_protocol::WorkflowRunSnapshot,
    key: &AttemptKey,
    stop: StopDisposition,
    redaction_seed: &loopal_output_guard::FinalSinkRedactionSeed,
) -> WorkflowEventPayload {
    match stop {
        StopDisposition::Cancelled(reason) => WorkflowEventPayload::AttemptCancelled {
            node_id: key.node_id.clone(),
            attempt_id: key.attempt_id.clone(),
            reason,
        },
        StopDisposition::Failed(failure) => {
            prepare_spawn_failure(run, key, failure, redaction_seed).payload
        }
    }
}

fn lost_worker() -> WorkflowSpawnFailure {
    WorkflowSpawnFailure {
        completion: AgentCompletion::new("worker_stopped", None),
        failure: loopal_protocol::WorkflowAttemptFailure {
            class: WorkflowFailureClass::AmbiguousExecution,
            reason: "workflow worker stopped without a terminal outcome".into(),
        },
    }
}

pub(super) fn cleanup_timeout_failure(reason: &str) -> WorkflowSpawnFailure {
    WorkflowSpawnFailure {
        completion: AgentCompletion::new("workflow_cleanup_timeout", None),
        failure: loopal_protocol::WorkflowAttemptFailure {
            class: WorkflowFailureClass::AmbiguousExecution,
            reason: reason.into(),
        },
    }
}
