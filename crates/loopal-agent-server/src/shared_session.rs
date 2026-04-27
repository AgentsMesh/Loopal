//! `SharedSession` + `ClientHandle` — multi-client observers attached
//! to a single agent loop.
//!
//! Split out of [`crate::session_hub`] so that file stays focused on
//! the server-wide registry and storage singletons. The two types are
//! tightly coupled (every `SharedSession` holds a `Vec<ClientHandle>`)
//! but logically separate from the registry that owns sessions.

use std::sync::Arc;

use tokio::sync::Mutex;

use loopal_ipc::connection::Connection;
use loopal_protocol::InterruptSignal;
use loopal_runtime::agent_input::AgentInput;

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
    pub input_tx: tokio::sync::mpsc::Sender<AgentInput>,
    /// Interrupt signal shared with the agent loop.
    pub interrupt: InterruptSignal,
    pub interrupt_tx: Arc<tokio::sync::watch::Sender<u64>>,
}

impl SharedSession {
    /// Create a placeholder session (for bootstrapping before session_id is known).
    pub fn placeholder(
        input_tx: tokio::sync::mpsc::Sender<AgentInput>,
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
