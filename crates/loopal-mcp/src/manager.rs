use std::collections::HashMap;

use loopal_types::config::McpServerConfig;
use loopal_types::error::McpError;
use loopal_types::tool::ToolDefinition;
use serde_json::Value;
use tracing::{debug, info, warn};

use crate::client::McpClient;

/// Manages multiple MCP server connections.
pub struct McpManager {
    clients: HashMap<String, McpClient>,
    /// tool_name -> server_name mapping
    tool_map: HashMap<String, String>,
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
            tool_map: HashMap::new(),
        }
    }

    /// Start all configured MCP servers.
    /// Individual server failures are logged but do not abort the entire process.
    /// Returns an error only if ALL servers fail to start.
    pub async fn start_all(&mut self, configs: &[McpServerConfig]) -> Result<(), McpError> {
        let mut failure_count = 0;
        for config in configs {
            info!(server = %config.name, "starting MCP server");
            match McpClient::start(&config.command, &config.args, &config.env).await {
                Ok(client) => {
                    self.clients.insert(config.name.clone(), client);
                }
                Err(e) => {
                    warn!(server = %config.name, error = %e, "failed to start MCP server, skipping");
                    failure_count += 1;
                }
            }
        }

        if !configs.is_empty() && failure_count == configs.len() {
            return Err(McpError::ServerNotFound(
                "all MCP servers failed to start".to_string(),
            ));
        }

        // Discover tools from all successfully started servers
        self.discover_tools().await?;

        Ok(())
    }

    async fn discover_tools(&mut self) -> Result<(), McpError> {
        self.tool_map.clear();
        for (server_name, client) in &self.clients {
            let result = client
                .send_request("tools/list", serde_json::json!({}))
                .await?;
            if let Some(tools) = result.get("tools").and_then(|t| t.as_array()) {
                for tool in tools {
                    if let Some(name) = tool.get("name").and_then(|n| n.as_str()) {
                        self.tool_map
                            .insert(name.to_string(), server_name.clone());
                    }
                }
            }
        }
        info!(tool_count = self.tool_map.len(), "MCP tools discovered");
        Ok(())
    }

    /// Aggregate tool definitions from all connected servers, with server name.
    pub async fn get_tools_with_server(&self) -> Result<Vec<(String, ToolDefinition)>, McpError> {
        let mut tools = Vec::new();
        for (server_name, client) in &self.clients {
            let result = client
                .send_request("tools/list", serde_json::json!({}))
                .await?;
            if let Some(tool_list) = result.get("tools").and_then(|t| t.as_array()) {
                for tool in tool_list {
                    let name = tool
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let description = tool
                        .get("description")
                        .and_then(|d| d.as_str())
                        .unwrap_or("")
                        .to_string();
                    let input_schema = tool
                        .get("inputSchema")
                        .cloned()
                        .unwrap_or(serde_json::json!({}));

                    tools.push((server_name.clone(), ToolDefinition {
                        name,
                        description,
                        input_schema,
                    }));
                }
            }
        }
        Ok(tools)
    }

    /// Aggregate tool definitions from all connected servers.
    pub async fn get_tools(&self) -> Result<Vec<ToolDefinition>, McpError> {
        let mut tools = Vec::new();
        for client in self.clients.values() {
            let result = client
                .send_request("tools/list", serde_json::json!({}))
                .await?;
            if let Some(tool_list) = result.get("tools").and_then(|t| t.as_array()) {
                for tool in tool_list {
                    let name = tool
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let description = tool
                        .get("description")
                        .and_then(|d| d.as_str())
                        .unwrap_or("")
                        .to_string();
                    let input_schema = tool
                        .get("inputSchema")
                        .cloned()
                        .unwrap_or(serde_json::json!({}));

                    tools.push(ToolDefinition {
                        name,
                        description,
                        input_schema,
                    });
                }
            }
        }
        Ok(tools)
    }

    /// Call a tool on a specific server.
    pub async fn call_tool(
        &self,
        server: &str,
        name: &str,
        args: Value,
    ) -> Result<Value, McpError> {
        let client = self
            .clients
            .get(server)
            .ok_or_else(|| McpError::ServerNotFound(server.to_string()))?;

        info!(server = %server, tool = %name, "MCP tool call");
        client
            .send_request(
                "tools/call",
                serde_json::json!({
                    "name": name,
                    "arguments": args,
                }),
            )
            .await
    }

    /// Call a tool by name, auto-resolving the server.
    pub async fn call_tool_by_name(&self, name: &str, args: Value) -> Result<Value, McpError> {
        let server = self
            .tool_map
            .get(name)
            .ok_or_else(|| McpError::ServerNotFound(format!("no server for tool '{name}'")))?;

        debug!(tool = %name, server = %server, "MCP tool resolved");
        self.call_tool(server, name, args).await
    }
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}
