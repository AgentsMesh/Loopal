use std::sync::Arc;

use async_trait::async_trait;
use loopal_error::McpError;
use loopal_ipc::IpcBudget;
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

#[async_trait]
pub trait HubMcpClient: Send + Sync {
    async fn send_request(&self, method: &str, params: Value) -> Result<Value, String>;
}

pub struct McpProxyClient {
    client: Arc<dyn HubMcpClient>,
}

enum ProxyRpcError {
    Forbidden,
    TimedOut,
    Remote,
}

impl McpProxyClient {
    pub fn new(client: Arc<dyn HubMcpClient>) -> Self {
        Self { client }
    }

    async fn rpc(
        &self,
        method: &str,
        params: Value,
        budget: IpcBudget,
    ) -> Result<Value, ProxyRpcError> {
        let timeout = match budget {
            IpcBudget::Forbidden => return Err(ProxyRpcError::Forbidden),
            IpcBudget::Allowed(d) => d,
        };
        match tokio::time::timeout(timeout, self.client.send_request(method, params)).await {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(_)) => Err(ProxyRpcError::Remote),
            Err(_) => Err(ProxyRpcError::TimedOut),
        }
    }
}

#[async_trait]
impl McpProvider for McpProxyClient {
    async fn list_tools(&self, budget: IpcBudget) -> Vec<(String, ToolDefinition)> {
        let resp = match self
            .rpc(
                methods::HUB_MCP_LIST_TOOLS.name,
                Value::Object(Default::default()),
                budget,
            )
            .await
        {
            Ok(v) => v,
            Err(_) => {
                tracing::warn!("hub/mcp/list_tools failed");
                return Vec::new();
            }
        };
        let parsed: McpListToolsResponse = match serde_json::from_value(resp) {
            Ok(p) => p,
            Err(_) => {
                tracing::warn!("hub/mcp/list_tools returned invalid payload");
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
        budget: IpcBudget,
    ) -> Result<CallToolResult, McpError> {
        let req = McpCallToolRequest {
            server: server.to_string(),
            tool: tool.to_string(),
            args: args.clone(),
        };
        let params = serde_json::to_value(&req)
            .map_err(|_| McpError::Protocol("encode call_tool failed".into()))?;
        let resp = self
            .rpc(methods::HUB_MCP_CALL_TOOL.name, params, budget)
            .await
            .map_err(|error| match error {
                ProxyRpcError::Forbidden => {
                    McpError::TransportClosed("IpcBudget::Forbidden on critical path".into())
                }
                ProxyRpcError::TimedOut => McpError::Timeout("hub/mcp/call_tool timed out".into()),
                ProxyRpcError::Remote => {
                    McpError::TransportClosed("hub/mcp/call_tool failed".into())
                }
            })?;
        let parsed: McpCallToolResponse = serde_json::from_value(resp)
            .map_err(|_| McpError::Protocol("invalid call_tool response".into()))?;

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

    async fn snapshot(&self, budget: IpcBudget) -> Vec<McpConnectionSnapshot> {
        let resp = match self
            .rpc(
                methods::HUB_MCP_SNAPSHOT.name,
                Value::Object(Default::default()),
                budget,
            )
            .await
        {
            Ok(v) => v,
            Err(_) => {
                tracing::warn!("hub/mcp/snapshot failed");
                return Vec::new();
            }
        };
        let parsed: McpSnapshotResponse = match serde_json::from_value(resp) {
            Ok(p) => p,
            Err(_) => {
                tracing::warn!("hub/mcp/snapshot returned invalid payload");
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
