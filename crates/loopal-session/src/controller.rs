//! SessionController: observation + control + multi-agent connections.

use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::{mpsc, watch};

use loopal_protocol::{AgentEvent, ControlCommand, InterruptSignal, UserQuestionResponse};

use crate::controller_ops::ControlBackend;
use crate::event_handler;
use crate::state::SessionState;
use loopal_agent_hub::{HubClient, LocalChannels};

/// External handle — cheaply cloneable, shareable across consumers.
#[derive(Clone)]
pub struct SessionController {
    state: Arc<Mutex<SessionState>>,
    pub(crate) backend: Arc<ControlBackend>,
}

impl SessionController {
    /// Create with in-process channels (for unit tests — no real Hub).
    pub fn new(
        control_tx: mpsc::Sender<ControlCommand>,
        permission_tx: mpsc::Sender<bool>,
        question_tx: mpsc::Sender<UserQuestionResponse>,
        interrupt: InterruptSignal,
        interrupt_tx: Arc<watch::Sender<u64>>,
    ) -> Self {
        let channels = LocalChannels {
            control_tx,
            permission_tx,
            question_tx,
            mailbox_tx: None,
            interrupt,
            interrupt_tx,
        };
        Self {
            state: Arc::new(Mutex::new(SessionState::new())),
            backend: Arc::new(ControlBackend::Local(Arc::new(channels))),
        }
    }

    /// Create with a remote Hub IPC client (production mode — TCP attached).
    pub fn with_hub(client: Arc<HubClient>) -> Self {
        Self {
            state: Arc::new(Mutex::new(SessionState::new())),
            backend: Arc::new(ControlBackend::Hub(client)),
        }
    }

    /// Acquire the session state lock. Panics if the lock is poisoned.
    pub fn lock(&self) -> MutexGuard<'_, SessionState> {
        self.state.lock().expect("session state lock poisoned")
    }

    pub(crate) fn active_target(&self) -> String {
        self.lock().active_view.clone()
    }

    /// Interrupt the currently viewed agent.
    pub fn interrupt(&self) {
        let target = self.active_target();
        tracing::info!(target = %target, "session: interrupt signaled");
        self.backend.interrupt_target(&target);
    }

    /// Interrupt a specific named agent (e.g., to terminate from agent panel).
    pub fn interrupt_agent(&self, name: &str) {
        tracing::debug!(agent = %name, "session: interrupt agent");
        self.backend.interrupt_target(name);
    }

    /// Resolve a `ToolPermissionRequest` event for `(agent_name, tool_call_id)`.
    /// Caller clears `pending_permission` in the ViewClient.
    pub async fn respond_permission(&self, agent_name: &str, tool_call_id: &str, allow: bool) {
        self.backend
            .respond_permission(agent_name, tool_call_id, allow)
            .await;
    }

    /// Resolve a `UserQuestionRequest` event for `(agent_name, question_id)`.
    /// Caller clears `pending_question` in the ViewClient.
    pub async fn respond_question(
        &self,
        agent_name: &str,
        question_id: &str,
        answers: Vec<String>,
    ) {
        self.backend
            .respond_question(agent_name, question_id, answers)
            .await;
    }

    /// Cancel an in-flight `UserQuestionRequest`.
    pub async fn cancel_question(&self, agent_name: &str, question_id: &str) {
        self.backend.cancel_question(agent_name, question_id).await;
    }

    /// Send `hub/shutdown` to the Hub, asking it to shut down all agents
    /// and exit. No-op for in-process test backends without a real Hub.
    pub async fn shutdown_hub(&self) {
        let ControlBackend::Hub(client) = self.backend.as_ref() else {
            return;
        };
        client.shutdown_hub().await;
    }

    // === Event handling ===

    pub fn handle_event(&self, event: AgentEvent) {
        let mut state = self.lock();
        event_handler::apply_event(&mut state, event);
    }

    /// Fetch the list of agent names from the Hub. Returns empty for
    /// non-Hub backends.
    pub async fn fetch_agent_names(&self) -> Vec<String> {
        let ControlBackend::Hub(client) = self.backend.as_ref() else {
            return Vec::new();
        };
        let Ok(resp) = client.list_agents().await else {
            return Vec::new();
        };
        resp.get("agents")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Fetch the current `view/snapshot` for `agent` from the Hub.
    pub async fn fetch_view_snapshot(
        &self,
        agent: &str,
    ) -> Result<loopal_view_state::ViewSnapshot, String> {
        let ControlBackend::Hub(client) = self.backend.as_ref() else {
            return Err("not in hub mode".into());
        };
        let resp = client
            .connection()
            .send_request(
                loopal_ipc::protocol::methods::VIEW_SNAPSHOT.name,
                serde_json::json!({ "agent": agent }),
            )
            .await
            .map_err(|e| format!("view/snapshot for {agent}: {e}"))?;
        serde_json::from_value(resp).map_err(|e| format!("malformed snapshot for {agent}: {e}"))
    }

    /// Set the root session ID (for sub-agent ref persistence).
    pub fn set_root_session_id(&self, session_id: &str) {
        self.lock().root_session_id = Some(session_id.to_string());
    }

    pub fn root_session_id(&self) -> Option<String> {
        self.lock().root_session_id.clone()
    }
}
