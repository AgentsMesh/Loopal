use loopal_protocol::{
    WorkflowCancelRequest, WorkflowCancelResponse, WorkflowGetRequest, WorkflowGetResponse,
    WorkflowRequestRecord, WorkflowRunSnapshot, WorkflowRunSummary, WorkflowStartRequest,
    WorkflowStartResponse,
};

use crate::workflow::WorkflowCoordinatorError;

pub(super) fn validate_start(
    record: &WorkflowRequestRecord,
    planned: &WorkflowRunSnapshot,
    validated: &WorkflowRunSnapshot,
) -> Result<(), WorkflowCoordinatorError> {
    if record.operation != "start" {
        return Err(WorkflowCoordinatorError::RecoveryInvalid);
    }
    let request: WorkflowStartRequest = decode(&record.payload)?;
    let response: WorkflowStartResponse = decode(&record.response)?;
    if request.request_id != record.request_id
        || request.spec != planned.spec
        || response.summary != WorkflowRunSummary::from(validated)
    {
        return Err(WorkflowCoordinatorError::RecoveryInvalid);
    }
    Ok(())
}

pub(super) fn validate_get(
    record: &WorkflowRequestRecord,
    snapshot: &WorkflowRunSnapshot,
) -> Result<(), WorkflowCoordinatorError> {
    let request: WorkflowGetRequest = decode(&record.payload)?;
    let response: WorkflowGetResponse = decode(&record.response)?;
    if request.request_id != record.request_id
        || request.run_id != snapshot.id
        || response.run.as_ref() != Some(snapshot)
    {
        return Err(WorkflowCoordinatorError::RecoveryInvalid);
    }
    Ok(())
}

pub(super) fn validate_cancel(
    record: &WorkflowRequestRecord,
    snapshot: &WorkflowRunSnapshot,
    had_event: bool,
) -> Result<(), WorkflowCoordinatorError> {
    let request: WorkflowCancelRequest = decode(&record.payload)?;
    let response: WorkflowCancelResponse = decode(&record.response)?;
    if request.request_id != record.request_id
        || request.run_id != snapshot.id
        || response.summary != WorkflowRunSummary::from(snapshot)
        || response.already_terminal != (!had_event && snapshot.state.is_terminal())
    {
        return Err(WorkflowCoordinatorError::RecoveryInvalid);
    }
    Ok(())
}

fn decode<T: serde::de::DeserializeOwned>(
    value: &serde_json::Value,
) -> Result<T, WorkflowCoordinatorError> {
    serde_json::from_value(value.clone()).map_err(|_| WorkflowCoordinatorError::RecoveryInvalid)
}
