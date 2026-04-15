//! MCP control command handlers — status query and reconnect.

use std::collections::HashMap;

use loopal_error::Result;
use loopal_protocol::{AgentEventPayload, McpServerSnapshot};
use tracing::{error, info};

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

    pub(super) async fn handle_query_mcp_status(&mut self) -> Result<()> {
        let snapshots = self.collect_mcp_snapshots().await;
        self.emit(AgentEventPayload::McpStatusReport { servers: snapshots })
            .await
    }

    pub(super) async fn handle_mcp_reconnect(&mut self, server: String) -> Result<()> {
        info!(server = %server, "reconnecting MCP server");
        let mgr = self.params.deps.kernel.mcp_manager();
        let result = mgr.write().await.restart_connection(&server).await;
        if let Err(e) = result {
            error!(server = %server, error = %e, "MCP reconnect failed");
        }
        self.params
            .deps
            .kernel
            .register_mcp_tools_for_server(&server)
            .await;
        let snapshots = self.collect_mcp_snapshots().await;
        self.emit(AgentEventPayload::McpStatusReport { servers: snapshots })
            .await
    }

    async fn collect_mcp_snapshots(&self) -> Vec<McpServerSnapshot> {
        let source_map = self.load_mcp_source_map();
        let mgr = self.params.deps.kernel.mcp_manager();
        let reader = mgr.read().await;
        reader
            .collect_snapshots()
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
