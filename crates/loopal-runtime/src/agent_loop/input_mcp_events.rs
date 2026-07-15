use std::collections::HashMap;
use std::sync::{Arc, Weak};

use loopal_kernel::Kernel;
use loopal_mcp::LocalMcpProvider;
use loopal_protocol::{AgentEventPayload, McpServerSnapshot};

use crate::frontend::AgentFrontend;

pub(super) fn spawn(
    local: Option<Arc<LocalMcpProvider>>,
    kernel: Weak<Kernel>,
    frontend: Weak<dyn AgentFrontend>,
    cwd: String,
) {
    let sources = source_map(&cwd);
    if sources.is_empty() {
        return;
    }
    if let Some(local) = local {
        let mut changes = local.subscribe_settle_events();
        tokio::spawn(async move {
            while changes.changed().await.is_ok() {
                if !emit(&kernel, &frontend, &sources).await {
                    break;
                }
            }
        });
        return;
    }
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(proxy_poll_interval());
        interval.tick().await;
        let mut previous = None;
        loop {
            interval.tick().await;
            let Some(current) = snapshot(&kernel, &sources).await else {
                break;
            };
            let signature = serde_json::to_string(&current).ok();
            if signature == previous {
                continue;
            }
            let Some(active_kernel) = kernel.upgrade() else {
                break;
            };
            active_kernel.register_all_settled_mcp_tools().await;
            drop(active_kernel);
            previous = signature;
            let Some(frontend) = frontend.upgrade() else {
                break;
            };
            if frontend
                .emit(AgentEventPayload::McpStatusReport { servers: current })
                .await
                .is_err()
            {
                break;
            }
        }
    });
}

async fn emit(
    kernel: &Weak<Kernel>,
    frontend: &Weak<dyn AgentFrontend>,
    sources: &HashMap<String, String>,
) -> bool {
    let Some(servers) = snapshot(kernel, sources).await else {
        return false;
    };
    let Some(frontend) = frontend.upgrade() else {
        return false;
    };
    frontend
        .emit(AgentEventPayload::McpStatusReport { servers })
        .await
        .is_ok()
}

async fn snapshot(
    kernel: &Weak<Kernel>,
    sources: &HashMap<String, String>,
) -> Option<Vec<McpServerSnapshot>> {
    let kernel = kernel.upgrade()?;
    Some(
        kernel
            .mcp_provider()
            .snapshot(loopal_mcp::HUB_RPC_BUDGET)
            .await
            .into_iter()
            .map(|server| McpServerSnapshot {
                source: sources
                    .get(&server.name)
                    .cloned()
                    .unwrap_or_else(|| "unknown".into()),
                name: server.name,
                transport: server.transport,
                status: server.status,
                tool_count: server.tool_count,
                resource_count: server.resource_count,
                prompt_count: server.prompt_count,
                errors: server.errors,
            })
            .collect(),
    )
}

fn source_map(cwd: &str) -> HashMap<String, String> {
    loopal_config::load_config(std::path::Path::new(cwd))
        .map(|config| {
            config
                .mcp_servers
                .into_iter()
                .map(|(name, entry)| (name, entry.source.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn proxy_poll_interval() -> std::time::Duration {
    let seconds = std::env::var("LOOPAL_MCP_POLL_SECS")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| *value > 0)
        .unwrap_or(3);
    std::time::Duration::from_secs(seconds)
}
