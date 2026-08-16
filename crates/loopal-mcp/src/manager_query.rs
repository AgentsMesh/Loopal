use crate::manager::McpManager;
use crate::types::{McpPrompt, McpResource};
use loopal_config::McpServerConfig;
use loopal_error::McpError;

pub struct McpConnectionSnapshot {
    pub name: String,
    pub transport: String,
    pub status: String,
    pub tool_count: usize,
    pub resource_count: usize,
    pub prompt_count: usize,
    pub errors: Vec<String>,
}

impl McpManager {
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

    pub async fn read_resource(&self, server: &str, uri: &str) -> Result<String, McpError> {
        use rmcp::model::ResourceContents;

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

        let result = client.read_resource(uri).await?;
        let mut text = Vec::new();
        for content in &result.contents {
            match content {
                ResourceContents::TextResourceContents { text: value, .. } => {
                    text.push(sanitizer.sanitize_text(value));
                }
                ResourceContents::BlobResourceContents { .. } => sanitizer.reject_blob()?,
            }
        }
        Ok(text.join("\n"))
    }

    pub async fn disconnect_connection(&mut self, name: &str) -> Result<Vec<String>, McpError> {
        let conn = self
            .connections
            .get_mut(name)
            .ok_or_else(|| McpError::ServerNotFound(name.to_string()))?;
        let removed: Vec<String> = self
            .tool_map
            .iter()
            .filter(|(_, srv)| srv.as_str() == name)
            .map(|(tool, _)| tool.clone())
            .collect();
        self.tool_map.retain(|_, srv| srv != name);
        conn.disconnect().await;
        Ok(removed)
    }

    pub fn get_tools_for_server(&self, server: &str) -> Vec<loopal_tool_api::ToolDefinition> {
        self.connections
            .get(server)
            .filter(|c| c.status.is_connected())
            .map(|c| c.cached_tools.clone())
            .unwrap_or_default()
    }

    // For non-Connected servers, merge stderr_tail lines into `errors` so the
    // /mcp page shows actionable diagnostics from the server process instead
    // of just our generic "did not complete handshake" wrapper.
    pub fn collect_snapshots(&self) -> Vec<McpConnectionSnapshot> {
        self.connections
            .iter()
            .map(|(name, conn)| {
                let transport = match &conn.config {
                    McpServerConfig::Stdio { .. } => "stdio",
                    McpServerConfig::StreamableHttp { .. } => "streamable-http",
                };
                let mut errors = conn.errors.clone();
                if !conn.status.is_connected()
                    && let Ok(tail) = conn.stderr_tail.try_lock()
                {
                    for line in tail.iter() {
                        errors.push(format!("stderr: {line}"));
                    }
                }
                McpConnectionSnapshot {
                    name: name.clone(),
                    transport: transport.to_string(),
                    status: conn.status.to_string(),
                    tool_count: conn.cached_tools.len(),
                    resource_count: conn.cached_resources.len(),
                    prompt_count: conn.cached_prompts.len(),
                    errors,
                }
            })
            .collect()
    }
}
