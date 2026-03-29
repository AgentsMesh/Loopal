use chrono::{DateTime, Utc};
use serde::Serialize;

/// Payload emitted when a scheduled task fires.
#[derive(Debug, Clone, Serialize)]
pub struct ScheduledTrigger {
    /// ID of the task that fired.
    pub task_id: String,
    /// Prompt to inject into the agent loop.
    pub prompt: String,
    /// Timestamp when the trigger actually fired.
    pub fired_at: DateTime<Utc>,
}
