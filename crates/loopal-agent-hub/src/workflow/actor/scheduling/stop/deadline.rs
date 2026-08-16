mod attempt;
mod run_deadline;

use loopal_protocol::WorkflowRunSnapshot;

use super::super::super::WorkflowCoordinator;
use crate::workflow::WorkflowCoordinatorError;

pub(super) async fn run(
    coordinator: &mut WorkflowCoordinator,
    now: u64,
) -> Result<(), WorkflowCoordinatorError> {
    run_deadline::expire_all(coordinator, now).await?;
    attempt::expire_all(coordinator, now).await
}

pub(super) async fn expire_run(
    coordinator: &mut WorkflowCoordinator,
    owner: crate::workflow::WorkflowOwner,
    run: WorkflowRunSnapshot,
    now: u64,
) -> Result<(), WorkflowCoordinatorError> {
    run_deadline::expire(coordinator, owner, run, now).await
}

pub(super) fn run_expired(run: &WorkflowRunSnapshot, now: u64) -> bool {
    now >= run
        .created_at_unix_ms
        .saturating_add(run.spec.limits.run_deadline_ms)
}
