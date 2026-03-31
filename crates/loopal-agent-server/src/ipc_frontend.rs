use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::Mutex;
use tracing::debug;

use loopal_error::{LoopalError, Result};
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_protocol::{
    AgentEvent, AgentEventPayload, ControlCommand, Envelope, Question, UserQuestionResponse,
};
use loopal_runtime::agent_input::AgentInput;
use loopal_runtime::frontend::traits::{AgentFrontend, EventEmitter};
use loopal_tool_api::PermissionDecision;
use tokio::sync::mpsc;

use crate::ipc_emitter::IpcEventEmitter;

/// Agent frontend that communicates with consumers via IPC.
pub struct IpcFrontend {
    connection: Arc<Connection>,
    incoming_rx: Mutex<mpsc::Receiver<Incoming>>,
    agent_name: Option<String>,
}

impl IpcFrontend {
    pub fn new(
        connection: Arc<Connection>,
        incoming_rx: mpsc::Receiver<Incoming>,
        agent_name: Option<String>,
    ) -> Self {
        Self {
            connection,
            incoming_rx: Mutex::new(incoming_rx),
            agent_name,
        }
    }
}

#[async_trait]
impl AgentFrontend for IpcFrontend {
    async fn emit(&self, payload: AgentEventPayload) -> Result<()> {
        let event = AgentEvent {
            agent_name: self.agent_name.clone(),
            payload,
        };
        let params = serde_json::to_value(&event)
            .map_err(|e| LoopalError::Ipc(format!("serialize event: {e}")))?;
        self.connection
            .send_notification(methods::AGENT_EVENT.name, params)
            .await
            .map_err(LoopalError::Ipc)
    }

    async fn recv_input(&self) -> Option<AgentInput> {
        let mut rx = self.incoming_rx.lock().await;
        loop {
            let incoming = rx.recv().await?;
            match incoming {
                Incoming::Request { id, method, params } => {
                    let err_code = loopal_ipc::jsonrpc::INVALID_REQUEST;
                    match method.as_str() {
                        m if m == methods::AGENT_MESSAGE.name => {
                            match serde_json::from_value::<Envelope>(params) {
                                Ok(env) => {
                                    debug!(target_agent = %env.target, "recv_input: message");
                                    let _ = self
                                        .connection
                                        .respond(id, serde_json::json!({"ok": true}))
                                        .await;
                                    return Some(AgentInput::Message(env));
                                }
                                Err(e) => {
                                    tracing::warn!("malformed agent/message: {e}");
                                    let _ = self
                                        .connection
                                        .respond_error(id, err_code, &e.to_string())
                                        .await;
                                }
                            }
                        }
                        m if m == methods::AGENT_CONTROL.name => {
                            match serde_json::from_value::<ControlCommand>(params) {
                                Ok(cmd) => {
                                    debug!(?cmd, "recv_input: control");
                                    let _ = self
                                        .connection
                                        .respond(id, serde_json::json!({"ok": true}))
                                        .await;
                                    return Some(AgentInput::Control(cmd));
                                }
                                Err(e) => {
                                    tracing::warn!("malformed agent/control: {e}");
                                    let _ = self
                                        .connection
                                        .respond_error(id, err_code, &e.to_string())
                                        .await;
                                }
                            }
                        }
                        _ => {
                            let _ = self
                                .connection
                                .respond_error(
                                    id,
                                    loopal_ipc::jsonrpc::METHOD_NOT_FOUND,
                                    &format!("unknown: {method}"),
                                )
                                .await;
                        }
                    }
                }
                Incoming::Notification { .. } => {
                    // Interrupt notifications are intercepted by the
                    // interrupt_filter before reaching here. Any remaining
                    // notifications are silently skipped.
                }
            }
        }
    }

    async fn request_permission(
        &self,
        id: &str,
        name: &str,
        input: &serde_json::Value,
    ) -> PermissionDecision {
        debug!(tool = name, "requesting permission via IPC");
        let params = serde_json::json!({
            "tool_call_id": id,
            "tool_name": name,
            "tool_input": input,
        });
        let decision = match self
            .connection
            .send_request(methods::AGENT_PERMISSION.name, params)
            .await
        {
            Ok(value) => {
                if value.get("allow").and_then(Value::as_bool).unwrap_or(false) {
                    PermissionDecision::Allow
                } else {
                    PermissionDecision::Deny
                }
            }
            Err(_) => PermissionDecision::Deny,
        };
        debug!(tool = name, ?decision, "permission decision");
        decision
    }

    fn event_emitter(&self) -> Box<dyn EventEmitter> {
        Box::new(IpcEventEmitter {
            connection: self.connection.clone(),
            agent_name: self.agent_name.clone(),
        })
    }

    async fn ask_user(&self, questions: Vec<Question>) -> Vec<String> {
        debug!(count = questions.len(), "asking user via IPC");
        let params = serde_json::json!({ "questions": questions });
        match self
            .connection
            .send_request(methods::AGENT_QUESTION.name, params)
            .await
        {
            Ok(value) => {
                if let Ok(resp) = serde_json::from_value::<UserQuestionResponse>(value) {
                    resp.answers
                } else {
                    vec!["(IPC parse error)".into()]
                }
            }
            Err(_) => vec!["(IPC error)".into()],
        }
    }

    fn try_emit(&self, payload: AgentEventPayload) -> bool {
        let event = AgentEvent {
            agent_name: self.agent_name.clone(),
            payload,
        };
        let Ok(params) = serde_json::to_value(&event) else {
            return false;
        };
        let conn = self.connection.clone();
        tokio::spawn(async move {
            let _ = conn
                .send_notification(methods::AGENT_EVENT.name, params)
                .await;
        });
        true
    }
}
