use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use loopal_error::McpError;
use loopal_ipc::protocol::methods;
use loopal_protocol::{
    McpCallToolRequest, McpCallToolResponse, McpListToolsResponse, McpSnapshotResponse,
};
use loopal_tool_api::ToolDefinition;
use rmcp::model::{CallToolResult, Content};
use serde_json::Value;

use crate::manager_query::McpConnectionSnapshot;
use crate::provider::McpProvider;
use crate::tool_result_text::block_to_text;

/// Per-request IPC deadline. Defends against root agent dying mid-call
/// or a hub that accepts the request but never forwards it. Overridable
/// via `LOOPAL_MCP_PROXY_RPC_TIMEOUT_SECS` for slow MCP servers.
fn proxy_rpc_timeout() -> Duration {
    let secs = std::env::var("LOOPAL_MCP_PROXY_RPC_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(30);
    Duration::from_secs(secs)
}

#[async_trait]
pub trait HubMcpClient: Send + Sync {
    async fn send_request(&self, method: &str, params: Value) -> Result<Value, String>;
}

pub struct McpProxyClient {
    client: Arc<dyn HubMcpClient>,
}

impl McpProxyClient {
    pub fn new(client: Arc<dyn HubMcpClient>) -> Self {
        Self { client }
    }

    async fn rpc(&self, method: &str, params: Value) -> Result<Value, String> {
        match tokio::time::timeout(proxy_rpc_timeout(), self.client.send_request(method, params))
            .await
        {
            Ok(inner) => inner,
            Err(_) => Err(format!(
                "{method} timed out after {:?}",
                proxy_rpc_timeout()
            )),
        }
    }
}

#[async_trait]
impl McpProvider for McpProxyClient {
    async fn list_tools(&self) -> Vec<(String, ToolDefinition)> {
        let resp = match self
            .rpc(methods::HUB_MCP_LIST_TOOLS.name, Value::Object(Default::default()))
            .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "hub/mcp/list_tools failed");
                return Vec::new();
            }
        };
        let parsed: McpListToolsResponse = match serde_json::from_value(resp) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, "hub/mcp/list_tools: invalid payload");
                return Vec::new();
            }
        };
        parsed
            .tools
            .into_iter()
            .map(|entry| {
                (
                    entry.server,
                    ToolDefinition {
                        name: entry.name,
                        description: entry.description,
                        input_schema: entry.input_schema,
                    },
                )
            })
            .collect()
    }

    async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        args: &Value,
    ) -> Result<CallToolResult, McpError> {
        let req = McpCallToolRequest {
            server: server.to_string(),
            tool: tool.to_string(),
            args: args.clone(),
        };
        let params = serde_json::to_value(&req)
            .map_err(|e| McpError::Protocol(format!("encode call_tool: {e}")))?;
        let resp = self
            .rpc(methods::HUB_MCP_CALL_TOOL.name, params)
            .await
            .map_err(|e| McpError::TransportClosed(format!("hub/mcp/call_tool: {e}")))?;
        let parsed: McpCallToolResponse = serde_json::from_value(resp)
            .map_err(|e| McpError::Protocol(format!("call_tool response: {e}")))?;

        let content: Vec<Content> = parsed
            .content
            .iter()
            .map(|block| Content::text(block_to_text(block)))
            .collect();
        Ok(if parsed.is_error {
            CallToolResult::error(content)
        } else {
            CallToolResult::success(content)
        })
    }

    async fn snapshot(&self) -> Vec<McpConnectionSnapshot> {
        let resp = match self
            .rpc(methods::HUB_MCP_SNAPSHOT.name, Value::Object(Default::default()))
            .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "hub/mcp/snapshot failed");
                return Vec::new();
            }
        };
        let parsed: McpSnapshotResponse = match serde_json::from_value(resp) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, "hub/mcp/snapshot: invalid payload");
                return Vec::new();
            }
        };
        parsed
            .servers
            .into_iter()
            .map(|s| McpConnectionSnapshot {
                name: s.name,
                transport: s.transport,
                status: s.status,
                tool_count: s.tool_count,
                resource_count: s.resource_count,
                prompt_count: s.prompt_count,
                errors: s.errors,
            })
            .collect()
    }
}
