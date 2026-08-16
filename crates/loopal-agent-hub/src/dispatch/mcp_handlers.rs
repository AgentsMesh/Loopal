use std::sync::Arc;

use loopal_protocol::{
    McpCallToolRequest, McpListToolsResponse, McpReconnectRequest, McpReconnectResponse,
    McpSnapshotResponse, McpToolEntry,
};
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::hub::Hub;
use crate::request_principal::AgentPrincipal;

pub async fn handle_mcp_list_tools(
    hub: &Arc<Mutex<Hub>>,
    agent: &AgentPrincipal,
) -> Result<Value, String> {
    let (service, cwd) = exact_context(hub, agent).await?;
    let _ = service.provider_for(&cwd).await;
    let tools = service
        .list_tools_for(&agent.execution, &cwd)
        .await
        .into_iter()
        .map(|(server, definition)| McpToolEntry {
            server,
            name: definition.name,
            description: definition.description,
            input_schema: definition.input_schema,
        })
        .collect();
    serde_json::to_value(McpListToolsResponse { tools })
        .map_err(|error| format!("encode list_tools: {error}"))
}

pub async fn handle_mcp_call_tool(
    hub: &Arc<Mutex<Hub>>,
    params: Value,
    agent: &AgentPrincipal,
) -> Result<Value, String> {
    let request: McpCallToolRequest =
        serde_json::from_value(params).map_err(|e| format!("invalid call_tool params: {e}"))?;
    reject_model_secret_args(&request.args)?;
    let (service, cwd) = exact_context(hub, agent).await?;
    let _ = service.provider_for(&cwd).await;
    let provider = service
        .provider_for_call(&agent.execution, &cwd, &request.server)
        .await
        .ok_or_else(|| {
            format!(
                "no provider found for server '{}' visible to agent '{}'",
                request.server, agent.execution.address
            )
        })?;
    let result = provider
        .call_tool(
            &request.server,
            &request.tool,
            &request.args,
            loopal_mcp::HUB_RPC_BUDGET,
        )
        .await
        .map_err(|error| format!("call_tool: {error}"))?;
    Ok(json!(loopal_mcp::call_result_to_response(&result)))
}

pub async fn handle_mcp_reconnect(
    hub: &Arc<Mutex<Hub>>,
    params: Value,
    agent: &AgentPrincipal,
) -> Result<Value, String> {
    let request: McpReconnectRequest =
        serde_json::from_value(params).map_err(|e| format!("invalid reconnect params: {e}"))?;
    let (service, cwd) = exact_context(hub, agent).await?;
    let _ = service.provider_for(&cwd).await;
    let connected = service
        .reconnect_for(&agent.execution, &cwd, &request.server)
        .await
        .ok_or_else(|| format!("no provider found for server '{}'", request.server))?;
    serde_json::to_value(McpReconnectResponse { connected })
        .map_err(|error| format!("encode reconnect: {error}"))
}

pub async fn handle_mcp_snapshot(
    hub: &Arc<Mutex<Hub>>,
    agent: &AgentPrincipal,
) -> Result<Value, String> {
    let (service, cwd) = exact_context(hub, agent).await?;
    let _ = service.provider_for(&cwd).await;
    let servers = service
        .snapshots_for(&agent.execution, &cwd)
        .await
        .into_iter()
        .map(|snapshot| loopal_protocol::McpServerSnapshot {
            source: String::new(),
            name: snapshot.name,
            transport: snapshot.transport,
            status: snapshot.status,
            tool_count: snapshot.tool_count,
            resource_count: snapshot.resource_count,
            prompt_count: snapshot.prompt_count,
            errors: snapshot.errors,
        })
        .collect();
    serde_json::to_value(McpSnapshotResponse { servers })
        .map_err(|error| format!("encode snapshot: {error}"))
}

async fn exact_context(
    hub: &Arc<Mutex<Hub>>,
    agent: &AgentPrincipal,
) -> Result<(Arc<crate::HubMcpService>, std::path::PathBuf), String> {
    let locked = hub.lock().await;
    if !locked.registry.owns_active_lease(&agent.execution) {
        return Err("stale Agent connection".into());
    }
    let cwd = locked
        .spawn_registry
        .cwd_for(&agent.execution)
        .unwrap_or_else(|| agent.cwd.clone());
    Ok((locked.mcp_service.clone(), cwd))
}

fn reject_model_secret_args(args: &Value) -> Result<(), String> {
    if loopal_mcp::tool_adapter::contains_secret_placeholder(args) {
        return Err(loopal_mcp::tool_adapter::MCP_SECRET_ARG_REJECTION.into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "mcp_handler_tests.rs"]
mod handler_tests;

#[cfg(test)]
mod tests {
    use super::reject_model_secret_args;

    #[test]
    fn rejects_wire_author_and_nested_model_args() {
        for args in [
            serde_json::json!({"token": "<secret_ref:private_name>"}),
            serde_json::json!({"nested": [{"token": "{{secret:private_name}}"}]}),
            serde_json::json!({"{{secret:private_name}}": "value"}),
            serde_json::json!({"<secret_ref:private_name>": "value"}),
        ] {
            let error = reject_model_secret_args(&args).unwrap_err();
            assert_eq!(error, loopal_mcp::tool_adapter::MCP_SECRET_ARG_REJECTION);
            assert!(!error.contains("private_name"));
        }
    }

    #[test]
    fn accepts_ordinary_model_args() {
        assert!(
            reject_model_secret_args(&serde_json::json!({
                "query": "ordinary", "nested": [1, true, null]
            }))
            .is_ok()
        );
    }
}
