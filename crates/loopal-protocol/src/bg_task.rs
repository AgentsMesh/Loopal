//! Background task snapshot types for TUI / IPC observation.
//!
//! Defined in protocol so that presentation layers (TUI, ACP) can display
//! background task status without depending on the tool-level store.

/// Observable status of a background task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BgTaskStatus {
    Running,
    Completed,
    Failed,
}

/// Minimal, read-only snapshot of a background task for display.
#[derive(Debug, Clone)]
pub struct BgTaskSnapshot {
    pub id: String,
    pub description: String,
    pub status: BgTaskStatus,
}
