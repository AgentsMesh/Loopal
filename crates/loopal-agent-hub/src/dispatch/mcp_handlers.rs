use std::sync::Arc;

use loopal_protocol::{
    McpCallToolRequest, McpListToolsResponse, McpSnapshotResponse, McpToolEntry,
};
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::hub::Hub;

pub async fn handle_mcp_list_tools(
    hub: &Arc<Mutex<Hub>>,
    from_agent: &str,
) -> Result<Value, String> {
    let (mcp_service, cwd) = {
        let h = hub.lock().await;
        let cwd = h
            .spawn_registry
            .cwd_of(from_agent)
            .unwrap_or_else(|| h.default_cwd.clone());
        (h.mcp_service.clone(), cwd)
    };
    // Touch hub-singleton for this cwd so the lazy provider is alive
    let _ = mcp_service.provider_for(&cwd).await;
    let tools: Vec<McpToolEntry> = mcp_service
        .list_tools_for(from_agent, &cwd)
        .await
        .into_iter()
        .map(|(server, def)| McpToolEntry {
            server,
            name: def.name,
            description: def.description,
            input_schema: def.input_schema,
        })
        .collect();
    serde_json::to_value(McpListToolsResponse { tools })
        .map_err(|e| format!("encode list_tools: {e}"))
}

pub async fn handle_mcp_call_tool(
    hub: &Arc<Mutex<Hub>>,
    params: Value,
    from_agent: &str,
) -> Result<Value, String> {
    let mut req: McpCallToolRequest =
        serde_json::from_value(params).map_err(|e| format!("invalid call_tool params: {e}"))?;
    let (mcp_service, cwd) = {
        let h = hub.lock().await;
        let cwd = h
            .spawn_registry
            .cwd_of(from_agent)
            .unwrap_or_else(|| h.default_cwd.clone());
        (h.mcp_service.clone(), cwd)
    };
    let _ = mcp_service.provider_for(&cwd).await;
    let provider = mcp_service
        .provider_for_call(from_agent, &cwd, &req.server)
        .await
        .ok_or_else(|| {
            format!(
                "no provider found for server '{}' visible to agent '{from_agent}'",
                req.server
            )
        })?;
    expand_secret_refs_in_args(hub, &cwd, from_agent, &req.tool, &mut req.args).await;
    let result = provider
        .call_tool(&req.server, &req.tool, &req.args)
        .await
        .map_err(|e| format!("call_tool: {e}"))?;
    Ok(json!(loopal_mcp::call_result_to_response(&result)))
}

pub async fn handle_mcp_snapshot(hub: &Arc<Mutex<Hub>>, from_agent: &str) -> Result<Value, String> {
    let (provider, _cwd) = resolve_provider(hub, from_agent).await?;
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
    serde_json::to_value(McpSnapshotResponse { servers })
        .map_err(|e| format!("encode snapshot: {e}"))
}

async fn resolve_provider(
    hub: &Arc<Mutex<Hub>>,
    from_agent: &str,
) -> Result<(Arc<dyn loopal_mcp::McpProvider>, std::path::PathBuf), String> {
    let (mcp_service, cwd) = {
        let h = hub.lock().await;
        let cwd = h
            .spawn_registry
            .cwd_of(from_agent)
            .unwrap_or_else(|| h.default_cwd.clone());
        (h.mcp_service.clone(), cwd)
    };
    let provider = mcp_service.provider_for(&cwd).await;
    Ok((provider, cwd))
}

async fn expand_secret_refs_in_args(
    hub: &Arc<Mutex<Hub>>,
    cwd: &std::path::Path,
    from_agent: &str,
    tool_name: &str,
    args: &mut Value,
) {
    let vault = { hub.lock().await.vault_service.clone() };
    let Some(vault) = vault else {
        return;
    };
    let ctx = loopal_hub_vault::AuditContext {
        agent_name: from_agent.to_string(),
        depth: 0,
        tool_name: Some(tool_name.to_string()),
    };
    walk_and_expand(args, &vault, cwd, &ctx).await;
}

fn walk_and_expand<'a>(
    value: &'a mut Value,
    vault: &'a loopal_hub_vault::HubVaultService,
    cwd: &'a std::path::Path,
    ctx: &'a loopal_hub_vault::AuditContext,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
    Box::pin(async move {
        match value {
            Value::String(s) => {
                if !s.contains("<secret_ref:") {
                    return;
                }
                if let Ok(expanded) = vault.expand_wire(cwd, s, ctx.clone()).await {
                    use loopal_secret_client::ExposeSecret;
                    *s = expanded.expose_secret().to_string();
                }
            }
            Value::Array(arr) => {
                for item in arr.iter_mut() {
                    walk_and_expand(item, vault, cwd, ctx).await;
                }
            }
            Value::Object(map) => {
                for (_k, v) in map.iter_mut() {
                    walk_and_expand(v, vault, cwd, ctx).await;
                }
            }
            _ => {}
        }
    })
}
