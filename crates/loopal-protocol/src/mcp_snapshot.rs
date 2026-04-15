use serde::{Deserialize, Serialize};

/// Point-in-time snapshot of a single MCP server's runtime state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerSnapshot {
    pub name: String,
    /// "stdio" or "streamable-http"
    pub transport: String,
    /// Config layer source, e.g. "global", "project", "plugin:xxx"
    pub source: String,
    /// Display string: "connected", "disconnected", "connecting", "failed: ..."
    pub status: String,
    pub tool_count: usize,
    pub resource_count: usize,
    pub prompt_count: usize,
    pub errors: Vec<String>,
}
