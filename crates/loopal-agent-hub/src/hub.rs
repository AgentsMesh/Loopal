//! Hub — thin coordination layer over AgentRegistry + UiDispatcher.
//!
//! Agents and UI clients are managed by separate subsystems.
//! Hub ties them together: agent events flow to UI via broadcast,
//! permission requests flow from agents to UI clients via relay.

use std::sync::Arc;

use tokio::sync::mpsc;

use loopal_protocol::AgentEvent;

use crate::agent_registry::AgentRegistry;
use crate::ui_dispatcher::UiDispatcher;
use crate::uplink::HubUplink;

/// Central coordinator — delegates to specialized subsystems.
pub struct Hub {
    /// Agent connections, lifecycle, routing, completion.
    pub registry: AgentRegistry,
    /// UI client connections, event broadcast, permission relay.
    pub ui: UiDispatcher,
    /// Optional uplink to a parent MetaHub for cross-hub communication.
    /// `None` = standalone mode (default, identical to pre-MetaHub behavior).
    /// `Some(...)` = cluster mode (local misses escalate to MetaHub).
    pub uplink: Option<Arc<HubUplink>>,
    /// TCP listener port, set after `start_hub_listener`. `None` if not listening.
    pub listener_port: Option<u16>,
}

impl Hub {
    pub fn new(event_tx: mpsc::Sender<AgentEvent>) -> Self {
        Self {
            registry: AgentRegistry::new(event_tx),
            ui: UiDispatcher::new(),
            uplink: None,
            listener_port: None,
        }
    }

    /// Create a no-op Hub (for tests that don't need real connections).
    pub fn noop() -> Self {
        let (tx, _rx) = mpsc::channel(1);
        Self {
            registry: AgentRegistry::new(tx),
            ui: UiDispatcher::new(),
            uplink: None,
            listener_port: None,
        }
    }
}
