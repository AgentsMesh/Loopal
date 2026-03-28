/// Single MCP server connection lifecycle.
///
/// Wraps `McpClient` with status tracking, config, and capability-guarded discovery.
use std::sync::Arc;
use std::time::Duration;

use loopal_config::McpServerConfig;
use loopal_tool_api::ToolDefinition;
use serde_json::Value;
use tracing::{info, warn};

use crate::client::McpClient;
use crate::handler::SamplingCallback;
use crate::transport;
use crate::types::{CapabilitySummary, ConnectionStatus, McpPrompt, McpResource};

/// A managed connection to a single MCP server.
pub struct McpConnection {
    pub name: String,
    pub status: ConnectionStatus,
    pub config: McpServerConfig,
    pub cached_tools: Vec<ToolDefinition>,
    pub cached_resources: Vec<McpResource>,
    pub cached_prompts: Vec<McpPrompt>,
    /// Server instructions from the initialize handshake.
    pub instructions: Option<String>,
    pub errors: Vec<String>,
    client: Option<McpClient>,
    sampling: Option<Arc<dyn SamplingCallback>>,
}

impl McpConnection {
    pub fn new(
        name: String,
        config: McpServerConfig,
        sampling: Option<Arc<dyn SamplingCallback>>,
    ) -> Self {
        Self {
            name,
            status: ConnectionStatus::Disconnected,
            config,
            cached_tools: Vec::new(),
            cached_resources: Vec::new(),
            cached_prompts: Vec::new(),
            instructions: None,
            errors: Vec::new(),
            client: None,
            sampling,
        }
    }

    /// Establish connection and discover capabilities.
    pub async fn connect(&mut self) {
        self.status = ConnectionStatus::Connecting;
        self.errors.clear();
        self.cached_tools.clear();
        self.cached_resources.clear();
        self.cached_prompts.clear();
        self.instructions = None;

        let timeout = Duration::from_millis(self.config.timeout_ms());

        let result = self.create_client(timeout).await;
        match result {
            Ok(client) => {
                // Extract server instructions from handshake.
                if let Some(info) = client.peer_info() {
                    self.instructions = info.instructions.clone();
                }
                self.client = Some(client);
                self.discover_capabilities().await;
                // Status is Connected even with discovery errors — the transport
                // works but some capabilities may be missing. Check `errors` for details.
                self.status = ConnectionStatus::Connected;
                if self.errors.is_empty() {
                    info!(server = %self.name, tools = self.cached_tools.len(), "connected");
                } else {
                    warn!(server = %self.name, errors = ?self.errors, "connected with errors");
                }
            }
            Err(e) => {
                let msg = format!("connection failed: {e}");
                self.errors.push(msg.clone());
                self.status = ConnectionStatus::Failed(msg);
            }
        }
    }

    /// Disconnect and release the client.
    pub async fn disconnect(&mut self) {
        self.client = None;
        self.status = ConnectionStatus::Disconnected;
        self.cached_tools.clear();
        self.cached_resources.clear();
        self.cached_prompts.clear();
        self.instructions = None;
    }

    /// Get the underlying client (if connected).
    pub fn client(&self) -> Option<&McpClient> {
        self.client.as_ref()
    }

    /// Create client by selecting the right transport for our config.
    async fn create_client(&self, timeout: Duration) -> Result<McpClient, loopal_error::McpError> {
        let sampling = self.sampling.clone();
        match &self.config {
            McpServerConfig::Stdio {
                command, args, env, ..
            } => transport::connect_stdio(command, args, env, timeout, sampling).await,
            McpServerConfig::StreamableHttp { url, headers, .. } => {
                transport::connect_http(url, headers, timeout, sampling).await
            }
        }
    }

    /// Discover tools/resources/prompts based on server capabilities.
    async fn discover_capabilities(&mut self) {
        let Some(client) = &self.client else { return };
        let caps = extract_capabilities(client);

        if caps.tools {
            match client.list_tools().await {
                Ok(result) => {
                    self.cached_tools = result
                        .tools
                        .iter()
                        .map(|t| ToolDefinition {
                            name: t.name.to_string(),
                            description: t
                                .description
                                .as_ref()
                                .map(|d| d.to_string())
                                .unwrap_or_default(),
                            input_schema: Value::Object((*t.input_schema).clone()),
                        })
                        .collect();
                }
                Err(e) => self.errors.push(format!("tools/list: {e}")),
            }
        }

        if caps.resources {
            match client.list_resources().await {
                Ok(result) => {
                    self.cached_resources = result
                        .resources
                        .iter()
                        .map(|r| McpResource {
                            uri: r.uri.to_string(),
                            name: r.name.to_string(),
                            description: r.description.as_ref().map(|d| d.to_string()),
                            mime_type: r.mime_type.as_ref().map(|m| m.to_string()),
                        })
                        .collect();
                }
                Err(e) => self.errors.push(format!("resources/list: {e}")),
            }
        }
        if caps.prompts {
            match client.list_prompts().await {
                Ok(result) => {
                    self.cached_prompts = result
                        .prompts
                        .iter()
                        .map(|p| McpPrompt {
                            name: p.name.to_string(),
                            description: p.description.as_ref().map(|d| d.to_string()),
                        })
                        .collect();
                }
                Err(e) => self.errors.push(format!("prompts/list: {e}")),
            }
        }
    }
}

fn extract_capabilities(client: &McpClient) -> CapabilitySummary {
    let Some(info) = client.peer_info() else {
        return CapabilitySummary::default();
    };
    CapabilitySummary {
        tools: info.capabilities.tools.is_some(),
        resources: info.capabilities.resources.is_some(),
        prompts: info.capabilities.prompts.is_some(),
    }
}
