mod abort;

use loopal_protocol::WorkflowEventPayload;

use super::super::super::WorkflowCoordinator;
use super::super::commit;
use super::effect;
use crate::workflow::scheduler::{
    ActiveAttempt, ActiveAttemptPhase, AttemptKey, PendingAttempt, StopDisposition,
    WorkflowCleanupStatus, WorkflowPreparedWorker, WorkflowSpawnFailure,
};
use crate::workflow::{WorkflowCoordinatorError, WorkflowOwner};

pub(super) fn request_abort(
    coordinator: &mut WorkflowCoordinator,
    owner: &WorkflowOwner,
    run_id: &loopal_protocol::WorkflowRunId,
) {
    abort::request_run(coordinator, owner, run_id);
}

pub(super) fn request_attempt_abort(
    coordinator: &mut WorkflowCoordinator,
    owner: &WorkflowOwner,
    key: &AttemptKey,
) {
    abort::request_attempt(coordinator, owner, key);
}

pub(super) fn mark_cancelled(
    coordinator: &mut WorkflowCoordinator,
    owner: &WorkflowOwner,
    run_id: &loopal_protocol::WorkflowRunId,
    reason: &str,
) {
    for pending in coordinator
        .pending
        .values_mut()
        .filter(|pending| &pending.owner == owner && &pending.key.run_id == run_id)
    {
        pending.stop = Some(StopDisposition::Cancelled(reason.into()));
    }
}

pub(super) async fn finish(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    key: AttemptKey,
    prepared: Result<WorkflowPreparedWorker, WorkflowSpawnFailure>,
    run: loopal_protocol::WorkflowRunSnapshot,
    pending: PendingAttempt,
) -> Result<(), WorkflowCoordinatorError> {
    match prepared {
        Err(_) => terminalize_unprepared(coordinator, owner, key, run, pending).await,
        Ok(worker) if super::super::callbacks::unique_lease(coordinator, &worker.execution) => {
            bind_and_stop(coordinator, owner, key, run, pending, worker).await
        }
        Ok(worker) => {
            coordinator.contain_execution(worker.execution);
            coordinator.poison_owner(owner);
            Err(WorkflowCoordinatorError::InvalidExecutionLease)
        }
    }
}

pub(in crate::workflow::actor) async fn terminalize_after_abort(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    key: AttemptKey,
    run: loopal_protocol::WorkflowRunSnapshot,
    pending: PendingAttempt,
    status: WorkflowCleanupStatus,
) -> Result<(), WorkflowCoordinatorError> {
    if coordinator.state.is_poisoned(&owner) {
        return Ok(());
    }
    let stop = if status == WorkflowCleanupStatus::TimedOut {
        StopDisposition::Failed(effect::cleanup_timeout_failure(
            "workflow worker preparation cleanup timed out",
        ))
    } else {
        pending.stop.clone().unwrap_or_else(|| {
            StopDisposition::Cancelled("cancelled before worker preparation completed".into())
        })
    };
    let next = commit::payload(
        coordinator,
        &owner,
        &run,
        effect::terminal_payload(&run, &key, stop, &coordinator.redaction_seed),
        coordinator.clock.now_unix_ms(),
    )
    .await?;
    if next.state == loopal_protocol::WorkflowRunState::Running {
        super::super::dispatch::admit(coordinator, owner, key.run_id).await?;
    }
    Ok(())
}

async fn bind_and_stop(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    key: AttemptKey,
    run: loopal_protocol::WorkflowRunSnapshot,
    pending: PendingAttempt,
    worker: WorkflowPreparedWorker,
) -> Result<(), WorkflowCoordinatorError> {
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
        coordinator.contain_execution(execution);
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
            deadline_unix_ms: pending.deadline_unix_ms,
            shutdown_after_unix_ms: Some(
                coordinator
                    .clock
                    .now_unix_ms()
                    .saturating_add(coordinator.cancel_grace_ms),
            ),
            phase: ActiveAttemptPhase::Interrupting,
            stop: pending.stop.clone().or_else(|| {
                Some(StopDisposition::Cancelled(
                    "cancelled before worker preparation completed".into(),
                ))
            }),
        },
    );
    effect::interrupt(coordinator, owner, key, execution);
    Ok(())
}

async fn terminalize_unprepared(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    key: AttemptKey,
    run: loopal_protocol::WorkflowRunSnapshot,
    pending: PendingAttempt,
) -> Result<(), WorkflowCoordinatorError> {
    let stop = pending.stop.clone().unwrap_or_else(|| {
        StopDisposition::Cancelled("cancelled before worker preparation completed".into())
    });
    let payload = effect::terminal_payload(&run, &key, stop, &coordinator.redaction_seed);
    let next = commit::payload(
        coordinator,
        &owner,
        &run,
        payload,
        coordinator.clock.now_unix_ms(),
    )
    .await?;
    if next.state == loopal_protocol::WorkflowRunState::Running {
        super::super::dispatch::admit(coordinator, owner, key.run_id).await?;
    }
    Ok(())
}
