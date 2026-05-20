use std::sync::Arc;
use std::time::Duration;

use loopal_mcp::McpProvider;
use loopal_mcp::McpToolAdapter;
use loopal_mcp::types::{McpPrompt, McpResource};
use loopal_tool_api::ToolDefinition;
use tracing::{info, warn};

use super::{Kernel, McpBackend};

impl Kernel {
    pub fn mcp_instructions(&self) -> &[(String, String)] {
        &self.mcp_instructions
    }

    pub fn mcp_resources(&self) -> &[(String, McpResource)] {
        &self.mcp_resources
    }

    pub fn mcp_prompts(&self) -> &[(String, McpPrompt)] {
        &self.mcp_prompts
    }

    pub fn mcp_provider(&self) -> Arc<dyn McpProvider> {
        self.mcp.provider()
    }

    /// Replace the MCP backend. Sub-agents inject a proxy here.
    pub fn set_mcp_provider(&mut self, provider: Arc<dyn McpProvider>) {
        self.mcp = McpBackend::Proxy(provider);
    }

    /// Kick off background MCP connection. Returns immediately.
    /// No-op when the backend is a remote proxy.
    pub async fn spawn_mcp(&self) {
        let Some(local) = self.mcp.local() else {
            return;
        };
        if self.settings.mcp_servers.is_empty() {
            return;
        }
        if let Some(store) = self.secrets() {
            local.manager().write().await.set_secrets(store.clone());
        }
        local.spawn_background(self.settings.mcp_servers.clone());
    }

    /// Wait for MCP to settle, snapshot derived metadata, and register tool
    /// adapters. Returns true iff the wait observed every in-flight spawn.
    pub async fn finalize_mcp_tools(&mut self, max_wait: Duration) -> bool {
        let settled = match self.mcp.local() {
            Some(local) => local.wait_until_settled(max_wait).await,
            None => true,
        };

        let (instructions, resources, prompts) = self.snapshot_local_mcp_metadata().await;
        self.mcp_instructions = instructions;
        self.mcp_resources = resources;
        self.mcp_prompts = prompts;

        self.register_mcp_tool_adapters().await;
        settled
    }

    /// Snapshot startup-time MCP metadata (instructions / resources / prompts).
    /// reason: these fields are a one-shot capture at `finalize_mcp_tools`;
    /// later reconnects only re-register tools via `register_mcp_tools_for_server`
    /// and do NOT refresh instructions/resources/prompts — MCP servers
    /// effectively treat these as handshake-time constants. We use `try_read`
    /// to avoid blocking on the background spawn's pending write lock —
    /// tokio::sync::RwLock is write-preferring, so a queued writer would
    /// otherwise starve us past the bounded-wait budget.
    async fn snapshot_local_mcp_metadata(
        &self,
    ) -> (
        Vec<(String, String)>,
        Vec<(String, McpResource)>,
        Vec<(String, McpPrompt)>,
    ) {
        let Some(local) = self.mcp.local() else {
            return (Vec::new(), Vec::new(), Vec::new());
        };
        let arc_mgr = local.manager();
        let Ok(mgr) = arc_mgr.try_read() else {
            return (Vec::new(), Vec::new(), Vec::new());
        };
        (
            mgr.get_server_instructions(),
            mgr.get_resources(),
            mgr.get_prompts(),
        )
    }

    /// Public re-entry point for the late-registration listener spawned in
    /// `build_kernel_from_config`. Idempotent — already-registered tools are
    /// skipped by the inner ToolRegistry conflict check.
    pub async fn register_all_settled_mcp_tools(&self) {
        self.register_mcp_tool_adapters().await;
    }

    async fn register_mcp_tool_adapters(&self) {
        let provider = self.mcp.provider();
        let tools = self.snapshot_tools_for_registration().await;
        let mut skipped = Vec::new();
        for (server_name, tool_def) in tools {
            if self.tool_registry.get(&tool_def.name).is_some() {
                warn!(
                    tool = %tool_def.name, server = %server_name,
                    "MCP tool name conflicts with existing tool, skipping"
                );
                skipped.push(tool_def.name.clone());
                continue;
            }
            info!(tool = %tool_def.name, server = %server_name, "registering MCP tool");
            let adapter = McpToolAdapter::new(tool_def, server_name, provider.clone());
            self.tool_registry.register(Box::new(adapter));
        }
        let (Some(local), false) = (self.mcp.local(), skipped.is_empty()) else {
            return;
        };
        let arc_mgr = local.manager();
        let Ok(mut mgr) = arc_mgr.try_write() else {
            return;
        };
        for name in &skipped {
            mgr.remove_tool_mapping(name);
        }
    }

    /// Non-blocking variant of `mcp_provider().list_tools()` for the local
    /// path. reason: same write-preferring RwLock concern as
    /// `snapshot_local_mcp_metadata` — never block finalize past the bounded
    /// wait. For Proxy mode (sub-agents), we always go through the trait so
    /// IPC fetches the remote tool list.
    async fn snapshot_tools_for_registration(&self) -> Vec<(String, loopal_tool_api::ToolDefinition)> {
        if let Some(local) = self.mcp.local() {
            let arc_mgr = local.manager();
            return match arc_mgr.try_read() {
                Ok(mgr) => mgr.get_tools_with_server(),
                Err(_) => Vec::new(),
            };
        }
        self.mcp.provider().list_tools().await
    }

    /// Register tools from a (re)connected MCP server. Used by reconnect path.
    pub async fn register_mcp_tools_for_server(&self, server: &str) {
        let Some(local) = self.mcp.local() else {
            return;
        };
        let new_tools: Vec<ToolDefinition> = {
            let mgr = local.manager();
            let mgr = mgr.read().await;
            mgr.get_tools_for_server(server)
        };
        let provider = self.mcp.provider();
        for tool_def in new_tools {
            if self.tool_registry.get(&tool_def.name).is_some() {
                warn!(
                    tool = %tool_def.name, server = %server,
                    "MCP tool conflicts with existing tool, skipping"
                );
                continue;
            }
            info!(tool = %tool_def.name, server = %server, "dynamically registering MCP tool");
            let adapter = McpToolAdapter::new(tool_def, server.to_string(), provider.clone());
            self.tool_registry.register(Box::new(adapter));
        }
    }

    pub fn unregister_tools(&self, names: &[String]) {
        for name in names {
            info!(tool = %name, "unregistering MCP tool");
            self.tool_registry.unregister(name);
        }
    }
}
