//! Projected message types — pure data snapshots of agent conversation.
//!
//! These are UI-agnostic structured views of `Message`. Used by projection
//! to convert internal messages into a consumer-friendly format without
//! any real-time session state (status, progress, timing).

use serde::{Deserialize, Serialize};

/// Structured view of an agent message — pure data, no session state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectedMessage {
    pub role: String,
    pub content: String,
    pub tool_calls: Vec<ProjectedToolCall>,
    pub image_count: usize,
}

/// Snapshot of a tool call — result is final, no in-flight status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectedToolCall {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub result: Option<String>,
    pub is_error: bool,
    pub input: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
}
