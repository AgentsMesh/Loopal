use loopal_protocol::{WorkflowAttemptState, WorkflowEventPayload};

use super::super::super::WorkflowCoordinator;
use super::super::{callbacks, commit};
use crate::workflow::recovery::{WorkflowAttemptReconnect, WorkflowAttemptReconnectResponse};
use crate::workflow::scheduler::{
    ActiveAttempt, ActiveAttemptPhase, AttemptKey, WorkflowPreparedDelivery,
    WorkflowRecoveryAdoptionError, WorkflowRecoveryAdoptionRequest,
};
use crate::workflow::{WorkflowCoordinatorError, WorkflowOwner};

pub(crate) async fn run(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    request: WorkflowAttemptReconnect,
) -> Result<WorkflowAttemptReconnectResponse, WorkflowCoordinatorError> {
    validate_owner(coordinator, &owner)?;
    let causation = &request.causation;
    let deadline = coordinator
        .recovery_deadlines
        .get(&causation.attempt_id)
        .copied()
        .ok_or(WorkflowCoordinatorError::StaleExecutionLease)?;
    if coordinator.clock.now_unix_ms() >= deadline
        || coordinator
            .recovered_adoptions
            .contains(&causation.attempt_id)
    {
        return Err(WorkflowCoordinatorError::StaleExecutionLease);
    }
    let mut run = coordinator.scheduler_snapshot(&owner, &causation.run_id)?;
    let attempt = run
        .attempts
        .iter()
        .find(|attempt| attempt.id == causation.attempt_id)
        .ok_or(WorkflowCoordinatorError::RecoveryInvalid)?;
    if attempt.node_id != causation.node_id
        || attempt.state.is_terminal()
        || !request.capability.matches_digest(attempt.capability_digest)
        || attempt
            .agent
            .as_ref()
            .is_some_and(|agent| agent != &request.execution.address)
    {
        return Err(WorkflowCoordinatorError::InvalidExecutionLease);
    }
    if !matches!(
        attempt.state,
        WorkflowAttemptState::Dispatching | WorkflowAttemptState::Running
    ) {
        return Err(WorkflowCoordinatorError::InvalidExecutionLease);
    }
    let prior_state = attempt.state;
    let dispatched_at_unix_ms = attempt.dispatched_at_unix_ms;
    let key = AttemptKey {
        run_id: causation.run_id.clone(),
        node_id: causation.node_id.clone(),
        attempt_id: causation.attempt_id.clone(),
    };
    let output_contract =
        (causation.node_id == run.spec.output_node).then(|| run.spec.output_contract.clone());
    let worker = coordinator
        .spawner
        .adopt_recovered(WorkflowRecoveryAdoptionRequest {
            owner: owner.clone(),
            causation: causation.clone(),
            execution: request.execution.clone(),
            output_contract,
        })
        .await
        .map_err(map_adoption_error)?;
    if worker.execution != request.execution
        || !callbacks::unique_lease(coordinator, &worker.execution)
    {
        coordinator.contain_execution(worker.execution);
        coordinator.poison_owner(owner);
        return Err(WorkflowCoordinatorError::InvalidExecutionLease);
    }
    let delivery = WorkflowPreparedDelivery::new(Ok(worker), coordinator.spawner.clone());
    if attempt.agent.is_none() {
        run = commit::payload(
            coordinator,
            &owner,
            &run,
            WorkflowEventPayload::AttemptBound {
                node_id: causation.node_id.clone(),
                attempt_id: causation.attempt_id.clone(),
                agent: request.execution.address.clone(),
            },
            coordinator.clock.now_unix_ms(),
        )
        .await?;
    }
    if prior_state == WorkflowAttemptState::Dispatching {
        run = commit::payload(
            coordinator,
            &owner,
            &run,
            WorkflowEventPayload::AttemptRunning {
                node_id: causation.node_id.clone(),
                attempt_id: causation.attempt_id.clone(),
            },
            coordinator.clock.now_unix_ms(),
        )
        .await?;
    }
    let worker = delivery
        .into_result()
        .expect("recovery custody contains a prepared worker");
    let execution = worker.execution;
    coordinator.active.insert(
        key.attempt_id.clone(),
        ActiveAttempt {
            owner: owner.clone(),
            key: key.clone(),
            execution: execution.clone(),
            outcome: Some(worker.outcome),
            outcome_waiter: None,
            shutdown_waiter: None,
            deadline_unix_ms: dispatched_at_unix_ms
                .saturating_add(run.spec.limits.attempt_timeout_ms),
            shutdown_after_unix_ms: None,
            phase: ActiveAttemptPhase::Running,
            stop: None,
        },
    );
    callbacks::spawn_outcome_waiter(coordinator, owner, key, execution.clone());
    coordinator.recovery_deadlines.remove(&causation.attempt_id);
    coordinator
        .recovered_adoptions
        .insert(causation.attempt_id.clone());
    Ok(WorkflowAttemptReconnectResponse {
        execution,
        attempt_state: if prior_state == WorkflowAttemptState::Dispatching {
            WorkflowAttemptState::Running
        } else {
            prior_state
        },
    })
}

fn map_adoption_error(error: WorkflowRecoveryAdoptionError) -> WorkflowCoordinatorError {
    match error {
        WorkflowRecoveryAdoptionError::ConflictingOwner => {
            WorkflowCoordinatorError::InvalidExecutionLease
        }
        WorkflowRecoveryAdoptionError::MissingCustody
        | WorkflowRecoveryAdoptionError::StaleExecution
        | WorkflowRecoveryAdoptionError::InvalidPhase => {
            WorkflowCoordinatorError::StaleExecutionLease
        }
    }
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
