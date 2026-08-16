#[path = "dispatch_reservation.rs"]
mod reservation;

use loopal_protocol::{
    WorkflowEventPayload, WorkflowNodeState, WorkflowRunId, WorkflowRunSnapshot, WorkflowRunState,
};

use super::super::WorkflowCoordinator;
use super::commit;
use crate::workflow::{WorkflowCoordinatorError, WorkflowOwner};

pub(super) async fn admit(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    run_id: WorkflowRunId,
) -> Result<(), WorkflowCoordinatorError> {
    validate(coordinator, &owner, &run_id)?;
    let mut run = coordinator.scheduler_snapshot(&owner, &run_id)?;
    crate::workflow::worker_profile::validate_spec_profiles(&run.spec)?;
    if run.state == WorkflowRunState::Validated {
        let now = coordinator.clock.now_unix_ms();
        run = commit::payload(
            coordinator,
            &owner,
            &run,
            WorkflowEventPayload::RunStarted,
            now,
        )
        .await?;
    }
    if run.state != WorkflowRunState::Running {
        return Ok(());
    }
    let now = coordinator.clock.now_unix_ms();
    if deadline(&run, now) {
        super::stop::expire_run_deadline(coordinator, owner, run, now).await?;
        return Err(WorkflowCoordinatorError::RunDeadlineExceeded);
    }
    dispatch_ready(coordinator, owner, run).await
}

async fn dispatch_ready(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    mut run: WorkflowRunSnapshot,
) -> Result<(), WorkflowCoordinatorError> {
    let slots = run.spec.limits.max_parallel as usize
        - run
            .attempts
            .iter()
            .filter(|attempt| !attempt.state.is_terminal())
            .count();
    let ready: Vec<_> = run
        .nodes
        .iter()
        .filter(|node| node.state == WorkflowNodeState::Ready)
        .take(slots)
        .map(|node| node.id.clone())
        .collect();
    for node_id in ready {
        run = reservation::reserve(coordinator, &owner, run, node_id).await?;
        if run.state != WorkflowRunState::Running {
            break;
        }
    }
    Ok(())
}

fn validate(
    coordinator: &WorkflowCoordinator,
    owner: &WorkflowOwner,
    run_id: &WorkflowRunId,
) -> Result<(), WorkflowCoordinatorError> {
    if !coordinator.mode.executes() {
        return Err(WorkflowCoordinatorError::Disabled);
    }
    if !owner.is_valid() {
        return Err(WorkflowCoordinatorError::InvalidOwner);
    }
    if !run_id.is_valid() {
        return Err(WorkflowCoordinatorError::InvalidRunId);
    }
    if coordinator.state.is_poisoned(owner) {
        return Err(WorkflowCoordinatorError::OwnerPoisoned);
    }
    if !coordinator.state.is_recovered(owner) {
        return Err(WorkflowCoordinatorError::RecoveryRequired);
    }
    Ok(())
}

fn deadline(run: &WorkflowRunSnapshot, now: u64) -> bool {
    now >= run
        .created_at_unix_ms
        .saturating_add(run.spec.limits.run_deadline_ms)
}
