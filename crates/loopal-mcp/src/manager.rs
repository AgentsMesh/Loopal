/// Manages multiple MCP server connections.
///
/// Core lifecycle (start) and tool dispatch. Reconnect, restart, and query
/// methods are in `manager_query.rs`.
use std::collections::HashMap;
use std::sync::Arc;

use indexmap::IndexMap;
use loopal_config::McpServerConfig;
use loopal_error::McpError;
use loopal_tool_api::ToolDefinition;
use loopal_vault_api::Vault;
use rmcp::model::CallToolResult;
use serde_json::Value;
use tracing::{debug, info, warn};

use crate::connection::McpConnection;
use crate::handler::SamplingCallback;
use crate::secret_expand::expand_mcp_config;

/// Manages multiple MCP server connections and tool routing.
pub struct McpManager {
    pub(crate) connections: IndexMap<String, McpConnection>,
    /// tool_name → server_name for fast dispatch.
    pub(crate) tool_map: HashMap<String, String>,
    /// Shared sampling callback for all connections.
    sampling: Option<Arc<dyn SamplingCallback>>,
    /// Optional secret store for resolving `{{secret:X}}` placeholders in
    /// env / headers / url at spawn time.
    secrets: Option<Arc<dyn Vault>>,
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            connections: IndexMap::new(),
            tool_map: HashMap::new(),
            sampling: None,
            secrets: None,
        }
    }

    /// Set the sampling callback for MCP server-initiated LLM calls.
    pub fn set_sampling(&mut self, callback: Arc<dyn SamplingCallback>) {
        self.sampling = Some(callback);
    }

    /// Configure the secret store used to expand `{{secret:X}}` placeholders
    /// in MCP server configs before spawn.
    pub fn set_secrets(&mut self, store: Arc<dyn Vault>) {
        self.secrets = Some(store);
    }

    /// Start all configured MCP servers.
    ///
    /// Connections are established concurrently. Individual failures are logged;
    /// returns error only if ALL servers fail.
    pub async fn start_all(
        &mut self,
        configs: &IndexMap<String, McpServerConfig>,
    ) -> Result<(), McpError> {
        let prepared = self.prepare_connections(configs).await;
        let results = connect_all(prepared).await;
        self.absorb_connections(results)
    }

    /// Snapshot sampling/secrets and expand placeholders without holding any
    /// caller's lock. Returns unconnected `McpConnection` objects ready for
    /// `connect_all`. Lets background spawn release the manager write lock
    /// during the slow `connect()` phase.
    pub async fn prepare_connections(
        &self,
        configs: &IndexMap<String, McpServerConfig>,
    ) -> Vec<McpConnection> {
        let mut prepared = Vec::new();
        for (name, cfg) in configs {
            let resolved = expand_mcp_config(cfg.clone(), self.secrets.as_ref()).await;
            if !resolved.enabled() {
                info!(server = %name, "MCP server disabled, skipping");
                continue;
            }
            prepared.push(McpConnection::new(
                name.clone(),
                resolved,
                self.sampling.clone(),
            ));
        }
        prepared
    }

    /// Insert already-connected (or already-failed) `McpConnection` objects
    /// into manager state. Errors only when EVERY server failed to connect.
    pub fn absorb_connections(
        &mut self,
        results: Vec<McpConnection>,
    ) -> Result<(), McpError> {
        let total = results.len();
        let mut failure_count = 0;
        for conn in results {
            if conn.status.is_connected() {
                let name = conn.name.clone();
                for tool in &conn.cached_tools {
                    if let Some(prev) = self.tool_map.insert(tool.name.clone(), name.clone()) {
                        warn!(
                            tool = %tool.name,
                            new_server = %name,
                            prev_server = %prev,
                            "MCP tool name conflict: overriding previous server"
                        );
                    }
                }
                self.connections.insert(name, conn);
            } else {
                warn!(server = %conn.name, errors = ?conn.errors, "failed to start MCP server");
                failure_count += 1;
                self.connections.insert(conn.name.clone(), conn);
            }
        }

        if total > 0 && failure_count == total {
            return Err(McpError::ServerNotFound(
                "all MCP servers failed to start".into(),
            ));
        }

        info!(
            servers = self.connections.len(),
            tools = self.tool_map.len(),
            "MCP servers started"
        );
        Ok(())
    }

    /// Return (server_name, ToolDefinition) for all connected servers.
    pub fn get_tools_with_server(&self) -> Vec<(String, ToolDefinition)> {
        self.connections
            .iter()
            .flat_map(|(name, conn)| {
                conn.cached_tools
                    .iter()
                    .map(move |t| (name.clone(), t.clone()))
            })
            .collect()
    }

    /// Call a tool on a specific server.
    pub async fn call_tool(
        &self,
        server: &str,
        name: &str,
        args: &Value,
    ) -> Result<CallToolResult, McpError> {
        let conn = self
            .connections
            .get(server)
            .ok_or_else(|| McpError::ServerNotFound(server.to_string()))?;
        let client = conn
            .client()
            .ok_or_else(|| McpError::TransportClosed(format!("'{server}' not connected")))?;

        let json_args = match args.as_object() {
            Some(map) => map.clone(),
            None => serde_json::Map::new(),
        };

        client.call_tool(name, json_args).await
    }

    /// Call a tool by name, auto-resolving the server.
    pub async fn call_tool_by_name(
        &self,
        name: &str,
        args: &Value,
    ) -> Result<CallToolResult, McpError> {
        let server = self
            .tool_map
            .get(name)
            .ok_or_else(|| McpError::ServerNotFound(format!("no server for tool '{name}'")))?
            .clone();
        debug!(tool = name, server = %server, "MCP tool resolved");
        self.call_tool(&server, name, args).await
    }

    /// Remove a tool from the routing map (used when Kernel skips a conflicting tool).
    pub fn remove_tool_mapping(&mut self, tool_name: &str) {
        self.tool_map.remove(tool_name);
    }
}

/// Lock-free concurrent connect — caller passes prepared `McpConnection`
/// objects (see `prepare_connections`); this drives all `connect()` futures
/// in parallel without any shared state, so a slow server cannot block
/// other readers of the originating manager.
pub async fn connect_all(prepared: Vec<McpConnection>) -> Vec<McpConnection> {
    futures::future::join_all(prepared.into_iter().map(|mut conn| async move {
        conn.connect().await;
        conn
    }))
    .await
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}
