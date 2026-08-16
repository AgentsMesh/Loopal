mod deadline;
mod effect;
pub(super) mod pending;

use loopal_protocol::{WorkflowEventPayload, WorkflowRunId};

use super::super::WorkflowCoordinator;
use super::commit;
use crate::types::AgentExecutionRef;
use crate::workflow::scheduler::{
    ActiveAttemptPhase, AttemptKey, PendingAttempt, StopDisposition, WorkflowCleanupStatus,
    WorkflowPreparedWorker, WorkflowSpawnFailure,
};
use crate::workflow::{WorkflowCoordinatorError, WorkflowOwner};

pub(super) fn begin_cancel_effects(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    run_id: WorkflowRunId,
    reason: String,
) {
    pending::mark_cancelled(coordinator, &owner, &run_id, &reason);
    pending::request_abort(coordinator, &owner, &run_id);
    let attempts: Vec<_> = coordinator
        .active
        .values_mut()
        .filter(|active| active.owner == owner && active.key.run_id == run_id)
        .map(|active| {
            active.stop = Some(StopDisposition::Cancelled(reason.clone()));
            active.shutdown_after_unix_ms = Some(
                coordinator
                    .clock
                    .now_unix_ms()
                    .saturating_add(coordinator.cancel_grace_ms),
            );
            active.phase = ActiveAttemptPhase::Interrupting;
            (active.key.clone(), active.execution.clone())
        })
        .collect();
    for (key, execution) in attempts {
        effect::interrupt(coordinator, owner.clone(), key, execution);
    }
}

pub(super) fn request_pending_attempt_abort(
    coordinator: &mut WorkflowCoordinator,
    owner: &WorkflowOwner,
    key: &AttemptKey,
) {
    pending::request_attempt_abort(coordinator, owner, key);
}

pub(super) async fn tick(
    coordinator: &mut WorkflowCoordinator,
    now_unix_ms: u64,
) -> Result<(), WorkflowCoordinatorError> {
    deadline::run(coordinator, now_unix_ms).await?;
    effect::escalate(coordinator, now_unix_ms);
    Ok(())
}

pub(super) async fn expire_run_deadline(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    run: loopal_protocol::WorkflowRunSnapshot,
    now_unix_ms: u64,
) -> Result<(), WorkflowCoordinatorError> {
    deadline::expire_run(coordinator, owner, run, now_unix_ms).await
}

pub(super) fn contain_execution(coordinator: &WorkflowCoordinator, execution: AgentExecutionRef) {
    effect::contain(coordinator, execution);
}

pub(super) fn quarantine_owner(coordinator: &mut WorkflowCoordinator, owner: &WorkflowOwner) {
    let run_ids: Vec<_> = coordinator
        .pending
        .values()
        .filter(|pending| &pending.owner == owner)
        .map(|pending| pending.key.run_id.clone())
        .collect();
    for run_id in run_ids {
        pending::request_abort(coordinator, owner, &run_id);
    }
    let ids: Vec<_> = coordinator
        .active
        .values()
        .filter(|active| &active.owner == owner)
        .map(|active| active.key.attempt_id.clone())
        .collect();
    for id in ids {
        if let Some(mut active) = coordinator.active.remove(&id) {
            let execution = active.execution.clone();
            // An escalated attempt already owns one exact shutdown
            // supervisor. If poisoning races its callback, hand the waiter
            // off to a sequential fallback instead of issuing a concurrent
            // shutdown for the same generation.
            if let Some(waiter) = active.shutdown_waiter.take() {
                effect::contain_after(coordinator, execution, waiter);
            } else {
                effect::contain(coordinator, execution);
            }
        }
    }
}

pub(super) async fn request_failure_stop(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    key: AttemptKey,
    execution: AgentExecutionRef,
    failure: WorkflowSpawnFailure,
    reason: &str,
    now_unix_ms: u64,
) -> Result<(), WorkflowCoordinatorError> {
    let run = coordinator.scheduler_snapshot(&owner, &key.run_id)?;
    commit::payload(
        coordinator,
        &owner,
        &run,
        WorkflowEventPayload::AttemptStopRequested {
            node_id: key.node_id.clone(),
            attempt_id: key.attempt_id.clone(),
            reason: bound_reason(reason.into()),
        },
        now_unix_ms,
    )
    .await?;
    let active = coordinator
        .active
        .get_mut(&key.attempt_id)
        .ok_or(WorkflowCoordinatorError::StaleExecutionLease)?;
    if active.execution != execution {
        return Ok(());
    }
    active.stop = Some(StopDisposition::Failed(failure));
    active.shutdown_after_unix_ms = Some(now_unix_ms.saturating_add(coordinator.cancel_grace_ms));
    active.phase = ActiveAttemptPhase::Interrupting;
    effect::interrupt(coordinator, owner, key, execution);
    Ok(())
}

pub(super) async fn stopped(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    key: AttemptKey,
    execution: AgentExecutionRef,
    status: WorkflowCleanupStatus,
) -> Result<(), WorkflowCoordinatorError> {
    effect::terminalize(coordinator, owner, key, execution, status).await
}

pub(super) async fn finish_preparation_stop(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    key: AttemptKey,
    prepared: Result<WorkflowPreparedWorker, WorkflowSpawnFailure>,
    run: loopal_protocol::WorkflowRunSnapshot,
    pending_attempt: PendingAttempt,
) -> Result<(), WorkflowCoordinatorError> {
    pending::finish(coordinator, owner, key, prepared, run, pending_attempt).await
}

pub(super) fn bound_reason(mut reason: String) -> String {
    reason.truncate(1_024);
    reason
}
