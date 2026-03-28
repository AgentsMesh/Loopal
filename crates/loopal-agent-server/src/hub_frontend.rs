//! Multi-client AgentFrontend that broadcasts events and routes permissions.
//!
//! Replaces per-connection `IpcFrontend` when a session has multiple clients.
//! Events are fanned out to all connected clients; permissions and questions
//! are routed to the primary client only.

#![allow(dead_code)] // replace_session used only by session_start

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tracing::{debug, info};

use loopal_error::{LoopalError, Result};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload, Question, UserQuestionResponse};
use loopal_runtime::agent_input::AgentInput;
use loopal_runtime::frontend::traits::{AgentFrontend, EventEmitter};
use loopal_tool_api::PermissionDecision;

use crate::hub_emitter::HubEventEmitter;
use crate::session_hub::{InputFromClient, SharedSession};

/// Frontend that multiplexes across all clients in a shared session.
pub struct HubFrontend {
    session: tokio::sync::RwLock<Arc<SharedSession>>,
    input_rx: tokio::sync::Mutex<tokio::sync::mpsc::Receiver<InputFromClient>>,
    agent_name: Option<String>,
    /// Watch channel for interrupt detection in recv_input.
    interrupt_rx: tokio::sync::Mutex<tokio::sync::watch::Receiver<u64>>,
}

impl HubFrontend {
    pub fn new(
        session: Arc<SharedSession>,
        input_rx: tokio::sync::mpsc::Receiver<InputFromClient>,
        agent_name: Option<String>,
        interrupt_rx: tokio::sync::watch::Receiver<u64>,
    ) -> Self {
        Self {
            session: tokio::sync::RwLock::new(session),
            input_rx: tokio::sync::Mutex::new(input_rx),
            agent_name,
            interrupt_rx: tokio::sync::Mutex::new(interrupt_rx),
        }
    }

    /// Replace the session reference (after session_id is known).
    pub async fn replace_session(&self, session: Arc<SharedSession>) {
        *self.session.write().await = session;
    }

    async fn get_session(&self) -> Arc<SharedSession> {
        self.session.read().await.clone()
    }
}

#[async_trait]
impl AgentFrontend for HubFrontend {
    async fn emit(&self, payload: AgentEventPayload) -> Result<()> {
        let event = AgentEvent {
            agent_name: self.agent_name.clone(),
            payload,
        };
        let params = serde_json::to_value(&event)
            .map_err(|e| LoopalError::Ipc(format!("serialize event: {e}")))?;
        let session = self.get_session().await;
        let conns = session.all_connections().await;
        let mut dead_clients = Vec::new();
        for (i, conn) in conns.iter().enumerate() {
            if conn
                .send_notification(methods::AGENT_EVENT.name, params.clone())
                .await
                .is_err()
            {
                dead_clients.push(i);
            }
        }
        // Remove disconnected clients
        if !dead_clients.is_empty() {
            session.remove_dead_connections(&dead_clients).await;
        }
        Ok(())
    }

    async fn recv_input(&self) -> Option<AgentInput> {
        let mut rx = self.input_rx.lock().await;
        let mut interrupt_rx = self.interrupt_rx.lock().await;
        // Consume stale interrupt notification from a previous turn.
        // By the time the agent loop re-enters recv_input(), the interrupt
        // has already been handled by TurnCancel. Without this, changed()
        // fires immediately on the old value and exits the agent loop.
        interrupt_rx.borrow_and_update();
        loop {
            tokio::select! {
                msg = rx.recv() => {
                    match msg? {
                        InputFromClient::Message(env) => return Some(AgentInput::Message(env)),
                        InputFromClient::Control(cmd) => return Some(AgentInput::Control(cmd)),
                        InputFromClient::Interrupt => continue,
                    }
                }
                _ = interrupt_rx.changed() => {
                    return None; // Interrupted — exit agent loop
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
        info!(tool = name, "requesting permission via IPC");
        let session = self.get_session().await;
        let Some(conn) = session.primary_connection().await else {
            info!(tool = name, "permission denied: no primary connection");
            return PermissionDecision::Deny;
        };
        let params = serde_json::json!({
            "tool_call_id": id,
            "tool_name": name,
            "tool_input": input,
        });
        match conn
            .send_request(methods::AGENT_PERMISSION.name, params)
            .await
        {
            Ok(value) => {
                let allow = value.get("allow").and_then(Value::as_bool).unwrap_or(false);
                info!(tool = name, allow, "permission response received");
                if allow {
                    PermissionDecision::Allow
                } else {
                    PermissionDecision::Deny
                }
            }
            Err(e) => {
                info!(tool = name, error = %e, "permission IPC failed");
                PermissionDecision::Deny
            }
        }
    }

    fn event_emitter(&self) -> Box<dyn EventEmitter> {
        let session = self.session.try_read().ok().map(|g| g.clone());
        Box::new(HubEventEmitter {
            session,
            agent_name: self.agent_name.clone(),
        })
    }

    async fn ask_user(&self, questions: Vec<Question>) -> Vec<String> {
        debug!(count = questions.len(), "asking user via hub");
        let session = self.get_session().await;
        let Some(conn) = session.primary_connection().await else {
            return vec!["(no primary client)".into()];
        };
        let params = serde_json::json!({ "questions": questions });
        match conn
            .send_request(methods::AGENT_QUESTION.name, params)
            .await
        {
            Ok(value) => serde_json::from_value::<UserQuestionResponse>(value)
                .map(|r| r.answers)
                .unwrap_or_else(|_| vec!["(parse error)".into()]),
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
        let session = match self.session.try_read() {
            Ok(guard) => guard.clone(),
            Err(_) => return false,
        };
        tokio::spawn(async move {
            for conn in session.all_connections().await {
                let _ = conn
                    .send_notification(methods::AGENT_EVENT.name, params.clone())
                    .await;
            }
        });
        true
    }
}
