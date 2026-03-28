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
use rmcp::model::CallToolResult;
use serde_json::Value;
use tracing::{debug, info, warn};

use crate::connection::McpConnection;
use crate::handler::SamplingCallback;

/// Manages multiple MCP server connections and tool routing.
pub struct McpManager {
    pub(crate) connections: IndexMap<String, McpConnection>,
    /// tool_name → server_name for fast dispatch.
    pub(crate) tool_map: HashMap<String, String>,
    /// Shared sampling callback for all connections.
    sampling: Option<Arc<dyn SamplingCallback>>,
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            connections: IndexMap::new(),
            tool_map: HashMap::new(),
            sampling: None,
        }
    }

    /// Set the sampling callback for MCP server-initiated LLM calls.
    pub fn set_sampling(&mut self, callback: Arc<dyn SamplingCallback>) {
        self.sampling = Some(callback);
    }

    /// Start all configured MCP servers.
    ///
    /// Connections are established concurrently. Individual failures are logged;
    /// returns error only if ALL servers fail.
    pub async fn start_all(
        &mut self,
        configs: &IndexMap<String, McpServerConfig>,
    ) -> Result<(), McpError> {
        // Phase 1: connect all enabled servers concurrently.
        let mut futures = Vec::new();
        for (name, config) in configs {
            if !config.enabled() {
                info!(server = %name, "MCP server disabled, skipping");
                continue;
            }
            let mut conn = McpConnection::new(name.clone(), config.clone(), self.sampling.clone());
            futures.push(async move {
                conn.connect().await;
                conn
            });
        }

        let results = futures::future::join_all(futures).await;

        // Phase 2: collect results (sequential, cheap — just HashMap inserts).
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

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}
