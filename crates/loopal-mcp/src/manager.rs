use std::collections::HashMap;
use std::sync::Arc;

use indexmap::IndexMap;
use loopal_config::McpServerConfig;
use loopal_error::McpError;
use loopal_secret_client::SecretClient;
use loopal_tool_api::ToolDefinition;
use rmcp::model::CallToolResult;
use serde_json::Value;
use tracing::{debug, info, warn};

use crate::connection::McpConnection;
use crate::handler::SamplingCallback;
use crate::manager_prepare::prepare_connection;

pub struct McpManager {
    pub(crate) connections: IndexMap<String, McpConnection>,
    pub(crate) tool_map: HashMap<String, String>,
    sampling: Option<Arc<dyn SamplingCallback>>,
    secret_client: Option<Arc<dyn SecretClient>>,
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            connections: IndexMap::new(),
            tool_map: HashMap::new(),
            sampling: None,
            secret_client: None,
        }
    }

    pub fn set_sampling(&mut self, callback: Arc<dyn SamplingCallback>) {
        self.sampling = Some(callback);
    }

    pub fn set_secret_client(&mut self, client: Arc<dyn SecretClient>) {
        self.secret_client = Some(client);
    }

    pub(crate) fn secret_client(&self) -> Option<Arc<dyn SecretClient>> {
        self.secret_client.clone()
    }

    pub(crate) fn sampling(&self) -> Option<Arc<dyn SamplingCallback>> {
        self.sampling.clone()
    }

    /// Connects concurrently and errors only when all enabled servers fail.
    pub async fn start_all(
        &mut self,
        configs: &IndexMap<String, McpServerConfig>,
    ) -> Result<(), McpError> {
        let prepared = self.prepare_connections(configs).await;
        let results = connect_all(prepared).await;
        self.absorb_connections(results)
    }

    /// Resolves config before the slow, lock-free connection phase.
    pub async fn prepare_connections(
        &self,
        configs: &IndexMap<String, McpServerConfig>,
    ) -> Vec<McpConnection> {
        let mut prepared = Vec::new();
        let secret_client = self.secret_client.clone();
        let sampling = self.sampling.clone();
        for (name, config) in configs {
            if let Some(connection) = prepare_connection(
                name.clone(),
                config.clone(),
                secret_client.clone(),
                sampling.clone(),
            )
            .await
            {
                prepared.push(connection);
            }
        }
        prepared
    }

    /// Commits settled connections and errors only when every server failed.
    pub fn absorb_connections(&mut self, results: Vec<McpConnection>) -> Result<(), McpError> {
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
        let sanitizer = conn
            .result_sanitizer()
            .await
            .map_err(|message| McpError::Protocol(message.into()))?;
        let client = conn
            .client()
            .ok_or_else(|| McpError::TransportClosed(format!("'{server}' not connected")))?;

        let json_args = match args.as_object() {
            Some(map) => map.clone(),
            None => serde_json::Map::new(),
        };

        let result = client.call_tool(name, json_args).await?;
        Ok(sanitizer.sanitize(result))
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

/// Connects prepared servers concurrently without holding manager state.
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
