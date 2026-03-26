//! ACP method dispatch and agent loop lifecycle management.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;
use tokio::sync::{Mutex, mpsc};
use tokio_util::sync::CancellationToken;
use tracing::info;

use loopal_config::ResolvedConfig;
use loopal_protocol::AgentEvent;
use loopal_provider_api::Provider;
use loopal_runtime::AgentInput;

use crate::jsonrpc::JsonRpcTransport;
use crate::types::*;

/// Active ACP session state.
pub struct ActiveSession {
    pub id: String,
    pub input_tx: mpsc::Sender<AgentInput>,
    pub event_rx: Mutex<mpsc::Receiver<AgentEvent>>,
    pub cancel_token: CancellationToken,
}

/// ACP handler — dispatches JSON-RPC methods and manages the agent session.
pub struct AcpHandler {
    pub transport: Arc<JsonRpcTransport>,
    pub session: Mutex<Option<ActiveSession>>,
    pub config: ResolvedConfig,
    pub cwd: PathBuf,
    /// Optional provider override for testing (injected into Kernel).
    pub(crate) provider_override: Option<Arc<dyn Provider>>,
    /// Optional base directory for session storage (defaults to ~/.loopal).
    pub(crate) session_base_dir: Option<PathBuf>,
}

impl AcpHandler {
    pub fn new(transport: Arc<JsonRpcTransport>, config: ResolvedConfig, cwd: PathBuf) -> Self {
        Self {
            transport,
            session: Mutex::new(None),
            config,
            cwd,
            provider_override: None,
            session_base_dir: None,
        }
    }

    /// Create a handler with a mock provider and isolated session storage.
    ///
    /// Used for E2E testing where real LLM calls and persistent storage
    /// are not desired.
    pub fn with_test_overrides(
        transport: Arc<JsonRpcTransport>,
        config: ResolvedConfig,
        cwd: PathBuf,
        provider: Arc<dyn Provider>,
        session_base_dir: PathBuf,
    ) -> Self {
        Self {
            transport,
            session: Mutex::new(None),
            config,
            cwd,
            provider_override: Some(provider),
            session_base_dir: Some(session_base_dir),
        }
    }

    /// Dispatch a JSON-RPC request to the appropriate handler.
    pub async fn dispatch(&self, id: i64, method: &str, params: Value) {
        match method {
            "initialize" => self.handle_initialize(id, params).await,
            "session/new" => self.handle_new_session(id, params).await,
            "session/prompt" => self.handle_prompt(id, params).await,
            "session/cancel" => self.handle_cancel(id).await,
            _ => {
                self.transport
                    .respond_error(
                        id,
                        crate::jsonrpc::METHOD_NOT_FOUND,
                        &format!("unknown method: {method}"),
                    )
                    .await;
            }
        }
    }

    async fn handle_initialize(&self, id: i64, params: Value) {
        let _params: InitializeParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => {
                self.transport
                    .respond_error(id, crate::jsonrpc::INVALID_REQUEST, &e.to_string())
                    .await;
                return;
            }
        };

        let result = InitializeResult {
            protocol_version: 1,
            agent_capabilities: AgentCapabilities { streaming: true },
            agent_info: AgentInfo {
                name: "loopal".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };
        let value = serde_json::to_value(result).unwrap_or_default();
        self.transport.respond(id, value).await;
        info!("ACP initialized");
    }

    async fn handle_cancel(&self, id: i64) {
        let guard = self.session.lock().await;
        if let Some(ref session) = *guard {
            session.cancel_token.cancel();
            self.transport.respond(id, Value::Null).await;
        } else {
            self.transport
                .respond_error(id, crate::jsonrpc::INVALID_REQUEST, "no active session")
                .await;
        }
    }
}
