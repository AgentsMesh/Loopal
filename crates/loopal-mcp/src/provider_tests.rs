use async_trait::async_trait;
use loopal_error::McpError;
use loopal_ipc::IpcBudget;
use loopal_tool_api::ToolDefinition;
use rmcp::model::CallToolResult;
use serde_json::Value;

use super::McpProvider;
use crate::manager_query::McpConnectionSnapshot;

struct Unsupported;

#[async_trait]
impl McpProvider for Unsupported {
    async fn list_tools(&self, _: IpcBudget) -> Vec<(String, ToolDefinition)> {
        Vec::new()
    }

    async fn call_tool(
        &self,
        _: &str,
        _: &str,
        _: &Value,
        _: IpcBudget,
    ) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::default())
    }

    async fn snapshot(&self, _: IpcBudget) -> Vec<McpConnectionSnapshot> {
        Vec::new()
    }
}

#[tokio::test]
async fn reconnect_defaults_to_fail_closed() {
    let error = Unsupported
        .reconnect("server", IpcBudget::forbidden())
        .await
        .unwrap_err();
    assert!(matches!(error, McpError::CapabilityNotSupported(_)));
}
