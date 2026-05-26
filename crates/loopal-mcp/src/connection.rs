use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use loopal_config::McpServerConfig;
use loopal_tool_api::ToolDefinition;
use serde_json::Value;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::client::McpClient;
use crate::handler::SamplingCallback;
use crate::transport;
use crate::types::{CapabilitySummary, ConnectionStatus, McpPrompt, McpResource};

// Surfaced to `/mcp` page when a stdio server enters Failed state so the
// user sees the server's own error output, not just our wrapper message.
const STDERR_RETENTION: usize = 16;

pub struct McpConnection {
    pub name: String,
    pub status: ConnectionStatus,
    pub config: McpServerConfig,
    pub cached_tools: Vec<ToolDefinition>,
    pub cached_resources: Vec<McpResource>,
    pub cached_prompts: Vec<McpPrompt>,
    pub instructions: Option<String>,
    pub errors: Vec<String>,
    // Tail-capped at STDERR_RETENTION for bounded memory; consumed by
    // manager_query::collect_snapshots for failure diagnostics.
    pub stderr_tail: Arc<Mutex<VecDeque<String>>>,
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
            stderr_tail: Arc::new(Mutex::new(VecDeque::with_capacity(STDERR_RETENTION))),
            client: None,
            sampling,
        }
    }

    pub async fn connect(&mut self) {
        self.status = ConnectionStatus::Connecting;
        self.errors.clear();
        self.cached_tools.clear();
        self.cached_resources.clear();
        self.cached_prompts.clear();
        self.instructions = None;
        let timeout = Duration::from_millis(self.config.timeout_ms());
        let result = match tokio::time::timeout(timeout, self.create_client(timeout)).await {
            Ok(inner) => inner,
            Err(_) => Err(loopal_error::McpError::ConnectionFailed(format!(
                "'{}' did not complete handshake within {:?}",
                self.name, timeout
            ))),
        };
        match result {
            Ok(client) => {
                if let Some(info) = client.peer_info() {
                    self.instructions = info.instructions.clone();
                }
                self.client = Some(client);
                self.discover_capabilities().await;
                // Connected even with discovery errors — transport works
                // but capabilities may be partial; check `errors` for details.
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

    pub async fn disconnect(&mut self) {
        self.client = None;
        self.status = ConnectionStatus::Disconnected;
        self.cached_tools.clear();
        self.cached_resources.clear();
        self.cached_prompts.clear();
        self.instructions = None;
    }

    pub fn client(&self) -> Option<&McpClient> {
        self.client.as_ref()
    }

    async fn create_client(&self, timeout: Duration) -> Result<McpClient, loopal_error::McpError> {
        let sampling = self.sampling.clone();
        match &self.config {
            McpServerConfig::Stdio {
                command, args, env, ..
            } => {
                transport::connect_stdio(
                    command,
                    args,
                    env,
                    timeout,
                    sampling,
                    Some(self.stderr_tail.clone()),
                )
                .await
            }
            McpServerConfig::StreamableHttp { url, headers, .. } => {
                transport::connect_http(url, headers, timeout, sampling).await
            }
        }
    }

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
