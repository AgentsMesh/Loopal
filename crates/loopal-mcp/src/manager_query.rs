/// Query and lifecycle methods for McpManager.
///
/// Split from `manager.rs` to stay within the 200-line file limit.
/// Contains: resource/prompt/instruction queries, reconnect, restart.
use loopal_error::McpError;
use tracing::warn;

use crate::manager::McpManager;
use crate::reconnect::{self, ReconnectPolicy};
use crate::types::{McpPrompt, McpResource};

impl McpManager {
    /// Collect server instructions from all connected MCP servers.
    pub fn get_server_instructions(&self) -> Vec<(String, String)> {
        self.connections
            .iter()
            .filter_map(|(name, conn)| {
                conn.instructions
                    .as_ref()
                    .map(|instr| (name.clone(), instr.clone()))
            })
            .collect()
    }

    /// Return (server_name, resource) for all connected servers.
    pub fn get_resources(&self) -> Vec<(String, McpResource)> {
        self.connections
            .iter()
            .flat_map(|(name, conn)| {
                conn.cached_resources
                    .iter()
                    .map(move |r| (name.clone(), r.clone()))
            })
            .collect()
    }

    /// Return (server_name, prompt) for all connected servers.
    pub fn get_prompts(&self) -> Vec<(String, McpPrompt)> {
        self.connections
            .iter()
            .flat_map(|(name, conn)| {
                conn.cached_prompts
                    .iter()
                    .map(move |p| (name.clone(), p.clone()))
            })
            .collect()
    }

    /// Read a specific resource by server name and URI.
    pub async fn read_resource(&self, server: &str, uri: &str) -> Result<String, McpError> {
        use rmcp::model::ResourceContents;

        let conn = self
            .connections
            .get(server)
            .ok_or_else(|| McpError::ServerNotFound(server.to_string()))?;
        let client = conn
            .client()
            .ok_or_else(|| McpError::TransportClosed(format!("'{server}' not connected")))?;

        let result = client.read_resource(uri).await?;
        let text = result
            .contents
            .iter()
            .filter_map(|c| match c {
                ResourceContents::TextResourceContents { text, .. } => Some(text.as_str()),
                ResourceContents::BlobResourceContents { .. } => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        Ok(text)
    }

    /// Reconnect an HTTP connection with exponential backoff, then rebuild tool_map.
    ///
    /// Not used in the hot path (tool_adapter uses restart_connection for single
    /// attempt). Reserved for future background health-check tasks.
    #[allow(dead_code)]
    pub(crate) async fn reconnect(&mut self, name: &str) -> Result<(), McpError> {
        let conn = self
            .connections
            .get_mut(name)
            .ok_or_else(|| McpError::ServerNotFound(name.to_string()))?;

        self.tool_map.retain(|_, srv| srv != name);

        let policy = ReconnectPolicy::default();
        reconnect::reconnect_loop(conn, &policy).await?;

        self.rebuild_tool_map_for(name);
        Ok(())
    }

    /// Restart a specific connection by name (manual restart, no backoff).
    ///
    /// Skips if the connection is already connected (guards against concurrent
    /// reconnect attempts from multiple tool adapters).
    pub async fn restart_connection(&mut self, name: &str) -> Result<(), McpError> {
        let conn = self
            .connections
            .get_mut(name)
            .ok_or_else(|| McpError::ServerNotFound(name.to_string()))?;

        if conn.status.is_connected() && conn.client().is_some_and(|c| !c.is_closed()) {
            return Ok(()); // Already reconnected by another task.
        }

        self.tool_map.retain(|_, srv| srv != name);
        conn.disconnect().await;
        conn.connect().await;

        self.rebuild_tool_map_for(name);
        Ok(())
    }

    fn rebuild_tool_map_for(&mut self, name: &str) {
        let Some(conn) = self.connections.get(name) else {
            return;
        };
        if !conn.status.is_connected() {
            return;
        }
        for tool in &conn.cached_tools {
            if let Some(prev) = self.tool_map.insert(tool.name.clone(), name.to_string()) {
                if prev != name {
                    warn!(
                        tool = %tool.name,
                        new_server = %name,
                        prev_server = %prev,
                        "MCP tool name conflict"
                    );
                }
            }
        }
    }
}
