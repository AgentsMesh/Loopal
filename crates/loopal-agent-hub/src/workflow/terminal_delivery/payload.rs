use loopal_output_guard::{FinalSinkRedactionSeed, OutputGuard};
use loopal_protocol::{
    MAX_WORKFLOW_TERMINAL_CONTENT_BYTES, MAX_WORKFLOW_TERMINAL_DETAIL_BYTES,
    MAX_WORKFLOW_TERMINAL_GOAL_BYTES, WorkflowOutput, WorkflowRunSnapshot,
    WorkflowTerminalDeliveryId, WorkflowTerminalNotification, WorkflowTerminalOutcome,
    truncate_workflow_terminal_text,
};

use crate::workflow::{WorkflowCoordinatorError, WorkflowOwner};

pub(in crate::workflow) fn from_snapshot(
    owner: &WorkflowOwner,
    run: &WorkflowRunSnapshot,
    seed: &FinalSinkRedactionSeed,
) -> Result<WorkflowTerminalNotification, WorkflowCoordinatorError> {
    let guard =
        OutputGuard::new(&seed.snapshot().map_err(|_| rejected())?).map_err(|_| rejected())?;
    let run_goal = bounded(&guard, &run.spec.run_goal, MAX_WORKFLOW_TERMINAL_GOAL_BYTES);
    let (outcome, detail) = outcome(run)?;
    let detail = bounded(&guard, &detail, MAX_WORKFLOW_TERMINAL_DETAIL_BYTES);
    let outcome = match outcome {
        OutcomeKind::Succeeded => WorkflowTerminalOutcome::Succeeded {
            result: detail.clone(),
        },
        OutcomeKind::Failed(class) => WorkflowTerminalOutcome::Failed {
            class,
            reason: detail.clone(),
        },
        OutcomeKind::Cancelled => WorkflowTerminalOutcome::Cancelled {
            reason: detail.clone(),
        },
    };
    let content = bounded(
        &guard,
        &format_content(run, &detail),
        MAX_WORKFLOW_TERMINAL_CONTENT_BYTES,
    );
    let notification = WorkflowTerminalNotification {
        delivery_id: WorkflowTerminalDeliveryId::new(
            owner.session_id.clone(),
            run.id.clone(),
            run.revision,
        ),
        state: run.state,
        run_goal,
        outcome,
        content,
    };
    notification.validate().map_err(|_| rejected())?;
    Ok(notification)
}

enum OutcomeKind {
    Succeeded,
    Failed(loopal_protocol::WorkflowFailureClass),
    Cancelled,
}

fn outcome(run: &WorkflowRunSnapshot) -> Result<(OutcomeKind, String), WorkflowCoordinatorError> {
    match run.state {
        loopal_protocol::WorkflowRunState::Succeeded => Ok((
            OutcomeKind::Succeeded,
            run.result
                .as_ref()
                .map(format_output)
                .unwrap_or_else(|| "Workflow completed without a result.".into()),
        )),
        loopal_protocol::WorkflowRunState::Failed => {
            let failure = run.failure.as_ref().ok_or_else(rejected)?;
            Ok((OutcomeKind::Failed(failure.class), failure.reason.clone()))
        }
        loopal_protocol::WorkflowRunState::Cancelled => Ok((
            OutcomeKind::Cancelled,
            "Workflow was cancelled before completion.".into(),
        )),
        _ => Err(rejected()),
    }
}

fn format_output(output: &WorkflowOutput) -> String {
    match output {
        WorkflowOutput::Text(text) => text.clone(),
        WorkflowOutput::Json(value) => serde_json::to_string_pretty(value)
            .unwrap_or_else(|_| "Workflow completed with an unencodable JSON result.".into()),
    }
}

fn format_content(run: &WorkflowRunSnapshot, detail: &str) -> String {
    match run.state {
        loopal_protocol::WorkflowRunState::Succeeded => format!(
            "Workflow {} completed successfully.\n\nGoal: {}\n\nResult:\n{}",
            run.id, run.spec.run_goal, detail
        ),
        loopal_protocol::WorkflowRunState::Failed => format!(
            "Workflow {} failed.\n\nGoal: {}\n\nReason:\n{}",
            run.id, run.spec.run_goal, detail
        ),
        loopal_protocol::WorkflowRunState::Cancelled => format!(
            "Workflow {} was cancelled.\n\nGoal: {}\n\n{}",
            run.id, run.spec.run_goal, detail
        ),
        _ => unreachable!("terminal payload requires terminal state"),
    }
}

fn bounded(guard: &OutputGuard, value: &str, max_bytes: usize) -> String {
    let redacted = guard.redact_text(value).into_inner();
    truncate_workflow_terminal_text(&redacted, max_bytes)
}

fn rejected() -> WorkflowCoordinatorError {
    WorkflowCoordinatorError::Encoding("workflow terminal payload rejected".into())
}
