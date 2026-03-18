use serde::{Deserialize, Serialize};

/// Unique identifier for an agent instance.
pub type AgentId = String;

/// Unique identifier for a task.
pub type TaskId = String;

/// Task status lifecycle: Pending → InProgress → Completed | Deleted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Deleted,
}

/// A task in the shared task list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub subject: String,
    pub description: String,
    pub status: TaskStatus,
    pub owner: Option<String>,
    pub blocked_by: Vec<TaskId>,
    pub blocks: Vec<TaskId>,
    #[serde(default)]
    pub metadata: serde_json::Value,
    pub created_at: String,
}

/// Team configuration for coordinating multiple agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub name: String,
    pub description: String,
    pub members: Vec<TeamMember>,
}

/// A member entry in a team configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMember {
    pub name: String,
    pub agent_id: AgentId,
    pub agent_type: String,
}
