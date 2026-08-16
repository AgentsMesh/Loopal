use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use loopal_config::McpServerConfig;
use loopal_secret_client::SecretClient;
use loopal_tool_api::ToolDefinition;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::client::McpClient;
use crate::connection_discovery::discover_capabilities;
use crate::connection_generation::ConnectionGeneration;
use crate::handler::SamplingCallback;
use crate::result_sanitizer::CallResultSanitizer;
use crate::secret_expand::{CONFIG_SECRET_ERROR, resolve_mcp_secret_seed};
use crate::secret_provenance::SecretProvenance;
use crate::types::{ConnectionStatus, McpPrompt, McpResource};

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
    pub stderr_tail: Arc<Mutex<VecDeque<String>>>,
    client: Option<McpClient>,
    sampling: Option<Arc<dyn SamplingCallback>>,
    secret_client: Option<Arc<dyn SecretClient>>,
    secret_provenance: Arc<SecretProvenance>,
    generation: ConnectionGeneration,
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
            secret_client: None,
            secret_provenance: Arc::new(SecretProvenance::default()),
            generation: ConnectionGeneration::new(),
        }
    }

    pub(crate) fn with_secret_client(mut self, client: Option<Arc<dyn SecretClient>>) -> Self {
        self.secret_client = client;
        self
    }

    pub(crate) fn generation(&self) -> ConnectionGeneration {
        self.generation.clone()
    }

    pub(crate) fn owns_generation(&self, generation: &ConnectionGeneration) -> bool {
        self.generation.is(generation)
    }

    pub(crate) async fn result_sanitizer(&self) -> Result<CallResultSanitizer, &'static str> {
        resolve_mcp_secret_seed(
            &self.config,
            self.secret_client.as_ref(),
            loopal_ipc::HUB_RPC_BUDGET,
        )
        .await
        .and_then(|seed| {
            self.secret_provenance.establish(&seed)?;
            Ok(
                match self.client.as_ref().and_then(McpClient::oauth_credentials) {
                    Some(credentials) => {
                        CallResultSanitizer::with_oauth_credentials(&seed, credentials.clone())
                    }
                    None => CallResultSanitizer::new(&seed),
                },
            )
        })
    }

    #[cfg(test)]
    pub(crate) fn with_client(mut self, client: McpClient) -> Self {
        self.client = Some(client);
        self.status = ConnectionStatus::Connected;
        self
    }

    pub async fn connect(&mut self) {
        self.generation = ConnectionGeneration::new();
        self.status = ConnectionStatus::Connecting;
        self.errors.clear();
        self.cached_tools.clear();
        self.cached_resources.clear();
        self.cached_prompts.clear();
        self.instructions = None;
        if self.secret_provenance.reset().is_err() {
            self.fail(CONFIG_SECRET_ERROR);
            return;
        }
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
                let sanitizer = match self.result_sanitizer().await {
                    Ok(sanitizer) => sanitizer,
                    Err(_) => {
                        self.fail(CONFIG_SECRET_ERROR);
                        return;
                    }
                };
                if let Some(info) = client.peer_info() {
                    self.instructions = info
                        .instructions
                        .as_deref()
                        .map(|value| sanitizer.sanitize_text(value));
                }
                self.client = Some(client);
                discover_capabilities(self, &sanitizer).await;
                self.status = ConnectionStatus::Connected;
                if self.errors.is_empty() {
                    info!(server = %self.name, tools = self.cached_tools.len(), "connected");
                } else {
                    warn!(server = %self.name, errors = ?self.errors, "connected with errors");
                }
            }
            Err(loopal_error::McpError::Protocol(message)) if message == CONFIG_SECRET_ERROR => {
                self.fail(CONFIG_SECRET_ERROR)
            }
            Err(_) => self.fail("connection failed: MCP connection failed"),
        }
    }

    pub async fn disconnect(&mut self) {
        self.generation = ConnectionGeneration::new();
        let credentials = self
            .client
            .as_ref()
            .and_then(McpClient::oauth_credentials)
            .cloned();
        if let Some(client) = self.client.as_mut() {
            client
                .close(Duration::from_millis(self.config.timeout_ms()))
                .await;
        }
        self.client = None;
        if let Some(credentials) = credentials {
            credentials.deactivate();
        }
        self.status = ConnectionStatus::Disconnected;
        self.cached_tools.clear();
        self.cached_resources.clear();
        self.cached_prompts.clear();
        self.instructions = None;
    }

    pub fn client(&self) -> Option<&McpClient> {
        self.client.as_ref()
    }

    fn fail(&mut self, message: &str) {
        let message = message.to_string();
        self.errors.push(message.clone());
        self.status = ConnectionStatus::Failed(message);
    }
}

#[path = "connection_create.rs"]
mod create;

#[cfg(test)]
#[path = "connection_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "connection_branch_tests.rs"]
mod branch_tests;
