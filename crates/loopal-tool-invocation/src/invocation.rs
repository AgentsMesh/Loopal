use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::id::InvocationId;
use crate::outcome::ToolResultMetadata;
use crate::state::InvocationState;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolInvocation {
    pub id: InvocationId,
    pub name: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<Value>,
    /// Process-local start time. `#[serde(skip)]` — re-anchors to `now` on
    /// deserialize. Terminal-state duration is carried inside `state` so
    /// cross-process restores preserve it.
    #[serde(skip, default = "Instant::now")]
    pub started_at: Instant,
    pub state: InvocationState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ToolResultMetadata>,
}

impl ToolInvocation {
    pub fn start(
        id: InvocationId,
        name: impl Into<String>,
        summary: impl Into<String>,
        input: Option<Value>,
        now: Instant,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            summary: summary.into(),
            input,
            started_at: now,
            state: InvocationState::Pending,
            batch_id: None,
            metadata: None,
        }
    }

    /// Live elapsed time: for active states, `now - started_at`. For
    /// terminal states, the stored `duration` from the transition moment.
    pub fn elapsed(&self, now: Instant) -> std::time::Duration {
        match self.state.duration() {
            Some(d) => d,
            None => now.saturating_duration_since(self.started_at),
        }
    }
}
