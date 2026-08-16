use loopal_protocol::{
    WorkflowAttemptSnapshot, WorkflowAttemptState, WorkflowNodeSnapshot, WorkflowNodeState,
    WorkflowRunSnapshot, WorkflowRunState,
};
use serde::Deserialize;

use super::common::{StrictAddress, StrictCompletion, StrictFailure, StrictOutput, StrictSpec};

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StrictRunState {
    Planned,
    Validated,
    Running,
    Cancelling,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StrictNodeState {
    Pending,
    Ready,
    Dispatching,
    Running,
    Cancelling,
    Succeeded,
    Failed,
    Cancelled,
    Skipped,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StrictAttemptState {
    Dispatching,
    Running,
    Cancelling,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StrictSnapshot {
    pub id: String,
    pub root_agent: StrictAddress,
    pub spec: StrictSpec,
    pub state: StrictRunState,
    pub revision: u64,
    pub nodes: Vec<StrictNodeSnapshot>,
    pub attempts: Vec<StrictAttemptSnapshot>,
    pub result: Option<StrictOutput>,
    pub failure: Option<StrictFailure>,
    pub created_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StrictNodeSnapshot {
    pub id: String,
    pub dependencies: Vec<String>,
    pub state: StrictNodeState,
    pub current_attempt: Option<String>,
    pub attempt_count: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StrictAttemptSnapshot {
    pub id: String,
    pub node_id: String,
    pub capability_digest: loopal_protocol::WorkflowAttemptCapabilityDigest,
    pub dispatched_at_unix_ms: u64,
    pub state: StrictAttemptState,
    pub agent: Option<StrictAddress>,
    pub entered_running: bool,
    pub completion: Option<StrictCompletion>,
    pub failure: Option<StrictFailure>,
    pub output: Option<StrictOutput>,
}

impl From<StrictSnapshot> for WorkflowRunSnapshot {
    fn from(value: StrictSnapshot) -> Self {
        Self {
            id: value.id.into(),
            root_agent: value.root_agent.into(),
            spec: value.spec.into(),
            state: value.state.into(),
            revision: value.revision,
            nodes: value.nodes.into_iter().map(Into::into).collect(),
            attempts: value.attempts.into_iter().map(Into::into).collect(),
            result: value.result.map(Into::into),
            failure: value.failure.map(Into::into),
            created_at_unix_ms: value.created_at_unix_ms,
            updated_at_unix_ms: value.updated_at_unix_ms,
        }
    }
}

impl From<StrictNodeSnapshot> for WorkflowNodeSnapshot {
    fn from(value: StrictNodeSnapshot) -> Self {
        Self {
            id: value.id.into(),
            dependencies: value.dependencies.into_iter().map(Into::into).collect(),
            state: value.state.into(),
            current_attempt: value.current_attempt.map(Into::into),
            attempt_count: value.attempt_count,
        }
    }
}

impl From<StrictAttemptSnapshot> for WorkflowAttemptSnapshot {
    fn from(value: StrictAttemptSnapshot) -> Self {
        Self {
            id: value.id.into(),
            node_id: value.node_id.into(),
            capability_digest: value.capability_digest,
            dispatched_at_unix_ms: value.dispatched_at_unix_ms,
            state: value.state.into(),
            agent: value.agent.map(Into::into),
            entered_running: value.entered_running,
            completion: value.completion.map(Into::into),
            failure: value.failure.map(Into::into),
            output: value.output.map(Into::into),
        }
    }
}

impl From<StrictRunState> for WorkflowRunState {
    fn from(value: StrictRunState) -> Self {
        match value {
            StrictRunState::Planned => Self::Planned,
            StrictRunState::Validated => Self::Validated,
            StrictRunState::Running => Self::Running,
            StrictRunState::Cancelling => Self::Cancelling,
            StrictRunState::Succeeded => Self::Succeeded,
            StrictRunState::Failed => Self::Failed,
            StrictRunState::Cancelled => Self::Cancelled,
        }
    }
}

impl From<StrictNodeState> for WorkflowNodeState {
    fn from(value: StrictNodeState) -> Self {
        match value {
            StrictNodeState::Pending => Self::Pending,
            StrictNodeState::Ready => Self::Ready,
            StrictNodeState::Dispatching => Self::Dispatching,
            StrictNodeState::Running => Self::Running,
            StrictNodeState::Cancelling => Self::Cancelling,
            StrictNodeState::Succeeded => Self::Succeeded,
            StrictNodeState::Failed => Self::Failed,
            StrictNodeState::Cancelled => Self::Cancelled,
            StrictNodeState::Skipped => Self::Skipped,
        }
    }
}

impl From<StrictAttemptState> for WorkflowAttemptState {
    fn from(value: StrictAttemptState) -> Self {
        match value {
            StrictAttemptState::Dispatching => Self::Dispatching,
            StrictAttemptState::Running => Self::Running,
            StrictAttemptState::Cancelling => Self::Cancelling,
            StrictAttemptState::Succeeded => Self::Succeeded,
            StrictAttemptState::Failed => Self::Failed,
            StrictAttemptState::Cancelled => Self::Cancelled,
        }
    }
}
