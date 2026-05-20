use std::sync::Arc;

use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolDefinition, ToolResult};
use rmcp::model::CallToolResult;
use serde_json::Value;
use tracing::Instrument;

use crate::provider::McpProvider;

pub struct McpToolAdapter {
    definition: ToolDefinition,
    server_name: String,
    provider: Arc<dyn McpProvider>,
}

impl McpToolAdapter {
    pub fn new(
        definition: ToolDefinition,
        server_name: String,
        provider: Arc<dyn McpProvider>,
    ) -> Self {
        Self {
            definition,
            server_name,
            provider,
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
        PermissionLevel::Write
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let mcp_span =
            tracing::info_span!("mcp_tool_call", mcp.tool = self.definition.name.as_str());
        async {
            let result = self
                .provider
                .call_tool(&self.server_name, &self.definition.name, &input)
                .await
                .map_err(LoopalError::Mcp)?;

            Ok(convert_tool_result(&result))
        }
        .instrument(mcp_span)
        .await
    }
}

fn convert_tool_result(result: &CallToolResult) -> ToolResult {
    let parts: Vec<String> = result.content.iter().filter_map(content_to_text).collect();

    ToolResult {
        content: parts.join("\n"),
        is_error: result.is_error.unwrap_or(false),
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
    use crate::local_provider::LocalMcpProvider;
    use crate::manager::McpManager;
    use tokio::sync::RwLock;

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
        let provider: Arc<dyn McpProvider> = Arc::new(LocalMcpProvider::new(manager));
        McpToolAdapter::new(definition, "test_server".to_string(), provider)
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
        assert_eq!(adapter.permission(), PermissionLevel::Write);
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
        let provider: Arc<dyn McpProvider> = Arc::new(LocalMcpProvider::new(manager));
        let adapter = McpToolAdapter::new(definition, "srv".to_string(), provider);
        assert_eq!(adapter.name(), "minimal");
        assert_eq!(adapter.description(), "");
        assert_eq!(adapter.parameters_schema(), serde_json::json!({}));
    }
}
