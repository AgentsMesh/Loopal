use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use loopal_error::LoopalError;

use crate::permission::PermissionLevel;
use crate::tool_context::ToolContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolDispatch {
    Pipeline,
    RunnerDirect,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> serde_json::Value;
    fn permission(&self) -> PermissionLevel;

    fn dispatch(&self) -> ToolDispatch {
        ToolDispatch::Pipeline
    }

    fn precheck(&self, _input: &serde_json::Value) -> Option<String> {
        None
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        ctx: &ToolContext,
    ) -> std::result::Result<ToolResult, LoopalError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: String,
    pub is_error: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl ToolResult {
    pub fn success(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: false,
            metadata: None,
        }
    }

    pub fn error(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: true,
            metadata: None,
        }
    }

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}
