//! MCP control command handlers — status query, reconnect, and disconnect.

use std::collections::HashMap;
use std::sync::Arc;

use loopal_error::Result;
use loopal_protocol::{AgentEventPayload, McpServerSnapshot};
use tracing::{error, info, warn};

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Emit initial MCP status after agent startup (best-effort, errors logged).
    pub(super) async fn emit_initial_mcp_status(&self) {
        let snapshots = self.collect_mcp_snapshots().await;
        if let Err(e) = self
            .emit(AgentEventPayload::McpStatusReport { servers: snapshots })
            .await
        {
            tracing::warn!(error = %e, "failed to emit initial MCP status");
        }
    }

    /// Spawn a background task that pushes McpStatusReport whenever a late
    /// MCP server finishes connecting. Without this, slow servers that
    /// settle AFTER the bounded-wait budget would never surface a state
    /// change to the TUI until the user manually opens `/mcp` and the
    /// QueryMcpStatus control command pulls a fresh snapshot.
    pub(super) fn spawn_mcp_settle_emitter(&self) {
        let Some(local) = self.params.deps.kernel.local_mcp_provider() else {
            return;
        };
        // reason: hold only Weak refs so this detached task does NOT
        // prolong the lifetimes of Kernel / Frontend after the agent loop
        // exits — otherwise IPC connection close handshake stalls (the
        // client-side rx never observes EOF because Frontend keeps its
        // outbound sender alive).
        let weak_kernel = Arc::downgrade(&self.params.deps.kernel);
        let weak_frontend = Arc::downgrade(&self.params.deps.frontend);
        let cwd = self.params.session.cwd.clone();
        let mut rx = local.subscribe_settle_events();
        drop(local);
        tokio::spawn(async move {
            while rx.changed().await.is_ok() {
                let Some(k) = weak_kernel.upgrade() else {
                    break;
                };
                let Some(f) = weak_frontend.upgrade() else {
                    break;
                };
                let snapshots = collect_mcp_snapshots_via_provider(&k, &cwd).await;
                if f.emit(AgentEventPayload::McpStatusReport { servers: snapshots })
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });
    }

    pub(super) async fn handle_query_mcp_status(&mut self) -> Result<()> {
        let snapshots = self.collect_mcp_snapshots().await;
        self.emit(AgentEventPayload::McpStatusReport { servers: snapshots })
            .await
    }

    pub(super) async fn handle_mcp_reconnect(&mut self, server: String) -> Result<()> {
        info!(server = %server, "reconnecting MCP server");
        match self.params.deps.kernel.mcp_manager() {
            Some(mgr) => {
                let result = mgr.write().await.restart_connection(&server).await;
                if let Err(e) = result {
                    error!(server = %server, error = %e, "MCP reconnect failed");
                }
                self.params
                    .deps
                    .kernel
                    .register_mcp_tools_for_server(&server)
                    .await;
            }
            None => warn!(
                server = %server,
                "MCP reconnect ignored: sub-agent does not own MCP connections"
            ),
        }
        let snapshots = self.collect_mcp_snapshots().await;
        self.emit(AgentEventPayload::McpStatusReport { servers: snapshots })
            .await
    }

    pub(super) async fn handle_mcp_disconnect(&mut self, server: String) -> Result<()> {
        info!(server = %server, "disconnecting MCP server");
        match self.params.deps.kernel.mcp_manager() {
            Some(mgr) => {
                let result = mgr.write().await.disconnect_connection(&server).await;
                match result {
                    Ok(removed_tools) => {
                        self.params.deps.kernel.unregister_tools(&removed_tools);
                    }
                    Err(e) => error!(server = %server, error = %e, "MCP disconnect failed"),
                }
            }
            None => warn!(
                server = %server,
                "MCP disconnect ignored: sub-agent does not own MCP connections"
            ),
        }
        let snapshots = self.collect_mcp_snapshots().await;
        self.emit(AgentEventPayload::McpStatusReport { servers: snapshots })
            .await
    }

    async fn collect_mcp_snapshots(&self) -> Vec<McpServerSnapshot> {
        let source_map = self.load_mcp_source_map();
        let snapshots = self
            .params
            .deps
            .kernel
            .mcp_provider()
            .snapshot(loopal_mcp::HUB_RPC_BUDGET)
            .await;
        snapshots
            .into_iter()
            .map(|s| McpServerSnapshot {
                source: source_map
                    .get(&s.name)
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string()),
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

    fn load_mcp_source_map(&self) -> HashMap<String, String> {
        let cwd = std::path::Path::new(&self.params.session.cwd);
        match loopal_config::load_config(cwd) {
            Ok(config) => config
                .mcp_servers
                .into_iter()
                .map(|(name, entry)| (name, entry.source.to_string()))
                .collect(),
            Err(_) => HashMap::new(),
        }
    }
}

/// Free-function variant used by the spawned settle-emitter task (no `self`).
async fn collect_mcp_snapshots_via_provider(
    kernel: &Arc<loopal_kernel::Kernel>,
    cwd: &str,
) -> Vec<McpServerSnapshot> {
    let source_map = match loopal_config::load_config(std::path::Path::new(cwd)) {
        Ok(config) => config
            .mcp_servers
            .into_iter()
            .map(|(name, entry)| (name, entry.source.to_string()))
            .collect::<HashMap<_, _>>(),
        Err(_) => HashMap::new(),
    };
    kernel
        .mcp_provider()
        .snapshot(loopal_mcp::HUB_RPC_BUDGET)
        .await
        .into_iter()
        .map(|s| McpServerSnapshot {
            source: source_map
                .get(&s.name)
                .cloned()
                .unwrap_or_else(|| "unknown".to_string()),
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
