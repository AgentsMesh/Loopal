//! MCP control command handlers — status query, reconnect, and disconnect.

use std::collections::HashMap;
use std::sync::Arc;

use loopal_error::Result;
use loopal_protocol::{AgentEventPayload, McpServerSnapshot};
use tracing::{error, info, warn};

use super::input_control::ControlOutcome;
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
        let weak_kernel = Arc::downgrade(&self.params.deps.kernel);
        let weak_frontend = Arc::downgrade(&self.params.deps.frontend);
        let cwd = self.params.session.cwd.clone();
        super::input_mcp_events::spawn(
            self.params.deps.kernel.local_mcp_provider(),
            weak_kernel,
            weak_frontend,
            cwd,
        );
    }

    pub(super) async fn handle_query_mcp_status(&mut self) -> Result<()> {
        let snapshots = self.collect_mcp_snapshots().await;
        self.emit(AgentEventPayload::McpStatusReport { servers: snapshots })
            .await
    }

    pub(super) async fn handle_mcp_reconnect(&mut self, server: String) -> Result<ControlOutcome> {
        info!(server = %server, "reconnecting MCP server");
        let rejection = match self.params.deps.kernel.mcp_manager() {
            Some(mgr) => {
                let result = mgr.write().await.restart_connection(&server).await;
                let rejection = if let Err(e) = result {
                    error!(server = %server, error = %e, "MCP reconnect failed");
                    Some(format!("MCP reconnect failed for {server}: {e}"))
                } else {
                    None
                };
                self.params
                    .deps
                    .kernel
                    .register_mcp_tools_for_server(&server)
                    .await;
                rejection
            }
            None => {
                warn!(server = %server, "MCP reconnect ignored: no owned connections");
                Some("this agent does not own MCP connections".to_string())
            }
        };
        let snapshots = self.collect_mcp_snapshots().await;
        self.emit(AgentEventPayload::McpStatusReport { servers: snapshots })
            .await?;
        Ok(rejection.map_or_else(ControlOutcome::applied, ControlOutcome::rejected))
    }

    pub(super) async fn handle_mcp_disconnect(&mut self, server: String) -> Result<ControlOutcome> {
        info!(server = %server, "disconnecting MCP server");
        let rejection = match self.params.deps.kernel.mcp_manager() {
            Some(mgr) => {
                let result = mgr.write().await.disconnect_connection(&server).await;
                match result {
                    Ok(removed_tools) => {
                        self.params.deps.kernel.unregister_tools(&removed_tools);
                        None
                    }
                    Err(e) => {
                        error!(server = %server, error = %e, "MCP disconnect failed");
                        Some(format!("MCP disconnect failed for {server}: {e}"))
                    }
                }
            }
            None => {
                warn!(server = %server, "MCP disconnect ignored: no owned connections");
                Some("this agent does not own MCP connections".to_string())
            }
        };
        let snapshots = self.collect_mcp_snapshots().await;
        self.emit(AgentEventPayload::McpStatusReport { servers: snapshots })
            .await?;
        Ok(rejection.map_or_else(ControlOutcome::applied, ControlOutcome::rejected))
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
