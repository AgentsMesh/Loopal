mod delivery;

use loopal_protocol::{
    MAX_WORKFLOW_REQUEST_OPERATION_BYTES, MAX_WORKFLOW_REQUEST_PAYLOAD_BYTES,
    MAX_WORKFLOW_REQUEST_RESPONSE_BYTES, WorkflowRequestRecord, WorkflowRunId,
    validate_workflow_spec,
};

use super::error::{WorkflowJournalError, WorkflowJournalLimit};
use super::record::{WORKFLOW_JOURNAL_VERSION, WorkflowJournalEntry};
use super::{MAX_WORKFLOW_EVENTS_PER_COMMIT, MAX_WORKFLOW_REQUEST_RECORD_BYTES};

pub(crate) fn entry(
    expected_session: &str,
    expected_run: &WorkflowRunId,
    value: &WorkflowJournalEntry,
) -> Result<(), WorkflowJournalError> {
    if value.version() != WORKFLOW_JOURNAL_VERSION {
        return Err(invalid(format!("unsupported version {}", value.version())));
    }
    match value {
        WorkflowJournalEntry::Init {
            snapshot,
            events,
            request,
            ..
        } => {
            run_id(expected_run, &snapshot.id)?;
            if snapshot.revision != 0
                || snapshot.state != loopal_protocol::WorkflowRunState::Planned
            {
                return Err(invalid("init snapshot must be planned at revision zero"));
            }
            validate_workflow_spec(&snapshot.spec)
                .map_err(|error| invalid(format!("invalid workflow spec: {error:?}")))?;
            validate_initial_snapshot(snapshot)?;
            validate_events(expected_run, events)?;
            if events.first().is_some_and(|event| event.revision != 1) {
                return Err(invalid("init events must begin at revision one"));
            }
            if let Some(request) = request {
                request_record(expected_run, request)?;
            }
        }
        WorkflowJournalEntry::Commit {
            run_id: commit_run,
            events,
            request,
            ..
        } => {
            run_id(expected_run, commit_run)?;
            validate_events(expected_run, events)?;
            if events.is_empty() && request.is_none() {
                return Err(invalid("commit must contain events or a request"));
            }
            if let Some(request) = request {
                request_record(expected_run, request)?;
            }
        }
        WorkflowJournalEntry::DeliveryIntent { notification, .. } => {
            notification
                .validate()
                .map_err(|error| invalid(format!("invalid delivery intent: {error:?}")))?;
            delivery::id(expected_session, expected_run, &notification.delivery_id)?;
        }
        WorkflowJournalEntry::DeliveryAck { delivery_id, .. } => {
            delivery::id(expected_session, expected_run, delivery_id)?;
        }
    }
    Ok(())
}

pub(crate) fn request_record(
    expected: &WorkflowRunId,
    record: &WorkflowRequestRecord,
) -> Result<(), WorkflowJournalError> {
    if !record.request_id.is_valid()
        || record.operation.is_empty()
        || record.operation.len() > MAX_WORKFLOW_REQUEST_OPERATION_BYTES
    {
        return Err(invalid("invalid workflow request identity"));
    }
    let payload = serde_json::to_vec(&record.payload).expect("JSON value serializes");
    let response = serde_json::to_vec(&record.response).expect("JSON value serializes");
    if payload.len() > MAX_WORKFLOW_REQUEST_PAYLOAD_BYTES {
        return Err(invalid("workflow request payload exceeds protocol limit"));
    }
    if response.len() > MAX_WORKFLOW_REQUEST_RESPONSE_BYTES {
        return Err(invalid("workflow request response exceeds protocol limit"));
    }
    super::strict::request::validate_shapes(
        expected.as_str(),
        record.request_id.as_str(),
        &record.operation,
        &record.payload,
        &record.response,
    )
    .map_err(|error| match error {
        super::strict::request::RequestShapeError::RunIdMismatch(actual) => {
            WorkflowJournalError::RunIdMismatch {
                expected: expected.to_string(),
                actual,
            }
        }
        super::strict::request::RequestShapeError::Invalid(detail) => {
            invalid(format!("invalid workflow request shape: {detail}"))
        }
    })?;
    let encoded = serde_json::to_vec(record)
        .map_err(|error| WorkflowJournalError::Serialization(error.to_string()))?;
    if encoded.len() > MAX_WORKFLOW_REQUEST_RECORD_BYTES {
        return Err(WorkflowJournalError::limit(
            WorkflowJournalLimit::RequestBytes,
            encoded.len(),
            MAX_WORKFLOW_REQUEST_RECORD_BYTES,
        ));
    }
    Ok(())
}

fn validate_events(
    expected_run: &WorkflowRunId,
    events: &[loopal_protocol::WorkflowEvent],
) -> Result<(), WorkflowJournalError> {
    if events.len() > MAX_WORKFLOW_EVENTS_PER_COMMIT {
        return Err(WorkflowJournalError::limit(
            WorkflowJournalLimit::EventsPerCommit,
            events.len(),
            MAX_WORKFLOW_EVENTS_PER_COMMIT,
        ));
    }
    for event in events {
        run_id(expected_run, &event.run_id)?;
    }
    for pair in events.windows(2) {
        if pair[1].revision != pair[0].revision.saturating_add(1) {
            return Err(invalid("journal event revisions must be contiguous"));
        }
    }
    Ok(())
}

fn validate_initial_snapshot(
    snapshot: &loopal_protocol::WorkflowRunSnapshot,
) -> Result<(), WorkflowJournalError> {
    if !snapshot.attempts.is_empty()
        || snapshot.result.is_some()
        || snapshot.failure.is_some()
        || snapshot.updated_at_unix_ms != snapshot.created_at_unix_ms
    {
        return Err(invalid("init snapshot contains non-planned state"));
    }
    if snapshot.nodes.len() != snapshot.spec.nodes.len() {
        return Err(invalid("init snapshot node set does not match spec"));
    }
    for (node, planned) in snapshot.nodes.iter().zip(&snapshot.spec.nodes) {
        if node.id != planned.id
            || node.dependencies != planned.dependencies
            || node.state != loopal_protocol::WorkflowNodeState::Pending
            || node.current_attempt.is_some()
            || node.attempt_count != 0
        {
            return Err(invalid("init snapshot node state is not planned"));
        }
    }
    Ok(())
}

pub(super) fn run_id(
    expected: &WorkflowRunId,
    actual: &WorkflowRunId,
) -> Result<(), WorkflowJournalError> {
    if !actual.is_valid() {
        return Err(WorkflowJournalError::InvalidRunId(actual.to_string()));
    }
    if actual != expected {
        return Err(WorkflowJournalError::RunIdMismatch {
            expected: expected.to_string(),
            actual: actual.to_string(),
        });
    }
    Ok(())
}

pub(super) fn invalid(detail: impl Into<String>) -> WorkflowJournalError {
    WorkflowJournalError::Corruption {
        path: std::path::PathBuf::new(),
        offset: 0,
        detail: detail.into(),
    }
}
