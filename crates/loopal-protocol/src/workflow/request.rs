use serde::{Deserialize, Serialize};

use super::WorkflowRequestId;

pub const MAX_WORKFLOW_REQUEST_RECORDS: usize = 64;
pub const MAX_WORKFLOW_REQUEST_OPERATION_BYTES: usize = 64;
pub const MAX_WORKFLOW_REQUEST_PAYLOAD_BYTES: usize = 1_048_576;
pub const MAX_WORKFLOW_REQUEST_RESPONSE_BYTES: usize = 65_536;
pub const MAX_WORKFLOW_REQUEST_LEDGER_BYTES: usize = 8 * 1_024 * 1_024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRequestRecord {
    pub request_id: WorkflowRequestId,
    pub operation: String,
    pub payload: serde_json::Value,
    pub response: serde_json::Value,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRequestLedger {
    records: Vec<WorkflowRequestRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkflowRequestDecision<'a> {
    New,
    Replay(&'a serde_json::Value),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkflowRequestError {
    PayloadMismatch { request_id: WorkflowRequestId },
    InvalidRequestId,
    InvalidOperation,
    PayloadTooLarge,
    ResponseTooLarge,
    LedgerFull,
}

impl WorkflowRequestLedger {
    pub fn decide<'a>(
        &'a self,
        request_id: &WorkflowRequestId,
        operation: &str,
        payload: &serde_json::Value,
    ) -> Result<WorkflowRequestDecision<'a>, WorkflowRequestError> {
        self.decide_with_response_size(
            request_id,
            operation,
            payload,
            MAX_WORKFLOW_REQUEST_RESPONSE_BYTES,
        )
    }

    pub fn decide_with_response_size<'a>(
        &'a self,
        request_id: &WorkflowRequestId,
        operation: &str,
        payload: &serde_json::Value,
        reserved_response_bytes: usize,
    ) -> Result<WorkflowRequestDecision<'a>, WorkflowRequestError> {
        if !request_id.is_valid() {
            return Err(WorkflowRequestError::InvalidRequestId);
        }
        if reserved_response_bytes > MAX_WORKFLOW_REQUEST_RESPONSE_BYTES {
            return Err(WorkflowRequestError::ResponseTooLarge);
        }
        let Some(record) = self
            .records
            .iter()
            .find(|record| &record.request_id == request_id)
        else {
            validate_operation(operation)?;
            if encoded_len(payload) > MAX_WORKFLOW_REQUEST_PAYLOAD_BYTES {
                return Err(WorkflowRequestError::PayloadTooLarge);
            }
            let reserved_bytes = encoded_len(payload)
                .saturating_add(reserved_response_bytes)
                .saturating_add(operation.len())
                .saturating_add(request_id.as_str().len());
            if self.records.len() >= MAX_WORKFLOW_REQUEST_RECORDS
                || self.encoded_bytes().saturating_add(reserved_bytes)
                    > MAX_WORKFLOW_REQUEST_LEDGER_BYTES
            {
                return Err(WorkflowRequestError::LedgerFull);
            }
            return Ok(WorkflowRequestDecision::New);
        };
        if record.operation == operation && record.payload == *payload {
            Ok(WorkflowRequestDecision::Replay(&record.response))
        } else {
            Err(WorkflowRequestError::PayloadMismatch {
                request_id: request_id.clone(),
            })
        }
    }

    pub fn record(&mut self, record: WorkflowRequestRecord) -> Result<(), WorkflowRequestError> {
        let response_bytes = encoded_len(&record.response);
        match self.decide_with_response_size(
            &record.request_id,
            &record.operation,
            &record.payload,
            response_bytes,
        )? {
            WorkflowRequestDecision::Replay(_) => return Ok(()),
            WorkflowRequestDecision::New => {}
        }
        self.records.push(record);
        Ok(())
    }

    pub fn records(&self) -> &[WorkflowRequestRecord] {
        &self.records
    }

    fn encoded_bytes(&self) -> usize {
        self.records.iter().fold(0usize, |total, record| {
            total
                .saturating_add(record.request_id.as_str().len())
                .saturating_add(record.operation.len())
                .saturating_add(encoded_len(&record.payload))
                .saturating_add(encoded_len(&record.response))
        })
    }
}

fn validate_operation(operation: &str) -> Result<(), WorkflowRequestError> {
    if operation.is_empty() || operation.len() > MAX_WORKFLOW_REQUEST_OPERATION_BYTES {
        Err(WorkflowRequestError::InvalidOperation)
    } else {
        Ok(())
    }
}

fn encoded_len(value: &serde_json::Value) -> usize {
    serde_json::to_vec(value)
        .expect("serde_json::Value always serializes")
        .len()
}
