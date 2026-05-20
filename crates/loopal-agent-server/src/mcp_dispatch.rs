use std::time::Duration;

use loopal_protocol::{
    McpCallToolRequest, McpListToolsResponse, McpSnapshotResponse, McpToolEntry,
};
use serde_json::Value;

use crate::dispatch::RpcErrorPayload;
use crate::session_hub::SessionHub;

/// reason: dispatch_loop processes incoming IPC sequentially. Without a
/// timeout on root-side `call_tool`, a single slow MCP server (chrome
/// navigate, image gen, etc.) would block every subsequent `hub/mcp/*`
/// request via head-of-line blocking. Worse: when `hub_forward_timeout`
/// (25s) elapses while root is still awaiting, root's response is
/// silently dropped — a successful tool call becomes invisible to the
/// sub-agent. Cap root's wait below hub's so root always fails first.
fn agent_mcp_call_timeout() -> Duration {
    let secs = std::env::var("LOOPAL_MCP_CALL_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(20);
    Duration::from_secs(secs)
}

pub async fn handle_list_tools(hub: &SessionHub) -> Result<Value, RpcErrorPayload> {
    let Some(provider) = hub.mcp_provider().await else {
        return ok(McpListToolsResponse { tools: Vec::new() });
    };
    let tools = provider
        .list_tools()
        .await
        .into_iter()
        .map(|(server, def)| McpToolEntry {
            server,
            name: def.name,
            description: def.description,
            input_schema: def.input_schema,
        })
        .collect();
    ok(McpListToolsResponse { tools })
}

pub async fn handle_call_tool(
    hub: &SessionHub,
    params: Value,
) -> Result<Value, RpcErrorPayload> {
    let req: McpCallToolRequest = serde_json::from_value(params)
        .map_err(|e| RpcErrorPayload::internal(format!("invalid call_tool params: {e}")))?;

    let provider = hub
        .mcp_provider()
        .await
        .ok_or_else(|| RpcErrorPayload::internal("no MCP provider attached to root agent"))?;

    let deadline = agent_mcp_call_timeout();
    let result = match tokio::time::timeout(
        deadline,
        provider.call_tool(&req.server, &req.tool, &req.args),
    )
    .await
    {
        Ok(inner) => inner.map_err(|e| RpcErrorPayload::internal(format!("call_tool: {e}")))?,
        Err(_) => {
            return Err(RpcErrorPayload::internal(format!(
                "call_tool: '{}/{}' exceeded {:?}",
                req.server, req.tool, deadline
            )));
        }
    };

    ok(loopal_mcp::call_result_to_response(&result))
}

pub async fn handle_snapshot(hub: &SessionHub) -> Result<Value, RpcErrorPayload> {
    let Some(provider) = hub.mcp_provider().await else {
        return ok(McpSnapshotResponse {
            servers: Vec::new(),
        });
    };
    let servers = provider
        .snapshot()
        .await
        .into_iter()
        .map(|s| loopal_protocol::McpServerSnapshot {
            source: String::new(),
            name: s.name,
            transport: s.transport,
            status: s.status,
            tool_count: s.tool_count,
            resource_count: s.resource_count,
            prompt_count: s.prompt_count,
            errors: s.errors,
        })
        .collect();
    ok(McpSnapshotResponse { servers })
}

fn ok<T: serde::Serialize>(value: T) -> Result<Value, RpcErrorPayload> {
    serde_json::to_value(value).map_err(|e| RpcErrorPayload::internal(format!("encode: {e}")))
}

