use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::protocol::methods;
use loopal_protocol::ROOT_AGENT_NAME;
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::hub::Hub;

pub async fn handle_mcp_list_tools(hub: &Arc<Mutex<Hub>>) -> Result<Value, String> {
    forward_to_root(hub, methods::AGENT_MCP_LIST_TOOLS.name, json!({})).await
}

pub async fn handle_mcp_call_tool(hub: &Arc<Mutex<Hub>>, params: Value) -> Result<Value, String> {
    forward_to_root(hub, methods::AGENT_MCP_CALL_TOOL.name, params).await
}

pub async fn handle_mcp_snapshot(hub: &Arc<Mutex<Hub>>) -> Result<Value, String> {
    forward_to_root(hub, methods::AGENT_MCP_SNAPSHOT.name, json!({})).await
}

/// reason: JSON-RPC has no cancellation. If sub-agent's `proxy_rpc_timeout`
/// elapses, its future is dropped — but the hub's `send_request` future
/// stays pending forever, leaking a dispatch task. Cap the hub side at a
/// slightly shorter budget so it always finishes first.
fn hub_forward_timeout() -> Duration {
    let secs = std::env::var("LOOPAL_HUB_MCP_FORWARD_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(25);
    Duration::from_secs(secs)
}

async fn forward_to_root(
    hub: &Arc<Mutex<Hub>>,
    method: &str,
    params: Value,
) -> Result<Value, String> {
    let conn = {
        let h = hub.lock().await;
        h.registry
            .get_agent_connection(ROOT_AGENT_NAME)
            .ok_or_else(|| format!("root agent '{ROOT_AGENT_NAME}' not registered"))?
    };
    let deadline = hub_forward_timeout();
    match tokio::time::timeout(deadline, conn.send_request(method, params)).await {
        Ok(inner) => inner.map_err(|e| format!("{method} to root agent failed: {e}")),
        Err(_) => Err(format!(
            "{method} forward to root timed out after {:?}",
            deadline
        )),
    }
}
