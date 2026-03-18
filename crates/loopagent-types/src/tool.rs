use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

use crate::error::LoopAgentError;
use crate::permission::PermissionLevel;

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> serde_json::Value;
    fn permission(&self) -> PermissionLevel;

    async fn execute(
        &self,
        input: serde_json::Value,
        ctx: &ToolContext,
    ) -> std::result::Result<ToolResult, LoopAgentError>;
}

#[derive(Debug, Clone)]
pub struct ToolContext {
    /// Current working directory
    pub cwd: PathBuf,
    /// Session ID
    pub session_id: String,
    /// Opaque shared state passed to tools — tools downcast via `Any`.
    pub shared: Option<Arc<dyn std::any::Any + Send + Sync>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Output content
    pub content: String,
    /// Whether the tool execution resulted in an error
    pub is_error: bool,
}

impl ToolResult {
    pub fn success(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: false,
        }
    }

    pub fn error(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: true,
        }
    }
}

/// Tool definition for sending to LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}
