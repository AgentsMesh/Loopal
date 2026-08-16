use loopal_protocol::{
    AgentCompletion, WorkflowAttemptFailure, WorkflowEventPayload, WorkflowFailureClass,
    WorkflowRunSnapshot, WorkflowRunState,
};

use super::super::super::super::WorkflowCoordinator;
use super::super::request_failure_stop;
use crate::workflow::scheduler::{StopDisposition, WorkflowSpawnFailure};
use crate::workflow::{WorkflowCoordinatorError, WorkflowOwner};

pub(super) async fn expire_all(
    coordinator: &mut WorkflowCoordinator,
    now: u64,
) -> Result<(), WorkflowCoordinatorError> {
    let mut runs = coordinator.state.scheduler_runs();
    runs.sort_by(|left, right| left.1.id.cmp(&right.1.id));
    for (owner, run) in runs {
        if run.state == WorkflowRunState::Running && super::run_expired(&run, now) {
            expire(coordinator, owner, run, now).await?;
        }
    }
    Ok(())
}

pub(super) async fn expire(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    run: WorkflowRunSnapshot,
    now: u64,
) -> Result<(), WorkflowCoordinatorError> {
    stop_pending(coordinator, &owner, &run, now).await?;
    stop_active(coordinator, &owner, &run, now).await?;
    let busy = coordinator
        .pending
        .values()
        .any(|attempt| attempt.owner == owner && attempt.key.run_id == run.id)
        || coordinator
            .active
            .values()
            .any(|attempt| attempt.owner == owner && attempt.key.run_id == run.id);
    if !busy {
        let current = coordinator.scheduler_snapshot(&owner, &run.id)?;
        super::super::super::commit::payload(
            coordinator,
            &owner,
            &current,
            WorkflowEventPayload::RunDeadlineExceeded {
                failure: deadline_failure(),
            },
            now,
        )
        .await?;
    }
    Ok(())
}

async fn stop_pending(
    coordinator: &mut WorkflowCoordinator,
    owner: &WorkflowOwner,
    run: &WorkflowRunSnapshot,
    now: u64,
) -> Result<(), WorkflowCoordinatorError> {
    let mut ids: Vec<_> = coordinator
        .pending
        .values()
        .filter(|attempt| &attempt.owner == owner && attempt.key.run_id == run.id)
        .map(|attempt| attempt.key.attempt_id.clone())
        .collect();
    ids.sort();
    for id in ids {
        if coordinator.pending[&id].stop.is_none() {
            let key = coordinator.pending[&id].key.clone();
            let current = coordinator.scheduler_snapshot(owner, &run.id)?;
            super::super::super::commit::payload(
                coordinator,
                owner,
                &current,
                WorkflowEventPayload::AttemptStopRequested {
                    node_id: key.node_id,
                    attempt_id: key.attempt_id,
                    reason: "workflow run deadline exceeded".into(),
                },
                now,
            )
            .await?;
        }
        if let Some(pending) = coordinator.pending.get_mut(&id) {
            pending.stop = Some(StopDisposition::Failed(deadline_spawn_failure()));
        }
        super::super::pending::request_abort(coordinator, owner, &run.id);
    }
    Ok(())
}

async fn stop_active(
    coordinator: &mut WorkflowCoordinator,
    owner: &WorkflowOwner,
    run: &WorkflowRunSnapshot,
    now: u64,
) -> Result<(), WorkflowCoordinatorError> {
    let mut attempts: Vec<_> = coordinator
        .active
        .values()
        .filter(|attempt| &attempt.owner == owner && attempt.key.run_id == run.id)
        .map(|attempt| {
            (
                attempt.key.clone(),
                attempt.execution.clone(),
                attempt.stop.is_some(),
            )
        })
        .collect();
    attempts.sort_by(|left, right| left.0.attempt_id.cmp(&right.0.attempt_id));
    for (key, execution, stopping) in attempts {
        if stopping {
            if let Some(active) = coordinator.active.get_mut(&key.attempt_id) {
                active.stop = Some(StopDisposition::Failed(deadline_spawn_failure()));
            }
        } else {
            request_failure_stop(
                coordinator,
                owner.clone(),
                key,
                execution,
                deadline_spawn_failure(),
                "workflow run deadline exceeded",
                now,
            )
            .await?;
        }
    }
    Ok(())
}

fn deadline_spawn_failure() -> WorkflowSpawnFailure {
    WorkflowSpawnFailure {
        completion: AgentCompletion::new("workflow_run_deadline", None),
        failure: deadline_failure(),
    }
}

fn deadline_failure() -> WorkflowAttemptFailure {
    WorkflowAttemptFailure {
        class: WorkflowFailureClass::Permanent,
        reason: "workflow run deadline exceeded".into(),
    }
}
