use super::super::super::WorkflowCoordinator;
use super::super::stop::pending;
use crate::workflow::command::WorkflowCommand;
use crate::workflow::scheduler::{
    AttemptKey, WorkflowCleanupStatus, abort_local_preparation, bounded_shutdown,
};
use crate::workflow::{WorkflowCoordinatorError, WorkflowOwner};

pub(in crate::workflow::actor) async fn preparation_aborted(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    key: AttemptKey,
    status: WorkflowCleanupStatus,
) -> Result<(), WorkflowCoordinatorError> {
    let Some(current) = coordinator.pending.get(&key.attempt_id) else {
        return Ok(());
    };
    if current.owner != owner || current.key != key || !current.abort_requested {
        return Ok(());
    }
    // A prepared lease won the race and is already under exact shutdown
    // custody. The abort acknowledgement only describes preparation; it must
    // not terminalize the run before that late lease is confirmed stopped.
    if current.late_execution.is_some() {
        return Ok(());
    }
    let prepare_task = coordinator
        .pending
        .get_mut(&key.attempt_id)
        .and_then(|pending| pending.prepare_abort.take());
    let preparation_finished = prepare_task.as_ref().is_some_and(|task| task.is_finished());
    // Cancelling the local task drops a queued `WorkflowPreparedDelivery` and
    // invokes its exact-lease containment guard. Production preparation also
    // owns process and registration guards for cancellation before delivery.
    if let Some(task) = prepare_task {
        abort_local_preparation(task).await;
    }
    if coordinator.state.is_poisoned(&owner) {
        // Poisoning closes the owner admission, so no durable terminal event
        // can be appended. Release the tombstone and its waiter custody here
        // instead of leaving shutdown drainage to re-terminalize it.
        coordinator.pending.remove(&key.attempt_id);
        return Ok(());
    }
    let Some(pending) = coordinator.pending.get_mut(&key.attempt_id) else {
        return Ok(());
    };
    if pending.owner != owner || pending.key != key || pending.late_execution.is_some() {
        return Ok(());
    }
    pending.abort_status = Some(status);
    // If the preparation task had already finished, its prepared result and
    // delivery-finished marker may still be queued ahead of this callback.
    // Keep the tombstone until one of those markers is observed. A task that
    // was still running has been joined and cannot publish a future result.
    // Preserve a delivery marker that was already observed before the abort
    // acknowledgement. Only an unfinished preparation task needs the marker
    // synthesized here after its local task has been joined.
    pending.delivery_finished |= !preparation_finished;
    if pending.delivery_finished {
        enqueue_abort_finalization(coordinator, owner, key);
    }
    Ok(())
}

pub(in crate::workflow::actor) async fn preparation_abort_settled(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    key: AttemptKey,
) -> Result<(), WorkflowCoordinatorError> {
    let Some(pending) = coordinator.pending.get(&key.attempt_id) else {
        return Ok(());
    };
    if pending.owner != owner || pending.key != key || !pending.abort_requested {
        return Ok(());
    }
    if pending.late_execution.is_some() {
        return Ok(());
    }
    if !pending.delivery_finished {
        return Ok(());
    }
    let Some(status) = pending.abort_status else {
        return Ok(());
    };
    let tombstone = coordinator
        .pending
        .remove(&key.attempt_id)
        .expect("validated pending preparation tombstone exists");
    if coordinator.state.is_poisoned(&owner) {
        return Ok(());
    }
    let run = coordinator.scheduler_snapshot(&owner, &key.run_id)?;
    pending::terminalize_after_abort(coordinator, owner, key, run, tombstone, status).await
}

pub(in crate::workflow::actor) fn enqueue_abort_finalization(
    coordinator: &WorkflowCoordinator,
    owner: WorkflowOwner,
    key: AttemptKey,
) {
    let callbacks = coordinator.callbacks.clone();
    tokio::spawn(async move {
        let Some(callbacks) = callbacks.upgrade() else {
            return;
        };
        let _ = callbacks
            .send(WorkflowCommand::FinalizePreparationAbort { owner, key })
            .await;
    });
}

pub(in crate::workflow::actor) async fn late_preparation_shutdown(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    key: AttemptKey,
    execution: crate::types::AgentExecutionRef,
    status: WorkflowCleanupStatus,
) -> Result<(), WorkflowCoordinatorError> {
    let Some(current) = coordinator.pending.get(&key.attempt_id) else {
        return Ok(());
    };
    if current.owner != owner
        || current.key != key
        || current.late_execution.as_ref() != Some(&execution)
    {
        return Ok(());
    }
    let mut pending = coordinator
        .pending
        .remove(&key.attempt_id)
        .expect("validated late preparation custody exists");
    // The late lease has one containment supervisor. Whether it confirmed or
    // timed out, resolve the coordinator tombstone exactly once; an overlapping
    // preparation-abort callback is stale after this point.
    pending.prepare_abort.take();
    pending.abort_waiter.take();
    pending.late_shutdown_waiter.take();
    let run = coordinator.scheduler_snapshot(&owner, &key.run_id)?;
    pending::terminalize_after_abort(coordinator, owner, key, run, pending, status).await
}

pub(in crate::workflow::actor) fn contain_late_preparation(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    key: AttemptKey,
    worker: crate::workflow::scheduler::WorkflowPreparedWorker,
) {
    let attempt_id = key.attempt_id.clone();
    let execution = worker.execution.clone();
    // The outcome receiver is intentionally dropped after the exact execution
    // is registered for shutdown. It must not be allowed to complete a
    // cancelled attempt while containment is in flight.
    drop(worker.outcome);
    let waiter = contain_late_execution(coordinator, owner, key, execution);
    coordinator
        .pending
        .get_mut(&attempt_id)
        .expect("late preparation tombstone exists before containment")
        .late_shutdown_waiter = Some(waiter);
}

fn contain_late_execution(
    coordinator: &WorkflowCoordinator,
    owner: WorkflowOwner,
    key: AttemptKey,
    execution: crate::types::AgentExecutionRef,
) -> tokio::task::JoinHandle<WorkflowCleanupStatus> {
    let spawner = coordinator.spawner.clone();
    let callbacks = coordinator.callbacks.clone();
    tokio::spawn(async move {
        let status = bounded_shutdown(spawner, &execution).await;
        if let Some(callbacks) = callbacks.upgrade() {
            let _ = callbacks
                .send(WorkflowCommand::LatePreparationShutdown {
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
