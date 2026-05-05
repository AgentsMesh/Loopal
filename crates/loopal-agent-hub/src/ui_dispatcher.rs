//! UI dispatcher — manages UI client connections and event broadcast.
//!
//! UI clients are NOT agents. They register here so the Hub can:
//! - Track which UIs are connected (no-UI fast deny in `pending_relay`)
//! - Broadcast `AgentEvent`s to every connected UI

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::broadcast;

use loopal_ipc::connection::Connection;
use loopal_protocol::AgentEvent;

pub struct UiDispatcher {
    pub(crate) clients: HashMap<String, Arc<Connection>>,
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

    pub fn register_client(&mut self, name: &str, conn: Arc<Connection>) {
        self.clients.insert(name.to_string(), conn);
        tracing::info!(client = %name, "registered UI client");
    }

    pub fn unregister_client(&mut self, name: &str) {
        self.clients.remove(name);
    }

    pub fn is_ui_client(&self, name: &str) -> bool {
        self.clients.contains_key(name)
    }

    pub fn clients_is_empty(&self) -> bool {
        self.clients.is_empty()
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<AgentEvent> {
        self.event_broadcast.subscribe()
    }

    pub fn event_broadcaster(&self) -> broadcast::Sender<AgentEvent> {
        self.event_broadcast.clone()
    }
}
