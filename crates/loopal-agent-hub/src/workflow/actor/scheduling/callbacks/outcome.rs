use loopal_protocol::{AgentCompletion, WorkflowAttemptFailure, WorkflowFailureClass};

use crate::types::AgentExecutionRef;
use crate::workflow::scheduler::{
    ActiveAttemptPhase, AttemptKey, WorkflowSpawnFailure, WorkflowWorkerOutcome, prepare_outcome,
};
use crate::workflow::{WorkflowCoordinatorError, WorkflowOwner};

use super::super::super::WorkflowCoordinator;
use super::super::{commit, dispatch};
use super::matches_active;

pub(in crate::workflow::actor) async fn finished(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    key: AttemptKey,
    execution: AgentExecutionRef,
    outcome: WorkflowWorkerOutcome,
) -> Result<(), WorkflowCoordinatorError> {
    if !matches_active(coordinator, &owner, &key, &execution, None) {
        return Ok(());
    }
    let phase = coordinator.active[&key.attempt_id].phase;
    if matches!(
        phase,
        ActiveAttemptPhase::Interrupting | ActiveAttemptPhase::ShuttingDown
    ) {
        // A worker outcome is not proof that interrupt/shutdown cleanup
        // completed. The cleanup supervisor owns the terminal decision while
        // the attempt is stopping; otherwise a later timeout callback could
        // become stale and lose the durable ambiguity marker.
        return Ok(());
    }
    if phase != ActiveAttemptPhase::Running {
        return Ok(());
    }
    let run = coordinator.scheduler_snapshot(&owner, &key.run_id)?;
    let payload = prepare_outcome(&run, &key, outcome, &coordinator.redaction_seed).payload;
    commit::payload(
        coordinator,
        &owner,
        &run,
        payload,
        coordinator.clock.now_unix_ms(),
    )
    .await?;
    coordinator.active.remove(&key.attempt_id);
    dispatch::admit(coordinator, owner, key.run_id).await
}

pub(in crate::workflow::actor) async fn outcome_lost(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    key: AttemptKey,
    execution: AgentExecutionRef,
) -> Result<(), WorkflowCoordinatorError> {
    if !matches_active(coordinator, &owner, &key, &execution, None) {
        return Ok(());
    }
    if coordinator.active[&key.attempt_id].phase != ActiveAttemptPhase::Running {
        return Ok(());
    }
    let now = coordinator.clock.now_unix_ms();
    super::super::stop::request_failure_stop(
        coordinator,
        owner,
        key,
        execution,
        lost_outcome_failure(),
        "workflow worker outcome channel closed",
        now,
    )
    .await
}

fn lost_outcome_failure() -> WorkflowSpawnFailure {
    WorkflowSpawnFailure {
        completion: AgentCompletion::new("worker_outcome_lost", None),
        failure: WorkflowAttemptFailure {
            class: WorkflowFailureClass::AmbiguousExecution,
            reason: "workflow worker outcome channel closed".into(),
        },
    }
}
