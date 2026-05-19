use serde::{Deserialize, Serialize};

use crate::address::QualifiedAddress;

/// Aggregated per-turn metrics. Extracted from `AgentEventPayload::TurnCompleted`
/// so the enum body stays readable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnSummary {
    pub turn_id: u32,
    pub duration_ms: u64,
    pub llm_calls: u32,
    pub tool_calls_requested: u32,
    pub tool_calls_approved: u32,
    pub tool_calls_denied: u32,
    pub tool_errors: u32,
    pub auto_continuations: u32,
    pub warnings_injected: u32,
    pub tokens_in: u32,
    pub tokens_out: u32,
    pub modified_files: Vec<String>,
}

/// Phases of a `CompactProgress` notification. Listed in firing order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactPhase {
    Microcompact,
    Summarize,
    Rehydrate,
    Done,
}

/// Outcome metrics for `AgentEventPayload::Compacted`. `strategy` is one of
/// `"auto"` (threshold-triggered) / `"manual"` (user `/compact`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionSummary {
    pub kept: usize,
    pub removed: usize,
    pub tokens_before: u32,
    pub tokens_after: u32,
    pub strategy: String,
    /// Storage UUID of the persisted summary message (anchor for
    /// `Marker::CompactBoundary` on resume).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary_msg_id: Option<String>,
    /// How many files the post-compact rehydrate stage re-read.
    #[serde(default)]
    pub files_rehydrated: usize,
}

/// Metadata attached to `AgentEventPayload::SubAgentSpawned`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentSpawn {
    pub name: String,
    pub agent_id: String,
    /// Parent address — qualified when spawned cross-hub.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<QualifiedAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}
