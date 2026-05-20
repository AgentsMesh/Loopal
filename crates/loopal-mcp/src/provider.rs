use async_trait::async_trait;
use loopal_error::McpError;
use loopal_tool_api::ToolDefinition;
use rmcp::model::CallToolResult;
use serde_json::Value;

use crate::manager_query::McpConnectionSnapshot;

/// MCP capability surface visible to tool adapters and the kernel's
/// dispatch path. Implementations: `LocalMcpProvider` (in-process,
/// owns connections) and `McpProxyClient` (forwards via Hub IPC to
/// the root agent).
///
/// Lifecycle concerns (wait-until-settled, reconnect policy) are
/// owner-only and intentionally NOT on this trait — they live on
/// the concrete `LocalMcpProvider` so the LSP contract here stays
/// honest: every method must behave the same regardless of impl.
#[async_trait]
pub trait McpProvider: Send + Sync {
    async fn list_tools(&self) -> Vec<(String, ToolDefinition)>;

    async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        args: &Value,
    ) -> Result<CallToolResult, McpError>;

    async fn snapshot(&self) -> Vec<McpConnectionSnapshot>;
}
