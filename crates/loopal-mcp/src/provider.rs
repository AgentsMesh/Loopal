use async_trait::async_trait;
use loopal_error::McpError;
use loopal_ipc::IpcBudget;
use loopal_tool_api::ToolDefinition;
use rmcp::model::CallToolResult;
use serde_json::Value;

use crate::manager_query::McpConnectionSnapshot;

/// MCP capability surface visible to tool adapters and the kernel's
/// dispatch path.
///
/// Every method takes an `IpcBudget` so the call-site is explicit about
/// "how patient am I?" — even for the in-process `LocalMcpProvider` which
/// has nothing to wait on, the parameter forces the caller to reason
/// about latency. Remote implementations (`McpProxyClient`) respect the
/// budget; `Forbidden` causes immediate `McpError::TransportClosed`.
#[async_trait]
pub trait McpProvider: Send + Sync {
    async fn list_tools(&self, budget: IpcBudget) -> Vec<(String, ToolDefinition)>;

    async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        args: &Value,
        budget: IpcBudget,
    ) -> Result<CallToolResult, McpError>;

    async fn snapshot(&self, budget: IpcBudget) -> Vec<McpConnectionSnapshot>;
}
