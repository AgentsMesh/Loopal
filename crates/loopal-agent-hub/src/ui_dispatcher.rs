//! UI dispatcher — manages UI client connections, event broadcast, permission relay.
//!
//! UI clients are NOT agents. Their connections are stored here, separate from
//! `AgentRegistry`. Permission relay uses these connections directly.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::broadcast;

use loopal_ipc::connection::Connection;
use loopal_protocol::AgentEvent;

/// A registered UI client with its server-side connection.
pub struct UiClientConn {
    /// Server-side IPC connection (Hub → UI client, for sending relay requests).
    pub conn: Arc<Connection>,
}

/// Manages UI client connections and event broadcasting.
pub struct UiDispatcher {
    /// Registered UI clients with their connections.
    pub(crate) clients: HashMap<String, UiClientConn>,
    /// Broadcast channel for agent events (multi-consumer delivery).
    pub(crate) event_broadcast: broadcast::Sender<AgentEvent>,
}

impl Default for UiDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl UiDispatcher {
    pub fn new() -> Self {
        let (broadcast_tx, _) = broadcast::channel(256);
        Self {
            clients: HashMap::new(),
            event_broadcast: broadcast_tx,
        }
    }

    /// Register a UI client with its server-side connection.
    pub fn register_client(&mut self, name: &str, conn: Arc<Connection>) {
        self.clients.insert(name.to_string(), UiClientConn { conn });
        tracing::info!(client = %name, "registered UI client");
    }

    /// Unregister a UI client.
    pub fn unregister_client(&mut self, name: &str) {
        self.clients.remove(name);
    }

    /// Check if a name is a registered UI client.
    pub fn is_ui_client(&self, name: &str) -> bool {
        self.clients.contains_key(name)
    }

    /// Get connections for all registered UI clients (for permission relay).
    pub fn get_client_connections(&self) -> Vec<(String, Arc<Connection>)> {
        self.clients
            .iter()
            .map(|(name, c)| (name.clone(), c.conn.clone()))
            .collect()
    }

    /// Subscribe to agent events. Returns a broadcast receiver.
    pub fn subscribe_events(&self) -> broadcast::Receiver<AgentEvent> {
        self.event_broadcast.subscribe()
    }

    /// Get the event broadcast sender (for event_router).
    pub fn event_broadcaster(&self) -> broadcast::Sender<AgentEvent> {
        self.event_broadcast.clone()
    }
}
