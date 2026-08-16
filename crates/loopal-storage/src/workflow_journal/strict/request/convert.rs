use loopal_protocol::WorkflowRequestRecord;

use super::super::StrictRequestRecord;

impl From<StrictRequestRecord> for WorkflowRequestRecord {
    fn from(value: StrictRequestRecord) -> Self {
        Self {
            request_id: value.request_id.into(),
            operation: value.operation,
            payload: value.payload,
            response: value.response,
        }
    }
}
