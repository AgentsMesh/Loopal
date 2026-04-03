//! Shared session registry for multi-client access.

#![allow(dead_code)] // agent/join + agent/list methods used when wired into dispatch_loop

use std::sync::Arc;

use tokio::sync::Mutex;

use loopal_ipc::connection::Connection;
use loopal_protocol::InterruptSignal;

/// A connected client handle within a shared session.
pub struct ClientHandle {
    pub id: String,
    pub connection: Arc<Connection>,
    /// True if this is the primary client (handles permissions/questions).
    pub is_primary: bool,
}

/// A shared session that multiple clients can observe.
pub struct SharedSession {
    pub session_id: String,
    pub clients: Mutex<Vec<ClientHandle>>,
    /// Channel to send input into the agent loop.
    pub input_tx: tokio::sync::mpsc::Sender<InputFromClient>,
    /// Interrupt signal shared with the agent loop.
    pub interrupt: InterruptSignal,
    pub interrupt_tx: Arc<tokio::sync::watch::Sender<u64>>,
}

/// Input forwarded from a client connection to the agent loop.
pub enum InputFromClient {
    Message(loopal_protocol::Envelope),
    Control(loopal_protocol::ControlCommand),
    Interrupt,
}

/// Server-wide session registry.
#[derive(Default)]
pub struct SessionHub {
    sessions: Mutex<Vec<Arc<SharedSession>>>,
    /// Test-only: injected mock provider for session creation.
    test_provider: Mutex<Option<Arc<dyn loopal_provider_api::Provider>>>,
    /// Override base directory for session/message storage (test sandboxes).
    session_dir_override: Mutex<Option<std::path::PathBuf>>,
}

impl SessionHub {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(Vec::new()),
            test_provider: Mutex::new(None),
            session_dir_override: Mutex::new(None),
        }
    }

    /// Set a mock provider for testing (consumed on next session creation).
    pub async fn set_test_provider(&self, provider: Arc<dyn loopal_provider_api::Provider>) {
        *self.test_provider.lock().await = Some(provider);
    }

    /// Get the test provider (if set). Cloned — available for multiple sessions.
    pub async fn get_test_provider(&self) -> Option<Arc<dyn loopal_provider_api::Provider>> {
        self.test_provider.lock().await.clone()
    }

    /// Override session storage directory (for sandbox/test environments).
    pub async fn set_session_dir_override(&self, dir: std::path::PathBuf) {
        *self.session_dir_override.lock().await = Some(dir);
    }

    /// Get the session directory override, if set.
    pub async fn session_dir_override(&self) -> Option<std::path::PathBuf> {
        self.session_dir_override.lock().await.clone()
    }

    /// Register a new session.
    pub async fn register_session(&self, session: Arc<SharedSession>) {
        self.sessions.lock().await.push(session);
    }

    /// Find a session by ID.
    pub async fn find_session(&self, id: &str) -> Option<Arc<SharedSession>> {
        self.sessions
            .lock()
            .await
            .iter()
            .find(|s| s.session_id == id)
            .cloned()
    }

    /// List all active session IDs.
    pub async fn list_session_ids(&self) -> Vec<String> {
        self.sessions
            .lock()
            .await
            .iter()
            .map(|s| s.session_id.clone())
            .collect()
    }

    /// Remove a session when the agent loop completes.
    pub async fn remove_session(&self, id: &str) {
        self.sessions.lock().await.retain(|s| s.session_id != id);
    }
}

impl SharedSession {
    /// Create a placeholder session (for bootstrapping before session_id is known).
    pub fn placeholder(
        input_tx: tokio::sync::mpsc::Sender<InputFromClient>,
        interrupt: InterruptSignal,
        interrupt_tx: Arc<tokio::sync::watch::Sender<u64>>,
    ) -> Self {
        Self {
            session_id: String::new(),
            clients: Mutex::new(Vec::new()),
            input_tx,
            interrupt,
            interrupt_tx,
        }
    }

    /// Add a client to this session. First client becomes primary.
    pub async fn add_client(&self, id: String, connection: Arc<Connection>) {
        let mut clients = self.clients.lock().await;
        let is_primary = clients.is_empty();
        clients.push(ClientHandle {
            id,
            connection,
            is_primary,
        });
    }

    /// Remove a client. If the removed client was primary, promote the next.
    pub async fn remove_client(&self, client_id: &str) {
        let mut clients = self.clients.lock().await;
        let was_primary = clients
            .iter()
            .find(|c| c.id == client_id)
            .is_some_and(|c| c.is_primary);
        clients.retain(|c| c.id != client_id);
        if was_primary && let Some(first) = clients.first_mut() {
            first.is_primary = true;
            tracing::info!(client = %first.id, "promoted to primary");
        }
    }

    /// Get the primary client's connection (for permission/question routing).
    pub async fn primary_connection(&self) -> Option<Arc<Connection>> {
        self.clients
            .lock()
            .await
            .iter()
            .find(|c| c.is_primary)
            .map(|c| c.connection.clone())
    }

    /// Get all client connections (for event broadcast).
    pub async fn all_connections(&self) -> Vec<Arc<Connection>> {
        self.clients
            .lock()
            .await
            .iter()
            .map(|c| c.connection.clone())
            .collect()
    }

    /// Broadcast a raw AgentEvent to all clients (preserving agent_name).
    /// Used for sub-agent event forwarding where agent_name must be retained.
    pub async fn broadcast_event(&self, event: &loopal_protocol::AgentEvent) {
        if let Ok(params) = serde_json::to_value(event) {
            for conn in self.all_connections().await {
                let _ = conn
                    .send_notification(
                        loopal_ipc::protocol::methods::AGENT_EVENT.name,
                        params.clone(),
                    )
                    .await;
            }
        }
    }

    /// Remove disconnected clients by index (called by HubFrontend after broadcast).
    pub async fn remove_dead_connections(&self, dead_indices: &[usize]) {
        let mut clients = self.clients.lock().await;
        // Remove in reverse order to preserve indices
        for &idx in dead_indices.iter().rev() {
            if idx < clients.len() {
                let removed = clients.remove(idx);
                tracing::info!(client = %removed.id, "removed dead connection");
            }
        }
        // Promote new primary if needed
        let has_primary = clients.iter().any(|c| c.is_primary);
        if !has_primary && let Some(first) = clients.first_mut() {
            first.is_primary = true;
            tracing::info!(client = %first.id, "promoted to primary (dead cleanup)");
        }
    }
}
