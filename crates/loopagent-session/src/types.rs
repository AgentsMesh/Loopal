//! Display types shared between session controller and UI consumers.
//!
//! These types represent the presentation-layer view of agent messages,
//! tool calls, and pending permission requests.

/// A message to display in the chat view.
#[derive(Debug, Clone)]
pub struct DisplayMessage {
    pub role: String,
    pub content: String,
    pub tool_calls: Vec<DisplayToolCall>,
}

/// A tool call to display in the chat view.
#[derive(Debug, Clone)]
pub struct DisplayToolCall {
    pub name: String,
    /// "pending", "success", "error"
    pub status: String,
    /// Call description, e.g. "Read(/tmp/foo.rs)". Not overwritten by ToolResult.
    pub summary: String,
    /// Full tool output (None while pending).
    /// Session layer applies loose storage-protection truncation (200 lines / 10 KB).
    pub result: Option<String>,
}

/// A pending tool permission request awaiting user approval.
#[derive(Debug, Clone)]
pub struct PendingPermission {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}
