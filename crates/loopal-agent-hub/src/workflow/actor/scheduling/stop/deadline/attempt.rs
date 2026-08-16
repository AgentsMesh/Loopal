use loopal_protocol::{
    AgentCompletion, WorkflowAttemptFailure, WorkflowEventPayload, WorkflowFailureClass,
};

use super::super::super::super::WorkflowCoordinator;
use super::super::{bound_reason, request_failure_stop};
use super::run_expired;
use crate::workflow::WorkflowCoordinatorError;
use crate::workflow::scheduler::{AttemptKey, StopDisposition, WorkflowSpawnFailure};

pub(super) async fn expire_all(
    coordinator: &mut WorkflowCoordinator,
    now: u64,
) -> Result<(), WorkflowCoordinatorError> {
    expire_pending(coordinator, now).await?;
    expire_active(coordinator, now).await
}

async fn expire_pending(
    coordinator: &mut WorkflowCoordinator,
    now: u64,
) -> Result<(), WorkflowCoordinatorError> {
    let mut attempts: Vec<_> = coordinator
        .pending
        .values()
        .filter(|pending| pending.stop.is_none() && now >= pending.deadline_unix_ms)
        .map(|pending| (pending.owner.clone(), pending.key.clone()))
        .collect();
    attempts.sort_by(|left, right| left.1.attempt_id.cmp(&right.1.attempt_id));
    for (owner, key) in attempts {
        let run = coordinator.scheduler_snapshot(&owner, &key.run_id)?;
        if run_expired(&run, now) {
            continue;
        }
        let failure = timeout_failure(&key, false);
        super::super::super::commit::payload(
            coordinator,
            &owner,
            &run,
            WorkflowEventPayload::AttemptStopRequested {
                node_id: key.node_id.clone(),
                attempt_id: key.attempt_id.clone(),
                reason: "workflow attempt preparation timed out".into(),
            },
            now,
        )
        .await?;
        if let Some(pending) = coordinator.pending.get_mut(&key.attempt_id) {
            pending.stop = Some(StopDisposition::Failed(failure));
        }
        super::super::pending::request_attempt_abort(coordinator, &owner, &key);
    }
    Ok(())
}

async fn expire_active(
    coordinator: &mut WorkflowCoordinator,
    now: u64,
) -> Result<(), WorkflowCoordinatorError> {
    let mut attempts: Vec<_> = coordinator
        .active
        .values()
        .filter(|active| active.stop.is_none() && now >= active.deadline_unix_ms)
        .map(|active| {
            (
                active.owner.clone(),
                active.key.clone(),
                active.execution.clone(),
                active.phase == crate::workflow::scheduler::ActiveAttemptPhase::Running,
            )
        })
        .collect();
    attempts.sort_by(|left, right| left.1.attempt_id.cmp(&right.1.attempt_id));
    for (owner, key, execution, entered_running) in attempts {
        let run = coordinator.scheduler_snapshot(&owner, &key.run_id)?;
        if run_expired(&run, now) {
            continue;
        }
        request_failure_stop(
            coordinator,
            owner,
            key.clone(),
            execution,
            timeout_failure(&key, entered_running),
            "workflow attempt timed out",
            now,
        )
        .await?;
    }
    Ok(())
}

fn timeout_failure(key: &AttemptKey, entered_running: bool) -> WorkflowSpawnFailure {
    WorkflowSpawnFailure {
        completion: AgentCompletion::new("workflow_timeout", None),
        failure: WorkflowAttemptFailure {
            class: if entered_running {
                WorkflowFailureClass::AmbiguousExecution
            } else {
                WorkflowFailureClass::TransientBeforeExecution
            },
            reason: bound_reason(format!("workflow attempt {} timed out", key.attempt_id)),
        },
    }
}
