use serde::{Deserialize, Serialize};

use super::{WorkflowAttemptFailure, WorkflowAttemptSnapshot, WorkflowFailureClass};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRetryDisposition {
    Automatic,
    ExplicitOnly,
    Never,
}

pub fn classify_workflow_retry(
    attempt: &WorkflowAttemptSnapshot,
    failure: &WorkflowAttemptFailure,
    attempts_used: usize,
    max_attempts: u32,
) -> WorkflowRetryDisposition {
    match failure.class {
        WorkflowFailureClass::TransientBeforeExecution
            if !attempt.entered_running && attempts_used < max_attempts as usize =>
        {
            WorkflowRetryDisposition::Automatic
        }
        WorkflowFailureClass::TransientBeforeExecution
        | WorkflowFailureClass::AmbiguousExecution => WorkflowRetryDisposition::ExplicitOnly,
        WorkflowFailureClass::Permanent => WorkflowRetryDisposition::Never,
    }
}

pub(crate) fn normalize_failure(
    entered_running: bool,
    mut failure: WorkflowAttemptFailure,
) -> WorkflowAttemptFailure {
    if entered_running && failure.class == WorkflowFailureClass::TransientBeforeExecution {
        failure.class = WorkflowFailureClass::AmbiguousExecution;
    }
    failure
}
