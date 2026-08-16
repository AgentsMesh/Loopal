#[path = "recovery_adopt.rs"]
mod adopt;
#[path = "recovery_handshake.rs"]
mod handshake;

use loopal_protocol::{
    AgentCompletion, WorkflowAttemptFailure, WorkflowAttemptId, WorkflowAttemptState,
    WorkflowEventPayload, WorkflowFailureClass,
};

use super::super::WorkflowCoordinator;
use super::commit;
use crate::workflow::{WorkflowCoordinatorError, WorkflowOwner};

pub(super) use adopt::run as adopt;
pub(super) use handshake::run as handshake;

pub(super) async fn reconcile(
    coordinator: &mut WorkflowCoordinator,
    owner: &WorkflowOwner,
    now_unix_ms: u64,
) -> Result<(), WorkflowCoordinatorError> {
    let attempts: Vec<_> = coordinator
        .recovery_deadlines
        .iter()
        .filter(|(_, deadline)| **deadline <= now_unix_ms)
        .map(|(attempt, _)| attempt.clone())
        .collect();
    expire(coordinator, Some(owner), attempts).await
}

pub(super) async fn reconcile_expired(
    coordinator: &mut WorkflowCoordinator,
    now_unix_ms: u64,
) -> Result<(), WorkflowCoordinatorError> {
    let attempts: Vec<_> = coordinator
        .recovery_deadlines
        .iter()
        .filter(|(_, deadline)| **deadline <= now_unix_ms)
        .map(|(attempt, _)| attempt.clone())
        .collect();
    expire(coordinator, None, attempts).await
}

async fn expire(
    coordinator: &mut WorkflowCoordinator,
    owner_filter: Option<&WorkflowOwner>,
    mut attempts: Vec<WorkflowAttemptId>,
) -> Result<(), WorkflowCoordinatorError> {
    attempts.sort();
    for attempt_id in attempts {
        let Some((owner, run_id)) = find_attempt(coordinator, &attempt_id) else {
            coordinator.recovery_deadlines.remove(&attempt_id);
            continue;
        };
        if owner_filter.is_some_and(|expected| expected != &owner) {
            continue;
        }
        fail_unreconciled(coordinator, &owner, &run_id, attempt_id.clone()).await?;
        coordinator.recovery_deadlines.remove(&attempt_id);
    }
    Ok(())
}

fn find_attempt(
    coordinator: &WorkflowCoordinator,
    attempt_id: &WorkflowAttemptId,
) -> Option<(WorkflowOwner, loopal_protocol::WorkflowRunId)> {
    coordinator
        .state
        .scheduler_runs()
        .into_iter()
        .find(|(_, run)| run.attempts.iter().any(|attempt| &attempt.id == attempt_id))
        .map(|(owner, run)| (owner, run.id))
}

async fn fail_unreconciled(
    coordinator: &mut WorkflowCoordinator,
    owner: &WorkflowOwner,
    run_id: &loopal_protocol::WorkflowRunId,
    attempt_id: WorkflowAttemptId,
) -> Result<(), WorkflowCoordinatorError> {
    let run = coordinator.scheduler_snapshot(owner, run_id)?;
    let attempt = run
        .attempts
        .iter()
        .find(|attempt| attempt.id == attempt_id)
        .ok_or(WorkflowCoordinatorError::RecoveryInvalid)?;
    if attempt.state.is_terminal() {
        return Ok(());
    }
    if !matches!(
        attempt.state,
        WorkflowAttemptState::Dispatching
            | WorkflowAttemptState::Running
            | WorkflowAttemptState::Cancelling
    ) {
        return Err(WorkflowCoordinatorError::RecoveryInvalid);
    }
    let uncertain = attempt.agent.is_some() || attempt.entered_running;
    let failure = WorkflowAttemptFailure {
        class: if uncertain {
            WorkflowFailureClass::AmbiguousExecution
        } else {
            WorkflowFailureClass::Permanent
        },
        reason: if uncertain {
            "workflow attempt did not reclaim its exact execution lease before recovery grace expired"
                .into()
        } else {
            "workflow dispatch was not bound before coordinator restart".into()
        },
    };
    commit::payload(
        coordinator,
        owner,
        &run,
        WorkflowEventPayload::AttemptFailed {
            node_id: attempt.node_id.clone(),
            attempt_id,
            completion: AgentCompletion::new("workflow_recovery_unreconciled", None),
            failure,
        },
        coordinator.clock.now_unix_ms(),
    )
    .await?;
    Ok(())
}
