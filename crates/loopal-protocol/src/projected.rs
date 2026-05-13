use loopal_tool_invocation::ToolResultMetadata;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectedMessage {
    pub role: String,
    pub content: String,
    pub tool_calls: Vec<ProjectedToolCall>,
    pub image_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectedToolCall {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub result: Option<String>,
    pub is_error: bool,
    pub input: Option<serde_json::Value>,
    pub metadata: Option<ToolResultMetadata>,
}
