use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::mcp_snapshot::McpServerSnapshot;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpListToolsResponse {
    pub tools: Vec<McpToolEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolEntry {
    pub server: String,
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpCallToolRequest {
    pub server: String,
    pub tool: String,
    pub args: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpReconnectRequest {
    pub server: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpReconnectResponse {
    pub connected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpCallToolResponse {
    pub content: Vec<McpContentBlock>,
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpContentBlock {
    Text { text: String },
    Image { mime_type: String, data: String },
    Audio { mime_type: String },
    Resource { uri: String, text: Option<String> },
    ResourceLink { uri: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSnapshotResponse {
    pub servers: Vec<McpServerSnapshot>,
}
