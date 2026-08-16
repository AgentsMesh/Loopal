use serde::{Deserialize, Serialize};

use crate::{AgentCompletion, QualifiedAddress, WorkflowAttemptCapabilityDigest};

use super::{WorkflowAttemptId, WorkflowNodeId, WorkflowOutput, WorkflowRunId, WorkflowSpec};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunState {
    Planned,
    Validated,
    Running,
    Cancelling,
    Succeeded,
    Failed,
    Cancelled,
}

impl WorkflowRunState {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowNodeState {
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

impl WorkflowNodeState {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::Skipped
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowAttemptState {
    Dispatching,
    Running,
    Cancelling,
    Succeeded,
    Failed,
    Cancelled,
}

impl WorkflowAttemptState {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowFailureClass {
    TransientBeforeExecution,
    AmbiguousExecution,
    Permanent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowAttemptFailure {
    pub class: WorkflowFailureClass,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowNodeSnapshot {
    pub id: WorkflowNodeId,
    pub dependencies: Vec<WorkflowNodeId>,
    pub state: WorkflowNodeState,
    pub current_attempt: Option<WorkflowAttemptId>,
    pub attempt_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowAttemptSnapshot {
    pub id: WorkflowAttemptId,
    pub node_id: WorkflowNodeId,
    pub capability_digest: WorkflowAttemptCapabilityDigest,
    pub dispatched_at_unix_ms: u64,
    pub state: WorkflowAttemptState,
    pub agent: Option<QualifiedAddress>,
    pub entered_running: bool,
    pub completion: Option<AgentCompletion>,
    pub failure: Option<WorkflowAttemptFailure>,
    pub output: Option<WorkflowOutput>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRunSnapshot {
    pub id: WorkflowRunId,
    pub root_agent: QualifiedAddress,
    pub spec: WorkflowSpec,
    pub state: WorkflowRunState,
    pub revision: u64,
    pub nodes: Vec<WorkflowNodeSnapshot>,
    pub attempts: Vec<WorkflowAttemptSnapshot>,
    pub result: Option<WorkflowOutput>,
    pub failure: Option<WorkflowAttemptFailure>,
    pub created_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
}

impl WorkflowRunSnapshot {
    pub fn planned(
        id: WorkflowRunId,
        root_agent: QualifiedAddress,
        spec: WorkflowSpec,
        created_at_unix_ms: u64,
    ) -> Self {
        let nodes = spec
            .nodes
            .iter()
            .map(|node| WorkflowNodeSnapshot {
                id: node.id.clone(),
                dependencies: node.dependencies.clone(),
                state: WorkflowNodeState::Pending,
                current_attempt: None,
                attempt_count: 0,
            })
            .collect();
        Self {
            id,
            root_agent,
            spec,
            state: WorkflowRunState::Planned,
            revision: 0,
            nodes,
            attempts: Vec::new(),
            result: None,
            failure: None,
            created_at_unix_ms,
            updated_at_unix_ms: created_at_unix_ms,
        }
    }
}
