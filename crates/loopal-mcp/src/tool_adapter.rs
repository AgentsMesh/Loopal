use std::sync::Arc;

use async_trait::async_trait;
use loopal_error::{LoopalError, McpError};
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolDefinition, ToolResult};
use rmcp::model::CallToolResult;
use serde_json::Value;
use tokio::sync::RwLock;
use tracing::warn;

use crate::manager::McpManager;
use crate::reconnect::ReconnectPolicy;

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

    async fn is_reconnectable(&self) -> bool {
        let mgr = self.manager.read().await;
        mgr.connections
            .get(&self.server_name)
            .is_some_and(|c| ReconnectPolicy::is_reconnectable(&c.config))
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
        PermissionLevel::Supervised
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        // Fast path: read lock for normal tool calls (allows concurrency).
        let result = {
            let mgr = self.manager.read().await;
            mgr.call_tool(&self.server_name, &self.definition.name, &input)
                .await
        };

        let result = match result {
            Ok(val) => val,
            Err(McpError::TransportClosed(_)) if self.is_reconnectable().await => {
                warn!(server = %self.server_name, tool = %self.definition.name, "transport closed, reconnecting");
                {
                    let mut mgr = self.manager.write().await;
                    let _ = mgr.restart_connection(&self.server_name).await;
                }
                let mgr = self.manager.read().await;
                mgr.call_tool(&self.server_name, &self.definition.name, &input)
                    .await
                    .map_err(LoopalError::Mcp)?
            }
            Err(e) => return Err(LoopalError::Mcp(e)),
        };

        Ok(convert_tool_result(&result))
    }
}

/// Convert rmcp `CallToolResult` to Loopal `ToolResult` without serialization.
///
/// Extracts text from all content blocks. Non-text content (images, resources)
/// is represented as descriptive placeholders since Loopal's ToolResult is
/// text-only.
fn convert_tool_result(result: &CallToolResult) -> ToolResult {
    let parts: Vec<String> = result.content.iter().filter_map(content_to_text).collect();

    ToolResult {
        content: parts.join("\n"),
        is_error: result.is_error.unwrap_or(false),
        is_completion: false,
        metadata: None,
    }
}

fn content_to_text(content: &rmcp::model::Content) -> Option<String> {
    use rmcp::model::{RawContent, ResourceContents};

    match &content.raw {
        RawContent::Text(t) => Some(t.text.clone()),
        RawContent::Image(img) => Some(format!(
            "![image](data:{};base64,{})",
            img.mime_type, img.data
        )),
        RawContent::Audio(audio) => Some(format!("[audio: {}]", audio.mime_type)),
        RawContent::Resource(res) => match &res.resource {
            ResourceContents::TextResourceContents { uri, text, .. } => {
                Some(format!("[resource {uri}]\n{text}"))
            }
            ResourceContents::BlobResourceContents { uri, .. } => {
                Some(format!("[binary resource: {uri}]"))
            }
        },
        RawContent::ResourceLink(link) => Some(format!("[resource: {}]", link.uri)),
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
}
