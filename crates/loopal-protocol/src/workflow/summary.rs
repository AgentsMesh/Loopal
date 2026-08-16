use serde::{Deserialize, Serialize};

use super::{
    WorkflowNodeId, WorkflowNodeState, WorkflowRunId, WorkflowRunSnapshot, WorkflowRunState,
};

pub const MAX_RECENT_WORKFLOW_SUMMARIES: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowStateCounts {
    pub pending: u32,
    pub ready: u32,
    pub active: u32,
    pub succeeded: u32,
    pub failed: u32,
    pub cancelled: u32,
    pub skipped: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRunSummary {
    pub id: WorkflowRunId,
    pub run_goal: String,
    pub state: WorkflowRunState,
    pub revision: u64,
    pub output_node: WorkflowNodeId,
    pub counts: WorkflowStateCounts,
    pub created_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkflowRunsSnapshot {
    pub active: Vec<WorkflowRunSummary>,
    pub recent: Vec<WorkflowRunSummary>,
}

impl WorkflowRunsSnapshot {
    pub fn is_empty(&self) -> bool {
        self.active.is_empty() && self.recent.is_empty()
    }
}

impl From<&WorkflowRunSnapshot> for WorkflowRunSummary {
    fn from(run: &WorkflowRunSnapshot) -> Self {
        let mut counts = WorkflowStateCounts {
            pending: 0,
            ready: 0,
            active: 0,
            succeeded: 0,
            failed: 0,
            cancelled: 0,
            skipped: 0,
        };
        for node in &run.nodes {
            match node.state {
                WorkflowNodeState::Pending => counts.pending += 1,
                WorkflowNodeState::Ready => counts.ready += 1,
                WorkflowNodeState::Dispatching
                | WorkflowNodeState::Running
                | WorkflowNodeState::Cancelling => counts.active += 1,
                WorkflowNodeState::Succeeded => counts.succeeded += 1,
                WorkflowNodeState::Failed => counts.failed += 1,
                WorkflowNodeState::Cancelled => counts.cancelled += 1,
                WorkflowNodeState::Skipped => counts.skipped += 1,
            }
        }
        Self {
            id: run.id.clone(),
            run_goal: run.spec.run_goal.clone(),
            state: run.state,
            revision: run.revision,
            output_node: run.spec.output_node.clone(),
            counts,
            created_at_unix_ms: run.created_at_unix_ms,
            updated_at_unix_ms: run.updated_at_unix_ms,
        }
    }
}
