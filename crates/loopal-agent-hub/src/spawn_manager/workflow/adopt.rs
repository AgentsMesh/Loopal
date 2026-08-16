use tokio::sync::oneshot;

use super::{AttemptPhase, ProductionWorkflowSpawner};
use crate::workflow::scheduler::{
    WorkflowPreparedWorker, WorkflowRecoveryAdoptionError, WorkflowRecoveryAdoptionRequest,
};

pub(super) async fn run(
    spawner: &ProductionWorkflowSpawner,
    request: WorkflowRecoveryAdoptionRequest,
) -> Result<WorkflowPreparedWorker, WorkflowRecoveryAdoptionError> {
    let control = {
        let hub = spawner.hub.lock().await;
        if !hub.registry.owns_active_lease(&request.execution) {
            return Err(WorkflowRecoveryAdoptionError::StaleExecution);
        }
        claim_owner(spawner, &request).await?
    };
    let (outcome_tx, outcome) = oneshot::channel();
    super::monitor::spawn(
        spawner,
        request.execution.clone(),
        control,
        outcome_tx,
        request.output_contract,
    );
    Ok(WorkflowPreparedWorker {
        execution: request.execution,
        outcome,
    })
}

async fn claim_owner(
    spawner: &ProductionWorkflowSpawner,
    request: &WorkflowRecoveryAdoptionRequest,
) -> Result<
    std::sync::Arc<crate::spawn_manager::spawn::PreparedControl>,
    WorkflowRecoveryAdoptionError,
> {
    let mut owners = spawner.attempts.lock().await;
    let attempt_id = &request.causation.attempt_id;
    if owners.by_execution.get(&request.execution) != Some(attempt_id) {
        return Err(WorkflowRecoveryAdoptionError::StaleExecution);
    }
    if owners.recovery_adopted.contains(attempt_id) {
        return Err(WorkflowRecoveryAdoptionError::MissingCustody);
    }
    let control = owners
        .by_attempt
        .get(attempt_id)
        .ok_or(WorkflowRecoveryAdoptionError::MissingCustody)?;
    validate_exact(control, request)?;
    let control = control.control.clone();
    owners.recovery_adopted.insert(attempt_id.clone());
    Ok(control)
}

fn validate_exact(
    owner: &super::AttemptOwner,
    request: &WorkflowRecoveryAdoptionRequest,
) -> Result<(), WorkflowRecoveryAdoptionError> {
    if owner.execution != request.execution {
        return Err(WorkflowRecoveryAdoptionError::StaleExecution);
    }
    if owner.owner != request.owner || owner.causation != request.causation {
        return Err(WorkflowRecoveryAdoptionError::ConflictingOwner);
    }
    if owner.phase != AttemptPhase::Running {
        return Err(WorkflowRecoveryAdoptionError::InvalidPhase);
    }
    if owner.process.is_none() || owner.process_shutdown.is_some() {
        return Err(WorkflowRecoveryAdoptionError::MissingCustody);
    }
    Ok(())
}
