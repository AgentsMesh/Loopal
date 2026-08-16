use super::*;
use async_trait::async_trait;
use loopal_backend::{LocalBackend, ResourceLimits};
use loopal_error::{LoopalError, McpError};
use loopal_ipc::IpcBudget;
use loopal_tool_api::ToolContext;
use rmcp::model::{CallToolResult, Content, RawResource, ResourceContents};
use tokio::sync::RwLock;

use crate::BINARY_DENIED_MARKER;
use crate::local_provider::LocalMcpProvider;
use crate::manager::McpManager;
use crate::manager_query::McpConnectionSnapshot;

fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "test_tool".to_string(),
        description: "A test tool for unit testing".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {"query": {"type": "string"}}
        }),
    }
}

fn make_adapter() -> McpToolAdapter {
    let definition = definition();
    let manager = Arc::new(RwLock::new(McpManager::default()));
    let provider: Arc<dyn McpProvider> = Arc::new(LocalMcpProvider::new(manager));
    McpToolAdapter::new(definition, "test_server".to_string(), provider)
}

#[test]
fn exposes_definition_and_permission() {
    let adapter = make_adapter();
    assert_eq!(adapter.name(), "test_tool");
    assert_eq!(adapter.description(), "A test tool for unit testing");
    assert_eq!(adapter.parameters_schema()["type"], "object");
    assert_eq!(adapter.permission(), PermissionLevel::Write);
    assert_eq!(adapter.server_name, "test_server");
    assert!(adapter.secret_eligible_params().is_empty());
}

#[test]
fn rejects_model_supplied_author_and_wire_placeholders_recursively() {
    let adapter = make_adapter();
    for input in [
        serde_json::json!({"token": "<secret_ref:api_key>"}),
        serde_json::json!({"nested": [{"token": "{{secret:api_key}}"}]}),
        serde_json::json!({"{{secret:api_key}}": "value"}),
        serde_json::json!({"<secret_ref:api_key>": "value"}),
    ] {
        let rejection = adapter.precheck(&input).unwrap();
        assert_eq!(rejection, MCP_SECRET_ARG_REJECTION);
        assert!(!rejection.contains("api_key"));
    }
}

#[test]
fn ordinary_model_arguments_pass_precheck() {
    assert!(
        make_adapter()
            .precheck(&serde_json::json!({"query": "ordinary", "nested": [1, true]}))
            .is_none()
    );
}

#[test]
fn supports_empty_schema() {
    let definition = ToolDefinition {
        name: "minimal".to_string(),
        description: String::new(),
        input_schema: serde_json::json!({}),
    };
    let manager = Arc::new(RwLock::new(McpManager::default()));
    let provider: Arc<dyn McpProvider> = Arc::new(LocalMcpProvider::new(manager));
    let adapter = McpToolAdapter::new(definition, "srv".to_string(), provider);
    assert_eq!(adapter.parameters_schema(), serde_json::json!({}));
}

struct StubProvider {
    result: CallToolResult,
    error: Option<String>,
}

#[async_trait]
impl McpProvider for StubProvider {
    async fn list_tools(&self, _budget: IpcBudget) -> Vec<(String, ToolDefinition)> {
        Vec::new()
    }

    async fn call_tool(
        &self,
        _server: &str,
        _tool: &str,
        _args: &Value,
        _budget: IpcBudget,
    ) -> Result<CallToolResult, McpError> {
        match &self.error {
            Some(message) => Err(McpError::ServerNotFound(message.clone())),
            None => Ok(self.result.clone()),
        }
    }

    async fn snapshot(&self, _budget: IpcBudget) -> Vec<McpConnectionSnapshot> {
        Vec::new()
    }
}

fn adapter_with_result(result: CallToolResult) -> McpToolAdapter {
    McpToolAdapter::new(
        definition(),
        "server".into(),
        Arc::new(StubProvider {
            result,
            error: None,
        }),
    )
}

fn error_adapter(message: &str) -> McpToolAdapter {
    McpToolAdapter::new(
        definition(),
        "server".into(),
        Arc::new(StubProvider {
            result: CallToolResult::default(),
            error: Some(message.into()),
        }),
    )
}

fn context() -> ToolContext {
    ToolContext::new(
        LocalBackend::new(
            std::env::temp_dir(),
            None,
            ResourceLimits::default(),
            "mcp-adapter-test",
        ),
        "mcp-adapter-test",
    )
}

#[tokio::test]
async fn executes_provider_and_converts_every_content_kind() {
    let audio: Content = serde_json::from_value(serde_json::json!({
        "type": "audio", "data": "YQ==", "mimeType": "audio/wav"
    }))
    .unwrap();
    let link = RawResource::new("memory://link", "link");
    let result = CallToolResult::error(vec![
        Content::text("plain"),
        Content::image("aW1hZ2U=", "image/png"),
        audio,
        Content::embedded_text("memory://text", "body"),
        Content::resource(ResourceContents::blob("YmxvYg==", "memory://blob")),
        Content::resource_link(link),
    ]);
    let adapter = adapter_with_result(result);

    let converted = adapter
        .execute(serde_json::json!({"query": "value"}), &context())
        .await
        .unwrap();

    assert!(converted.is_error);
    assert!(converted.content.contains("plain"));
    assert_eq!(converted.content.matches(BINARY_DENIED_MARKER).count(), 3);
    assert!(!converted.content.contains("aW1hZ2U="));
    assert!(!converted.content.contains("audio/wav"));
    assert!(!converted.content.contains("memory://blob"));
    assert!(converted.content.contains("[resource memory://text]\nbody"));
    assert!(converted.content.contains("[resource: memory://link]"));
}

#[tokio::test]
async fn maps_provider_errors_to_loopal_mcp_errors() {
    let adapter = error_adapter("missing");
    let error = adapter
        .execute(serde_json::json!({}), &context())
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        LoopalError::Mcp(McpError::ServerNotFound(_))
    ));
}
