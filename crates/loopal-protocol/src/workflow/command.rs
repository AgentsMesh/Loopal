use serde::{Deserialize, Serialize};

use super::{
    WorkflowAttemptCapability, WorkflowAttemptState, WorkflowRequestId, WorkflowRunId,
    WorkflowRunSnapshot, WorkflowRunSummary, WorkflowSpec,
};
use crate::WorkflowPermissionCausation;

pub const MAX_WORKFLOW_WAIT_MS: u64 = 300_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowStartRequest {
    pub request_id: WorkflowRequestId,
    pub spec: WorkflowSpec,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowStartResponse {
    pub summary: WorkflowRunSummary,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowGetRequest {
    pub request_id: WorkflowRequestId,
    pub run_id: WorkflowRunId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowGetResponse {
    pub run: Option<WorkflowRunSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowWaitRequest {
    pub request_id: WorkflowRequestId,
    pub run_id: WorkflowRunId,
    pub after_revision: u64,
    pub timeout_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowWaitStatus {
    Changed,
    Terminal,
    TimedOut,
    NotFound,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowWaitResponse {
    pub status: WorkflowWaitStatus,
    pub run: Option<WorkflowRunSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowCancelRequest {
    pub request_id: WorkflowRequestId,
    pub run_id: WorkflowRunId,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowCancelResponse {
    pub summary: WorkflowRunSummary,
    pub already_terminal: bool,
}

/// Startup proof sent over the worker's already-authenticated Hub connection.
/// Hub derives the worker address and connection generation from that transport.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowWorkerHandshakeRequest {
    pub causation: WorkflowPermissionCausation,
    pub capability: WorkflowAttemptCapability,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowWorkerHandshakeDisposition {
    Fresh,
    Recovered,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowWorkerHandshakeResponse {
    pub disposition: WorkflowWorkerHandshakeDisposition,
    pub attempt_state: WorkflowAttemptState,
}
