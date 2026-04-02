//! Event aggregator — unified broadcast channel for cross-hub agent events.
//!
//! Events are fed from `meta_hub_io_loop` (which receives `agent/event`
//! notifications from each Sub-Hub), prefixed with hub name, and sent
//! to this aggregator's broadcast channel.
//!
//! UI clients subscribe to the broadcast to observe all agent activity
//! across the entire cluster.

use tokio::sync::broadcast;

use loopal_protocol::AgentEvent;

/// Aggregates agent events from all connected sub-hubs into a single stream.
///
/// Events are pushed in by the per-Sub-Hub IO loop (not pulled by background tasks).
/// This avoids duplicate event paths and keeps the event flow explicit.
pub struct EventAggregator {
    /// Unified broadcast sender — UI clients subscribe to this.
    broadcast_tx: broadcast::Sender<AgentEvent>,
}

impl Default for EventAggregator {
    fn default() -> Self {
        Self::new()
    }
}

impl EventAggregator {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(512);
        Self { broadcast_tx: tx }
    }

    /// Subscribe to the aggregated event stream.
    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.broadcast_tx.subscribe()
    }

    /// Get the broadcast sender (for IO loops to push events).
    pub fn broadcaster(&self) -> broadcast::Sender<AgentEvent> {
        self.broadcast_tx.clone()
    }
}

/// Prefix the event's agent_name with `"hub_name/"` for global uniqueness.
pub fn prefix_agent_name(event: &mut AgentEvent, hub_name: &str) {
    if let Some(ref agent_name) = event.agent_name {
        event.agent_name = Some(format!("{hub_name}/{agent_name}"));
    } else {
        event.agent_name = Some(hub_name.to_string());
    }
}
