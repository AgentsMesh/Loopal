mod convert;
mod wire;

use serde::Deserialize;

use super::snapshot::StrictSnapshot;
use wire::*;

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum StrictWaitStatus {
    Changed,
    Terminal,
    TimedOut,
    NotFound,
}

pub(crate) enum RequestShapeError {
    RunIdMismatch(String),
    Invalid(String),
}

pub(crate) fn validate_shapes(
    expected_run: &str,
    expected_request: &str,
    operation: &str,
    payload: &serde_json::Value,
    response: &serde_json::Value,
) -> Result<(), RequestShapeError> {
    match operation {
        "start" => {
            let request: StrictStartRequest = decode(payload)?;
            let response: StrictStartResponse = decode(response)?;
            same_request(expected_request, &request.request_id)?;
            consume(request.spec);
            validate_summary(expected_run, response.summary)
        }
        "get" => {
            let request: StrictGetRequest = decode(payload)?;
            let response: StrictGetResponse = decode(response)?;
            same_request(expected_request, &request.request_id)?;
            same_run(expected_run, &request.run_id)?;
            optional_run(expected_run, response.run.as_ref())
        }
        "wait" => {
            let request: StrictWaitRequest = decode(payload)?;
            let response: StrictWaitResponse = decode(response)?;
            same_request(expected_request, &request.request_id)?;
            same_run(expected_run, &request.run_id)?;
            consume((request.after_revision, request.timeout_ms, response.status));
            optional_run(expected_run, response.run.as_ref())
        }
        "cancel" => {
            let request: StrictCancelRequest = decode(payload)?;
            let response: StrictCancelResponse = decode(response)?;
            same_request(expected_request, &request.request_id)?;
            same_run(expected_run, &request.run_id)?;
            consume((request.reason, response.already_terminal));
            validate_summary(expected_run, response.summary)
        }
        _ => Err(RequestShapeError::Invalid(format!(
            "unsupported workflow operation {operation}"
        ))),
    }
}

fn decode<T: for<'de> Deserialize<'de>>(value: &serde_json::Value) -> Result<T, RequestShapeError> {
    serde_json::from_value(value.clone())
        .map_err(|error| RequestShapeError::Invalid(error.to_string()))
}

fn same_request(expected: &str, actual: &str) -> Result<(), RequestShapeError> {
    (expected == actual).then_some(()).ok_or_else(|| {
        RequestShapeError::Invalid(format!(
            "request id mismatch: expected {expected}, found {actual}"
        ))
    })
}

fn same_run(expected: &str, actual: &str) -> Result<(), RequestShapeError> {
    (expected == actual)
        .then_some(())
        .ok_or_else(|| RequestShapeError::RunIdMismatch(actual.to_string()))
}

fn optional_run(expected: &str, run: Option<&StrictSnapshot>) -> Result<(), RequestShapeError> {
    match run {
        Some(run) => same_run(expected, &run.id),
        None => Ok(()),
    }
}

fn validate_summary(expected: &str, summary: StrictSummary) -> Result<(), RequestShapeError> {
    same_run(expected, &summary.id)?;
    consume((
        summary.run_goal,
        summary.state,
        summary.revision,
        summary.output_node,
        summary.created_at_unix_ms,
        summary.updated_at_unix_ms,
    ));
    let counts = summary.counts;
    consume((
        counts.pending,
        counts.ready,
        counts.active,
        counts.succeeded,
        counts.failed,
        counts.cancelled,
        counts.skipped,
    ));
    Ok(())
}

fn consume<T>(_value: T) {}
