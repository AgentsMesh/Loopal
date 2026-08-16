use serde::{Deserialize, Serialize};

use crate::{AgentCompletion, QualifiedAddress, WorkflowAttemptCapabilityDigest};

use super::{
    WorkflowAttemptFailure, WorkflowAttemptId, WorkflowNodeId, WorkflowOutput, WorkflowRunId,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowEvent {
    pub run_id: WorkflowRunId,
    pub revision: u64,
    pub occurred_at_unix_ms: u64,
    pub payload: WorkflowEventPayload,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkflowEventPayload {
    SpecValidated,
    RunStarted,
    RunDeadlineExceeded {
        failure: WorkflowAttemptFailure,
    },
    DispatchIntended {
        node_id: WorkflowNodeId,
        attempt_id: WorkflowAttemptId,
        capability_digest: WorkflowAttemptCapabilityDigest,
    },
    AttemptBound {
        node_id: WorkflowNodeId,
        attempt_id: WorkflowAttemptId,
        agent: QualifiedAddress,
    },
    AttemptRunning {
        node_id: WorkflowNodeId,
        attempt_id: WorkflowAttemptId,
    },
    AttemptStopRequested {
        node_id: WorkflowNodeId,
        attempt_id: WorkflowAttemptId,
        reason: String,
    },
    AttemptSucceeded {
        node_id: WorkflowNodeId,
        attempt_id: WorkflowAttemptId,
        completion: AgentCompletion,
        output: Option<WorkflowOutput>,
    },
    AttemptFailed {
        node_id: WorkflowNodeId,
        attempt_id: WorkflowAttemptId,
        completion: AgentCompletion,
        failure: WorkflowAttemptFailure,
    },
    CancelRequested {
        reason: Option<String>,
    },
    AttemptCancelled {
        node_id: WorkflowNodeId,
        attempt_id: WorkflowAttemptId,
        reason: String,
    },
}
