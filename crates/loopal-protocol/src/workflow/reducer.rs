use serde::{Deserialize, Serialize};

use super::{
    WorkflowEvent, WorkflowEventPayload, WorkflowJsonValidator, WorkflowOutputValidationError,
    WorkflowRunSnapshot, WorkflowValidationError,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkflowReduceOutcome {
    Applied(Box<WorkflowRunSnapshot>),
    IgnoredStale { current_revision: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkflowReduceError {
    WrongRun,
    InvalidRunId,
    InvalidAttemptId,
    RevisionGap {
        expected: u64,
        actual: u64,
    },
    TerminalImmutable,
    Validation {
        error: WorkflowValidationError,
    },
    OutputValidation {
        error: WorkflowOutputValidationError,
    },
    IllegalTransition {
        detail: String,
    },
    UnknownNode,
    UnknownAttempt,
    AttemptMismatch,
    AttemptExists,
    AttemptsExhausted,
    InvalidCompletion,
    MissingOutput,
}

pub fn reduce_workflow_event<V: WorkflowJsonValidator>(
    run: &WorkflowRunSnapshot,
    event: &WorkflowEvent,
    json_validator: &V,
) -> Result<WorkflowReduceOutcome, WorkflowReduceError> {
    if !run.id.is_valid() || !event.run_id.is_valid() {
        return Err(WorkflowReduceError::InvalidRunId);
    }
    if event.run_id != run.id {
        return Err(WorkflowReduceError::WrongRun);
    }
    if event.revision <= run.revision {
        return Ok(WorkflowReduceOutcome::IgnoredStale {
            current_revision: run.revision,
        });
    }
    let expected = run.revision.saturating_add(1);
    if event.revision != expected {
        return Err(WorkflowReduceError::RevisionGap {
            expected,
            actual: event.revision,
        });
    }
    if run.state.is_terminal() {
        return Err(WorkflowReduceError::TerminalImmutable);
    }
    let mut next = run.clone();
    super::reducer_transition::apply(&mut next, &event.payload, json_validator)?;
    if let WorkflowEventPayload::DispatchIntended { attempt_id, .. } = &event.payload {
        next.attempts
            .iter_mut()
            .find(|attempt| &attempt.id == attempt_id)
            .expect("dispatch transition creates its exact attempt")
            .dispatched_at_unix_ms = event.occurred_at_unix_ms;
    }
    next.revision = event.revision;
    next.updated_at_unix_ms = event.occurred_at_unix_ms;
    Ok(WorkflowReduceOutcome::Applied(Box::new(next)))
}

impl From<WorkflowValidationError> for WorkflowReduceError {
    fn from(error: WorkflowValidationError) -> Self {
        Self::Validation { error }
    }
}

impl From<WorkflowOutputValidationError> for WorkflowReduceError {
    fn from(error: WorkflowOutputValidationError) -> Self {
        Self::OutputValidation { error }
    }
}

pub(crate) fn illegal(detail: &str) -> WorkflowReduceError {
    WorkflowReduceError::IllegalTransition {
        detail: detail.into(),
    }
}
