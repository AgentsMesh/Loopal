use std::time::Duration;

use loopal_protocol::{AgentCompletion, WorkflowFailureClass};

use super::super::{AttemptPhase, ProductionWorkflowSpawner};
use crate::types::AgentExecutionRef;
use crate::workflow::scheduler::{
    WorkflowActivationFailure, WorkflowCleanupStatus, WorkflowSpawnFailure,
};

pub(in crate::spawn_manager::workflow) async fn activate(
    spawner: &ProductionWorkflowSpawner,
    execution: &AgentExecutionRef,
) -> Result<(), WorkflowActivationFailure> {
    let activation = {
        let mut owners = spawner.attempts.lock().await;
        let Some(attempt) = super::exact_mut(&mut owners, execution) else {
            return Err(stopped("workflow execution lease is stale"));
        };
        match attempt.phase {
            AttemptPhase::Prepared => {
                attempt.phase = AttemptPhase::Activating;
                Some((
                    attempt.control.clone(),
                    attempt.owner.clone(),
                    attempt.causation.clone(),
                    attempt.operation.clone(),
                ))
            }
            AttemptPhase::Stopping => None,
            AttemptPhase::Activating | AttemptPhase::Running => {
                return Err(WorkflowActivationFailure::Stopped(failure(
                    "workflow execution is not prepared",
                    WorkflowFailureClass::Permanent,
                )));
            }
        }
    };
    let Some((control, owner, causation, operation)) = activation else {
        return cleanup_failure(
            spawner,
            execution,
            "workflow execution stopped before agent/start",
        )
        .await;
    };
    if let Err(reason) = super::super::lifecycle_audit::append(
        spawner,
        &owner,
        &causation,
        Some(execution),
        super::super::lifecycle_audit::WorkflowAuditPhase::Activate,
    )
    .await
    {
        return cleanup_failure(spawner, execution, &reason).await;
    }
    let operation_guard = operation.lock().await;
    let still_activating = {
        let mut owners = spawner.attempts.lock().await;
        super::exact_mut(&mut owners, execution)
            .is_some_and(|attempt| attempt.phase == AttemptPhase::Activating)
    };
    if !still_activating {
        drop(operation_guard);
        return cleanup_failure(
            spawner,
            execution,
            "workflow execution stopped before agent/start",
        )
        .await;
    }
    if !spawner
        .hub
        .lock()
        .await
        .registry
        .owns_active_lease(execution)
    {
        drop(operation_guard);
        return cleanup_failure(
            spawner,
            execution,
            "workflow execution lease changed before agent/start",
        )
        .await;
    }
    match control.activate().await {
        Ok(()) => super::activation_finish::finish(spawner, execution).await,
        Err(reason) => {
            drop(operation_guard);
            cleanup_failure(spawner, execution, &reason).await
        }
    }
}

async fn cleanup_failure(
    spawner: &ProductionWorkflowSpawner,
    execution: &AgentExecutionRef,
    reason: &str,
) -> Result<(), WorkflowActivationFailure> {
    let status = super::cleanup::shutdown(spawner, execution, Duration::from_secs(5)).await;
    if status == WorkflowCleanupStatus::Confirmed {
        Err(stopped(reason))
    } else {
        Err(WorkflowActivationFailure::Uncertain(failure_reason(reason)))
    }
}

fn stopped(reason: &str) -> WorkflowActivationFailure {
    WorkflowActivationFailure::Stopped(failure(
        reason,
        WorkflowFailureClass::TransientBeforeExecution,
    ))
}

fn failure(reason: &str, class: WorkflowFailureClass) -> WorkflowSpawnFailure {
    WorkflowSpawnFailure {
        completion: AgentCompletion::new("workflow_activation_failed", Some(reason.into())),
        failure: loopal_protocol::WorkflowAttemptFailure {
            class,
            reason: reason.into(),
        },
    }
}

fn failure_reason(reason: &str) -> loopal_protocol::WorkflowAttemptFailure {
    loopal_protocol::WorkflowAttemptFailure {
        class: WorkflowFailureClass::AmbiguousExecution,
        reason: reason.into(),
    }
}
