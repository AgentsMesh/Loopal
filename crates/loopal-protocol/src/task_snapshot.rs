//! Task snapshot types for TUI / IPC observation.
//!
//! Defined in protocol so presentation layers can display task progress
//! without depending on the agent-level TaskStore.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskSnapshotStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSnapshot {
    pub id: String,
    pub subject: String,
    pub active_form: Option<String>,
    pub status: TaskSnapshotStatus,
    pub blocked_by: Vec<String>,
}
