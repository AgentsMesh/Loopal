use loopal_error::McpError;
use loopal_ipc::IpcBudget;
use loopal_ipc::protocol::methods;
use loopal_protocol::{McpReconnectRequest, McpReconnectResponse};

use super::proxy_client::{McpProxyClient, ProxyRpcError};

pub(super) async fn reconnect(
    client: &McpProxyClient,
    server: &str,
    budget: IpcBudget,
) -> Result<(), McpError> {
    let params = serde_json::to_value(McpReconnectRequest {
        server: server.to_string(),
    })
    .map_err(|_| McpError::Protocol("encode reconnect failed".into()))?;
    let response = client
        .rpc(methods::HUB_MCP_RECONNECT.name, params, budget)
        .await
        .map_err(|error| match error {
            ProxyRpcError::Forbidden => {
                McpError::TransportClosed("IpcBudget::Forbidden on critical path".into())
            }
            ProxyRpcError::TimedOut => McpError::Timeout("hub/mcp/reconnect timed out".into()),
            ProxyRpcError::Remote => McpError::TransportClosed("hub/mcp/reconnect failed".into()),
        })?;
    let response: McpReconnectResponse = serde_json::from_value(response)
        .map_err(|_| McpError::Protocol("invalid reconnect response".into()))?;
    response
        .connected
        .then_some(())
        .ok_or_else(|| McpError::ConnectionFailed("Hub MCP reconnect failed".into()))
}
