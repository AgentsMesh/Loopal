use loopal_protocol::{
    WorkflowAttemptState, WorkflowWorkerHandshakeDisposition, WorkflowWorkerHandshakeResponse,
};

use super::super::super::WorkflowCoordinator;
use crate::workflow::recovery::WorkflowAttemptReconnect;
use crate::workflow::scheduler::ActiveAttemptPhase;
use crate::workflow::{WorkflowCoordinatorError, WorkflowOwner};

/// Validate a worker startup proof against the coordinator's live attempt
/// ownership. Recovered workers use the existing single-use adoption path;
/// current workers are only acknowledged, so no running transition is
/// duplicated by reconnect.
pub(in crate::workflow::actor::scheduling) async fn run(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    request: WorkflowAttemptReconnect,
) -> Result<WorkflowWorkerHandshakeResponse, WorkflowCoordinatorError> {
    validate_owner(coordinator, &owner)?;
    if !request.causation.is_valid() || !super::super::callbacks::valid_lease(&request.execution) {
        return Err(WorkflowCoordinatorError::InvalidExecutionLease);
    }

    if coordinator
        .recovery_deadlines
        .contains_key(&request.causation.attempt_id)
    {
        let adopted = super::adopt(coordinator, owner, request).await?;
        return Ok(WorkflowWorkerHandshakeResponse {
            disposition: WorkflowWorkerHandshakeDisposition::Recovered,
            attempt_state: adopted.attempt_state,
        });
    }

    if coordinator
        .recovered_adoptions
        .contains(&request.causation.attempt_id)
    {
        return Err(WorkflowCoordinatorError::StaleExecutionLease);
    }

    let run = coordinator.scheduler_snapshot(&owner, &request.causation.run_id)?;
    let attempt = run
        .attempts
        .iter()
        .find(|attempt| attempt.id == request.causation.attempt_id)
        .ok_or(WorkflowCoordinatorError::RecoveryInvalid)?;
    if attempt.node_id != request.causation.node_id
        || attempt.state.is_terminal()
        || !request.capability.matches_digest(attempt.capability_digest)
        || attempt.agent.as_ref() != Some(&request.execution.address)
    {
        return Err(WorkflowCoordinatorError::InvalidExecutionLease);
    }

    let active = coordinator
        .active
        .get(&request.causation.attempt_id)
        .ok_or(WorkflowCoordinatorError::StaleExecutionLease)?;
    if active.owner != owner
        || active.key.run_id != request.causation.run_id
        || active.key.node_id != request.causation.node_id
        || active.key.attempt_id != request.causation.attempt_id
        || active.execution != request.execution
        || !matches!(
            active.phase,
            ActiveAttemptPhase::Activating | ActiveAttemptPhase::Running
        )
    {
        return Err(WorkflowCoordinatorError::StaleExecutionLease);
    }
    if !matches!(
        attempt.state,
        WorkflowAttemptState::Dispatching | WorkflowAttemptState::Running
    ) {
        return Err(WorkflowCoordinatorError::InvalidExecutionLease);
    }

    Ok(WorkflowWorkerHandshakeResponse {
        disposition: WorkflowWorkerHandshakeDisposition::Fresh,
        attempt_state: attempt.state,
    })
}

fn validate_owner(
    coordinator: &WorkflowCoordinator,
    owner: &WorkflowOwner,
) -> Result<(), WorkflowCoordinatorError> {
    if !coordinator.mode.executes() || !owner.is_valid() {
        return Err(WorkflowCoordinatorError::InvalidOwner);
    }
    if coordinator.state.is_poisoned(owner) {
        return Err(WorkflowCoordinatorError::OwnerPoisoned);
    }
    if !coordinator.state.is_recovered(owner) {
        return Err(WorkflowCoordinatorError::RecoveryRequired);
    }
    Ok(())
}

#[cfg(test)]
#[path = "recovery_handshake_tests.rs"]
mod tests;
