//! UI dispatcher — manages UI client connections and event broadcast.
//!
//! UI clients are NOT agents. They register here so the Hub can:
//! - Track which UIs are connected (no-UI fast deny in `pending_relay`)
//! - Broadcast `AgentEvent`s to every connected UI

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{broadcast, watch};

use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::{AgentEvent, UiCapabilities, UiCapability};

pub(crate) struct UiClient {
    pub name: String,
    #[allow(dead_code)]
    pub connection: Arc<Connection<Listening>>,
    pub capabilities: UiCapabilities,
}

/// Monotonic snapshot of the interactive UI surface currently attached.
///
/// Consumers use `generation` to observe lease topology changes without
/// polling. `capabilities` is the union across all live UI leases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiCapabilitySnapshot {
    pub generation: u64,
    pub capabilities: UiCapabilities,
}

pub struct UiDispatcher {
    pub(crate) clients: HashMap<String, UiClient>,
    pub(crate) event_broadcast: broadcast::Sender<AgentEvent>,
    resync_broadcast: broadcast::Sender<()>,
    capability_state: watch::Sender<UiCapabilitySnapshot>,
}

impl Default for UiDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl UiDispatcher {
    pub fn new() -> Self {
        let (broadcast_tx, _) = broadcast::channel(256);
        let (resync_broadcast, _) = broadcast::channel(16);
        let (capability_state, _) = watch::channel(UiCapabilitySnapshot {
            generation: 0,
            capabilities: UiCapabilities::NONE,
        });
        Self {
            clients: HashMap::new(),
            event_broadcast: broadcast_tx,
            resync_broadcast,
            capability_state,
        }
    }

    pub fn register_client(
        &mut self,
        name: &str,
        conn: Arc<Connection<Listening>>,
        capabilities: UiCapabilities,
    ) -> String {
        let lease_id = uuid::Uuid::new_v4().to_string();
        self.register_client_with_lease(&lease_id, name, conn, capabilities);
        lease_id
    }

    pub fn register_client_with_lease(
        &mut self,
        lease_id: &str,
        name: &str,
        conn: Arc<Connection<Listening>>,
        capabilities: UiCapabilities,
    ) {
        self.clients.insert(
            lease_id.to_string(),
            UiClient {
                name: name.to_string(),
                connection: conn,
                capabilities,
            },
        );
        self.publish_capabilities();
        tracing::info!(client = %name, lease_id, ?capabilities, "registered UI client");
    }

    pub fn unregister_client(&mut self, lease_id: &str) {
        if self.clients.remove(lease_id).is_some() {
            self.publish_capabilities();
        }
    }

    pub fn is_ui_client(&self, lease_id: &str) -> bool {
        self.clients.contains_key(lease_id)
    }

    pub fn has_client_name(&self, name: &str) -> bool {
        self.clients.values().any(|client| client.name == name)
    }

    pub fn clients_is_empty(&self) -> bool {
        self.clients.is_empty()
    }

    pub fn has_capability(&self, capability: UiCapability) -> bool {
        self.clients
            .values()
            .any(|client| client.capabilities.supports(capability))
    }

    pub fn client_has_capability(&self, lease_id: &str, capability: UiCapability) -> bool {
        self.clients
            .get(lease_id)
            .is_some_and(|client| client.capabilities.supports(capability))
    }

    pub(crate) fn client_lease(
        &self,
        lease_id: &str,
    ) -> Option<(String, UiCapabilities, Arc<Connection<Listening>>)> {
        self.clients.get(lease_id).map(|client| {
            (
                client.name.clone(),
                client.capabilities,
                client.connection.clone(),
            )
        })
    }

    pub fn capability_snapshot(&self) -> UiCapabilitySnapshot {
        *self.capability_state.borrow()
    }

    pub fn subscribe_capabilities(&self) -> watch::Receiver<UiCapabilitySnapshot> {
        self.capability_state.subscribe()
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<AgentEvent> {
        self.event_broadcast.subscribe()
    }

    pub fn event_broadcaster(&self) -> broadcast::Sender<AgentEvent> {
        self.event_broadcast.clone()
    }

    pub fn subscribe_resync(&self) -> broadcast::Receiver<()> {
        self.resync_broadcast.subscribe()
    }

    pub(crate) fn request_resync(&self) {
        let _ = self.resync_broadcast.send(());
    }

    fn publish_capabilities(&self) {
        let capabilities =
            self.clients
                .values()
                .fold(UiCapabilities::NONE, |mut aggregate, client| {
                    aggregate.permission |= client.capabilities.permission;
                    aggregate.question |= client.capabilities.question;
                    aggregate.plan_approval |= client.capabilities.plan_approval;
                    aggregate
                });
        let generation = self.capability_state.borrow().generation.saturating_add(1);
        self.capability_state.send_replace(UiCapabilitySnapshot {
            generation,
            capabilities,
        });
    }
}
