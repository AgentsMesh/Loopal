use loopal_protocol::{AgentCompletion, WorkflowEventPayload, WorkflowFailureClass};

use super::super::super::WorkflowCoordinator;
use super::super::{commit, dispatch};
use super::matches_active;
use crate::types::AgentExecutionRef;
use crate::workflow::command::WorkflowCommand;
use crate::workflow::scheduler::{
    ActiveAttemptPhase, AttemptKey, WorkflowActivationFailure, WorkflowSpawnFailure,
    prepare_spawn_failure,
};
use crate::workflow::{WorkflowCoordinatorError, WorkflowOwner};

pub(in crate::workflow::actor) async fn activated(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    key: AttemptKey,
    execution: AgentExecutionRef,
    result: Result<(), WorkflowActivationFailure>,
) -> Result<(), WorkflowCoordinatorError> {
    if !matches_active(
        coordinator,
        &owner,
        &key,
        &execution,
        Some(ActiveAttemptPhase::Activating),
    ) {
        return Ok(());
    }
    match result {
        Ok(()) => mark_running(coordinator, owner, key, execution).await,
        Err(WorkflowActivationFailure::Stopped(failure)) => {
            let run = coordinator.scheduler_snapshot(&owner, &key.run_id)?;
            let payload =
                prepare_spawn_failure(&run, &key, failure, &coordinator.redaction_seed).payload;
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
        Err(WorkflowActivationFailure::Uncertain(mut failure)) => {
            failure.class = WorkflowFailureClass::AmbiguousExecution;
            let now = coordinator.clock.now_unix_ms();
            super::super::stop::request_failure_stop(
                coordinator,
                owner,
                key,
                execution,
                WorkflowSpawnFailure {
                    completion: AgentCompletion::new("activation_uncertain", None),
                    failure,
                },
                "workflow worker activation was uncertain",
                now,
            )
            .await
        }
    }
}

async fn mark_running(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    key: AttemptKey,
    execution: AgentExecutionRef,
) -> Result<(), WorkflowCoordinatorError> {
    let run = coordinator.scheduler_snapshot(&owner, &key.run_id)?;
    let now = coordinator.clock.now_unix_ms();
    let next = commit::payload(
        coordinator,
        &owner,
        &run,
        WorkflowEventPayload::AttemptRunning {
            node_id: key.node_id.clone(),
            attempt_id: key.attempt_id.clone(),
        },
        now,
    )
    .await?;
    let active = coordinator.active.get_mut(&key.attempt_id).unwrap();
    active.phase = ActiveAttemptPhase::Running;
    active.deadline_unix_ms = now.saturating_add(next.spec.limits.attempt_timeout_ms);
    spawn_outcome_waiter(coordinator, owner, key, execution);
    Ok(())
}

pub(in crate::workflow::actor) fn spawn_outcome_waiter(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    key: AttemptKey,
    execution: AgentExecutionRef,
) {
    let Some(outcome) = coordinator
        .active
        .get_mut(&key.attempt_id)
        .and_then(|active| active.outcome.take())
    else {
        return;
    };
    let callbacks = coordinator.callbacks.clone();
    let attempt_id = key.attempt_id.clone();
    let waiter = tokio::spawn(async move {
        let command = match outcome.await {
            Ok(outcome) => WorkflowCommand::WorkerFinished {
                owner,
                key,
                execution,
                outcome,
            },
            Err(_) => WorkflowCommand::WorkerOutcomeLost {
                owner,
                key,
                execution,
            },
        };
        let Some(callbacks) = callbacks.upgrade() else {
            return;
        };
        let _ = callbacks.send(command).await;
    });
    if let Some(active) = coordinator.active.get_mut(&attempt_id) {
        active.outcome_waiter = Some(waiter);
    }
}
