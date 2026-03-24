//! `AcpFrontend` — implements `AgentFrontend` for the ACP transport.
//!
//! Bridges the agent loop with the ACP JSON-RPC IO loop:
//! - `emit()` forwards events to the handler's event loop via channel
//! - `recv_input()` waits for messages forwarded from ACP `session/prompt`
//! - `request_permission()` sends an ACP request to the client and awaits the response

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::{Mutex, mpsc};
use tokio_util::sync::CancellationToken;

use loopal_error::{LoopalError, Result};
use loopal_protocol::{AgentEvent, AgentEventPayload, Question};
use loopal_runtime::AgentInput;
use loopal_runtime::frontend::traits::{AgentFrontend, EventEmitter};
use loopal_tool_api::PermissionDecision;

use crate::jsonrpc::JsonRpcTransport;
use crate::types::{PermissionOutcome, RequestPermissionParams, RequestPermissionResult};

/// ACP-backed frontend that replaces `UnifiedFrontend` when running in `--acp` mode.
pub struct AcpFrontend {
    agent_name: Option<String>,
    event_tx: mpsc::Sender<AgentEvent>,
    input_rx: Mutex<mpsc::Receiver<AgentInput>>,
    transport: Arc<JsonRpcTransport>,
    session_id: String,
    cancel_token: CancellationToken,
}

impl AcpFrontend {
    pub fn new(
        agent_name: Option<String>,
        event_tx: mpsc::Sender<AgentEvent>,
        input_rx: mpsc::Receiver<AgentInput>,
        transport: Arc<JsonRpcTransport>,
        session_id: String,
        cancel_token: CancellationToken,
    ) -> Self {
        Self {
            agent_name,
            event_tx,
            input_rx: Mutex::new(input_rx),
            transport,
            session_id,
            cancel_token,
        }
    }
}

#[async_trait]
impl AgentFrontend for AcpFrontend {
    async fn emit(&self, payload: AgentEventPayload) -> Result<()> {
        let event = AgentEvent {
            agent_name: self.agent_name.clone(),
            payload,
        };
        self.event_tx
            .send(event)
            .await
            .map_err(|e| LoopalError::Other(format!("ACP event channel closed: {e}")))
    }

    async fn recv_input(&self) -> Option<AgentInput> {
        let mut rx = self.input_rx.lock().await;
        tokio::select! {
            msg = rx.recv() => msg,
            () = self.cancel_token.cancelled() => {
                tracing::info!("ACP cancellation triggered");
                None
            }
        }
    }

    async fn request_permission(
        &self,
        id: &str,
        name: &str,
        input: &serde_json::Value,
    ) -> PermissionDecision {
        let params = RequestPermissionParams {
            session_id: self.session_id.clone(),
            tool_call_id: id.to_string(),
            tool_name: name.to_string(),
            tool_input: input.clone(),
        };
        let params_value = match serde_json::to_value(params) {
            Ok(v) => v,
            Err(_) => return PermissionDecision::Deny,
        };

        match self
            .transport
            .request("session/requestPermission", params_value)
            .await
        {
            Ok(value) => match serde_json::from_value::<RequestPermissionResult>(value) {
                Ok(result) => match result.outcome {
                    PermissionOutcome::Allow => PermissionDecision::Allow,
                    PermissionOutcome::Deny => PermissionDecision::Deny,
                },
                Err(_) => PermissionDecision::Deny,
            },
            Err(_) => PermissionDecision::Deny,
        }
    }

    fn event_emitter(&self) -> Box<dyn EventEmitter> {
        Box::new(AcpEventEmitter {
            tx: self.event_tx.clone(),
            agent_name: self.agent_name.clone(),
        })
    }

    async fn ask_user(&self, _questions: Vec<Question>) -> Vec<String> {
        vec!["(not supported in ACP mode)".into()]
    }

    fn try_emit(&self, payload: AgentEventPayload) -> bool {
        let event = AgentEvent {
            agent_name: self.agent_name.clone(),
            payload,
        };
        self.event_tx.try_send(event).is_ok()
    }
}

/// Cloneable event emitter for use inside `tokio::spawn` blocks.
#[derive(Clone)]
struct AcpEventEmitter {
    tx: mpsc::Sender<AgentEvent>,
    agent_name: Option<String>,
}

#[async_trait]
impl EventEmitter for AcpEventEmitter {
    async fn emit(&self, payload: AgentEventPayload) -> Result<()> {
        let event = AgentEvent {
            agent_name: self.agent_name.clone(),
            payload,
        };
        self.tx
            .send(event)
            .await
            .map_err(|e| LoopalError::Other(format!("ACP event emitter channel closed: {e}")))
    }
}
