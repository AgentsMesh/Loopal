use loopal_protocol::{
    WorkflowEvent, WorkflowEventPayload, WorkflowReduceOutcome, WorkflowRunSnapshot,
    reduce_workflow_event,
};

use super::WorkflowCoordinatorError;

pub(super) fn apply_payload(
    run: &WorkflowRunSnapshot,
    payload: WorkflowEventPayload,
    occurred_at_unix_ms: u64,
) -> Result<(WorkflowEvent, WorkflowRunSnapshot), WorkflowCoordinatorError> {
    let event = WorkflowEvent {
        run_id: run.id.clone(),
        revision: run.revision.saturating_add(1),
        occurred_at_unix_ms,
        payload,
    };
    let next = apply_event(run, &event)?;
    Ok((event, next))
}

pub(super) fn apply_event(
    run: &WorkflowRunSnapshot,
    event: &WorkflowEvent,
) -> Result<WorkflowRunSnapshot, WorkflowCoordinatorError> {
    match reduce_workflow_event(run, event, &loopal_workflow_schema::WorkflowSchemaValidator)? {
        WorkflowReduceOutcome::Applied(next) => Ok(*next),
        WorkflowReduceOutcome::IgnoredStale { .. } => {
            Err(WorkflowCoordinatorError::UnexpectedStaleEvent)
        }
    }
}
