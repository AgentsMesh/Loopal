use std::collections::HashMap;
use std::sync::Arc;

use tokio::task::JoinSet;

mod terminal;

use super::super::WorkflowCoordinator;
use crate::workflow::scheduler::{
    ActiveAttempt, AttemptKey, PendingAttempt, StopDisposition, WorkflowCleanupStatus,
    WorkflowSpawner, abort_local_preparation, bounded_abort_prepare, bounded_shutdown,
};
use crate::workflow::{WorkflowCoordinatorError, WorkflowOwner};

#[derive(Clone)]
struct DrainRecord {
    owner: WorkflowOwner,
    key: AttemptKey,
    kind: DrainKind,
    stop: Option<StopDisposition>,
}

#[derive(Clone, Copy)]
enum DrainKind {
    Pending,
    Active,
}

pub(super) async fn run(
    coordinator: &mut WorkflowCoordinator,
) -> Result<(), WorkflowCoordinatorError> {
    let mut cleanup = JoinSet::new();
    let mut outstanding = HashMap::new();

    for (_, attempt) in std::mem::take(&mut coordinator.pending) {
        let record = DrainRecord {
            owner: attempt.owner.clone(),
            key: attempt.key.clone(),
            kind: DrainKind::Pending,
            stop: attempt.stop.clone(),
        };
        outstanding.insert(record.key.attempt_id.clone(), record.clone());
        cleanup.spawn(clean_pending(attempt, coordinator.spawner.clone(), record));
    }
    for (_, attempt) in std::mem::take(&mut coordinator.active) {
        let record = DrainRecord {
            owner: attempt.owner.clone(),
            key: attempt.key.clone(),
            kind: DrainKind::Active,
            stop: attempt.stop.clone(),
        };
        outstanding.insert(record.key.attempt_id.clone(), record.clone());
        cleanup.spawn(clean_active(attempt, coordinator.spawner.clone(), record));
    }

    let mut first_error = None;
    let mut timed_out = false;
    while let Some(result) = cleanup.join_next().await {
        match result {
            Ok((record, status)) => {
                outstanding.remove(&record.key.attempt_id);
                timed_out |= status == WorkflowCleanupStatus::TimedOut;
                if let Err(error) = terminal::terminalize(coordinator, record, status).await {
                    first_error.get_or_insert(error);
                }
            }
            Err(error) => {
                // The lightweight records below let us still terminalize a
                // lease if a cleanup task itself panics or is cancelled.
                timed_out = true;
                tracing::error!(%error, "workflow cleanup task failed during coordinator drain");
            }
        }
    }

    for (_, record) in outstanding {
        if let Err(error) =
            terminal::terminalize(coordinator, record, WorkflowCleanupStatus::TimedOut).await
        {
            first_error.get_or_insert(error);
        }
    }

    if let Some(error) = first_error {
        Err(error)
    } else if timed_out {
        Err(WorkflowCoordinatorError::CleanupTimeout)
    } else {
        Ok(())
    }
}

async fn clean_pending(
    mut attempt: PendingAttempt,
    spawner: Arc<dyn WorkflowSpawner>,
    record: DrainRecord,
) -> (DrainRecord, WorkflowCleanupStatus) {
    let status = if let Some(waiter) = attempt.late_shutdown_waiter.take() {
        waiter.await.unwrap_or(WorkflowCleanupStatus::TimedOut)
    } else if let Some(execution) = attempt.late_execution.take() {
        bounded_shutdown(spawner, &execution).await
    } else if let Some(waiter) = attempt.abort_waiter.take() {
        waiter.await.unwrap_or(WorkflowCleanupStatus::TimedOut)
    } else {
        bounded_abort_prepare(spawner, &attempt.causation).await
    };
    if let Some(task) = attempt.prepare_abort.take() {
        abort_local_preparation(task).await;
    }
    (record, status)
}

async fn clean_active(
    mut attempt: ActiveAttempt,
    spawner: Arc<dyn WorkflowSpawner>,
    record: DrainRecord,
) -> (DrainRecord, WorkflowCleanupStatus) {
    let status = if let Some(waiter) = attempt.shutdown_waiter.take() {
        waiter.await.unwrap_or(WorkflowCleanupStatus::TimedOut)
    } else {
        bounded_shutdown(spawner, &attempt.execution).await
    };
    (record, status)
}
