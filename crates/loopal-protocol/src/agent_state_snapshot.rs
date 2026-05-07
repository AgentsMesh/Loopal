//! Agent → Hub full-state dump for ViewState cold-start rebuild.
//!
//! Hub keeps `ViewState` purely in memory; when a hub restarts or an
//! agent reconnects, it requests this snapshot via `agent/state_snapshot`
//! IPC and seeds the per-agent `ViewStateReducer` with it. Subsequent
//! mutations come through the normal `AgentEvent` stream.
//!
//! `observable` is intentionally absent: token counts, mode, status, and
//! tool counts are accumulated by the Hub-side reducer from the agent's
//! event stream, not duplicated on the agent side.

use serde::{Deserialize, Serialize};

use crate::bg_task::BgTaskSnapshot;
use crate::cron_snapshot::CronJobSnapshot;
use crate::task_snapshot::TaskSnapshot;
use crate::thread_goal::ThreadGoal;

/// Complete observable state of a single agent process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStateSnapshot {
    pub tasks: Vec<TaskSnapshot>,
    pub crons: Vec<CronJobSnapshot>,
    pub bg_tasks: Vec<BgTaskSnapshot>,
    /// Persistent thread goal (if any). Each newly attached client receives
    /// it through the same `agent/state_snapshot` IPC path so the UI can
    /// render the goal indicator without waiting for the next mutation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_goal: Option<ThreadGoal>,
}

impl AgentStateSnapshot {
    pub fn empty() -> Self {
        Self {
            tasks: Vec::new(),
            crons: Vec::new(),
            bg_tasks: Vec::new(),
            thread_goal: None,
        }
    }
}
