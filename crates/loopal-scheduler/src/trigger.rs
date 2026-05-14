use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ScheduledTrigger {
    pub task_id: String,
    pub prompt: String,
    pub fired_at: DateTime<Utc>,
}
