use std::sync::Arc;

use async_trait::async_trait;
use loopagent_types::error::LoopAgentError;
use loopagent_types::permission::PermissionLevel;
use loopagent_types::tool::{Tool, ToolContext, ToolDefinition, ToolResult};
use serde_json::Value;
use tokio::sync::RwLock;

use crate::manager::McpManager;

/// Wraps an MCP tool as a local Tool trait implementation.
pub struct McpToolAdapter {
    definition: ToolDefinition,
    server_name: String,
    manager: Arc<RwLock<McpManager>>,
}

impl McpToolAdapter {
    pub fn new(
        definition: ToolDefinition,
        server_name: String,
        manager: Arc<RwLock<McpManager>>,
    ) -> Self {
        Self {
            definition,
            server_name,
            manager,
        }
    }
}

#[async_trait]
impl Tool for McpToolAdapter {
    fn name(&self) -> &str {
        &self.definition.name
    }

    fn description(&self) -> &str {
        &self.definition.description
    }

    fn parameters_schema(&self) -> Value {
        self.definition.input_schema.clone()
    }

    fn permission(&self) -> PermissionLevel {
        // MCP tools are external; treat as supervised by default
        PermissionLevel::Supervised
    }

    async fn execute(
        &self,
        input: Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult, LoopAgentError> {
        let mgr = self.manager.read().await;
        let result = mgr
            .call_tool(&self.server_name, &self.definition.name, input)
            .await
            .map_err(LoopAgentError::Mcp)?;

        // MCP tool results have "content" array; extract text
        let content = if let Some(content_arr) = result.get("content").and_then(|c| c.as_array()) {
            content_arr
                .iter()
                .filter_map(|item| item.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            result.to_string()
        };

        let is_error = result
            .get("isError")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        Ok(ToolResult { content, is_error })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_adapter() -> McpToolAdapter {
        let definition = ToolDefinition {
            name: "test_tool".to_string(),
            description: "A test tool for unit testing".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                }
            }),
        };
        let manager = Arc::new(RwLock::new(McpManager::default()));
        McpToolAdapter::new(definition, "test_server".to_string(), manager)
    }

    #[test]
    fn test_name_returns_definition_name() {
        let adapter = make_adapter();
        assert_eq!(adapter.name(), "test_tool");
    }

    #[test]
    fn test_description_returns_definition_description() {
        let adapter = make_adapter();
        assert_eq!(adapter.description(), "A test tool for unit testing");
    }

    #[test]
    fn test_parameters_schema_returns_input_schema() {
        let adapter = make_adapter();
        let schema = adapter.parameters_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["query"].is_object());
    }

    #[test]
    fn test_permission_returns_supervised() {
        let adapter = make_adapter();
        assert_eq!(adapter.permission(), PermissionLevel::Supervised);
    }

    #[test]
    fn test_construction_stores_server_name() {
        let adapter = make_adapter();
        assert_eq!(adapter.server_name, "test_server");
    }

    #[test]
    fn test_construction_with_empty_schema() {
        let definition = ToolDefinition {
            name: "minimal".to_string(),
            description: String::new(),
            input_schema: serde_json::json!({}),
        };
        let manager = Arc::new(RwLock::new(McpManager::default()));
        let adapter = McpToolAdapter::new(definition, "srv".to_string(), manager);
        assert_eq!(adapter.name(), "minimal");
        assert_eq!(adapter.description(), "");
        assert_eq!(adapter.parameters_schema(), serde_json::json!({}));
    }

    // Note: execute() cannot be unit-tested because it requires a real McpManager
    // with a connected MCP server. The call path goes through
    // McpManager::call_tool -> McpClient::send_request which needs a running
    // subprocess. This is best tested via integration tests.
}
