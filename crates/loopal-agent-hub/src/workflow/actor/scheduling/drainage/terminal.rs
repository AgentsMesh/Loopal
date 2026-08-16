use loopal_protocol::{
    AgentCompletion, WorkflowAttemptFailure, WorkflowEventPayload, WorkflowFailureClass,
};

use super::super::commit;
use super::{DrainKind, DrainRecord};
use crate::workflow::WorkflowCoordinatorError;
use crate::workflow::actor::WorkflowCoordinator;
use crate::workflow::scheduler::{
    StopDisposition, WorkflowCleanupStatus, WorkflowSpawnFailure, prepare_spawn_failure,
};

pub(super) async fn terminalize(
    coordinator: &mut WorkflowCoordinator,
    record: DrainRecord,
    status: WorkflowCleanupStatus,
) -> Result<(), WorkflowCoordinatorError> {
    let Some(run) = coordinator
        .state
        .owned_snapshot(&record.owner, &record.key.run_id)
    else {
        return Ok(());
    };
    if run.state.is_terminal() {
        return Ok(());
    }
    let payload = terminal_payload(&run, &record, status, &coordinator.redaction_seed);
    commit::payload(
        coordinator,
        &record.owner,
        &run,
        payload,
        coordinator.clock.now_unix_ms(),
    )
    .await?;
    Ok(())
}

fn terminal_payload(
    run: &loopal_protocol::WorkflowRunSnapshot,
    record: &DrainRecord,
    status: WorkflowCleanupStatus,
    redaction_seed: &loopal_output_guard::FinalSinkRedactionSeed,
) -> WorkflowEventPayload {
    match status {
        WorkflowCleanupStatus::TimedOut => {
            let reason = match record.kind {
                DrainKind::Pending => "workflow worker preparation cleanup timed out",
                DrainKind::Active => "workflow worker shutdown timed out",
            };
            prepare_spawn_failure(run, &record.key, cleanup_failure(reason), redaction_seed).payload
        }
        WorkflowCleanupStatus::Confirmed => match record.stop.clone() {
            Some(StopDisposition::Cancelled(reason))
                if run.state == loopal_protocol::WorkflowRunState::Cancelling =>
            {
                WorkflowEventPayload::AttemptCancelled {
                    node_id: record.key.node_id.clone(),
                    attempt_id: record.key.attempt_id.clone(),
                    reason,
                }
            }
            Some(StopDisposition::Failed(failure)) => {
                prepare_spawn_failure(run, &record.key, no_retry_failure(failure), redaction_seed)
                    .payload
            }
            _ => {
                prepare_spawn_failure(
                    run,
                    &record.key,
                    shutdown_failure(record.kind),
                    redaction_seed,
                )
                .payload
            }
        },
    }
}

fn cleanup_failure(reason: &str) -> WorkflowSpawnFailure {
    WorkflowSpawnFailure {
        completion: AgentCompletion::new("workflow_cleanup_timeout", None),
        failure: WorkflowAttemptFailure {
            class: WorkflowFailureClass::AmbiguousExecution,
            reason: reason.into(),
        },
    }
}

fn shutdown_failure(kind: DrainKind) -> WorkflowSpawnFailure {
    WorkflowSpawnFailure {
        completion: AgentCompletion::new("workflow_shutdown", None),
        failure: WorkflowAttemptFailure {
            class: match kind {
                DrainKind::Pending => WorkflowFailureClass::Permanent,
                DrainKind::Active => WorkflowFailureClass::AmbiguousExecution,
            },
            reason: match kind {
                DrainKind::Pending => "workflow coordinator shut down before worker execution",
                DrainKind::Active => "workflow coordinator shut down an active worker",
            }
            .into(),
        },
    }
}

fn no_retry_failure(mut failure: WorkflowSpawnFailure) -> WorkflowSpawnFailure {
    if failure.failure.class == WorkflowFailureClass::TransientBeforeExecution {
        failure.failure.class = WorkflowFailureClass::Permanent;
    }
    failure
}
