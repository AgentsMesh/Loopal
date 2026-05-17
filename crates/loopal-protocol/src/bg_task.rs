use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BgTaskStatus {
    Running,
    Completed,
    Failed,
    Killed,
}

impl BgTaskStatus {
    pub fn is_terminal(self) -> bool {
        !matches!(self, BgTaskStatus::Running)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BgTaskSnapshot {
    pub id: String,
    pub description: String,
    pub status: BgTaskStatus,
    pub exit_code: Option<i32>,
    pub created_at_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BgTaskDetail {
    pub id: String,
    pub description: String,
    pub status: BgTaskStatus,
    pub exit_code: Option<i32>,
    pub output: String,
    pub created_at_unix_ms: u64,
}

impl BgTaskDetail {
    pub fn to_snapshot(&self) -> BgTaskSnapshot {
        BgTaskSnapshot {
            id: self.id.clone(),
            description: self.description.clone(),
            status: self.status,
            exit_code: self.exit_code,
            created_at_unix_ms: self.created_at_unix_ms,
        }
    }
}
