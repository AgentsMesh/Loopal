//! Background task snapshot types for TUI / IPC observation.
//!
//! Defined in protocol so that presentation layers (TUI, ACP) can display
//! background task status without depending on the tool-level store.

use serde::{Deserialize, Serialize};

/// Observable status of a background task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BgTaskStatus {
    Running,
    Completed,
    Failed,
}

/// Minimal, read-only snapshot of a background task for panel display.
#[derive(Debug, Clone)]
pub struct BgTaskSnapshot {
    pub id: String,
    pub description: String,
    pub status: BgTaskStatus,
    pub exit_code: Option<i32>,
}

/// Full detail of a background task, including captured output.
///
/// Transmitted via `BgTasksUpdate` events from agent process to TUI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BgTaskDetail {
    pub id: String,
    pub description: String,
    pub status: BgTaskStatus,
    pub exit_code: Option<i32>,
    pub output: String,
}

impl BgTaskDetail {
    pub fn to_snapshot(&self) -> BgTaskSnapshot {
        BgTaskSnapshot {
            id: self.id.clone(),
            description: self.description.clone(),
            status: self.status,
            exit_code: self.exit_code,
        }
    }
}
