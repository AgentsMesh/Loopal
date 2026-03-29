//! Hub — thin coordination layer over AgentRegistry + UiDispatcher.
//!
//! Agents and UI clients are managed by separate subsystems.
//! Hub ties them together: agent events flow to UI via broadcast,
//! permission requests flow from agents to UI clients via relay.

use tokio::sync::mpsc;

use loopal_protocol::AgentEvent;

use crate::agent_registry::AgentRegistry;
use crate::ui_dispatcher::UiDispatcher;

/// Central coordinator — delegates to specialized subsystems.
pub struct Hub {
    /// Agent connections, lifecycle, routing, completion.
    pub registry: AgentRegistry,
    /// UI client connections, event broadcast, permission relay.
    pub ui: UiDispatcher,
}

impl Hub {
    pub fn new(event_tx: mpsc::Sender<AgentEvent>) -> Self {
        Self {
            registry: AgentRegistry::new(event_tx),
            ui: UiDispatcher::new(),
        }
    }

    /// Create a no-op Hub (for tests that don't need real connections).
    pub fn noop() -> Self {
        let (tx, _rx) = mpsc::channel(1);
        Self {
            registry: AgentRegistry::new(tx),
            ui: UiDispatcher::new(),
        }
    }
}
