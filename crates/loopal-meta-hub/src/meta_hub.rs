//! MetaHub — top-level composition of all subsystems.
//!
//! Thin coordinator that owns HubRegistry, GlobalRouter, EventAggregator,
//! and UiDispatcher. Delegates all actual work to the subsystems.

use crate::aggregator::EventAggregator;
use crate::hub_registry::HubRegistry;
use crate::router::GlobalRouter;

use loopal_agent_hub::UiDispatcher;

/// Central coordinator for a cluster of sub-hubs.
///
/// Analogous to `Hub` in `loopal-agent-hub`, but manages hubs instead of agents.
/// Does not hold any agent connections directly — all agent management stays
/// within individual sub-hubs.
pub struct MetaHub {
    /// Sub-hub connection lifecycle management.
    pub registry: HubRegistry,
    /// Cross-hub address resolution and message routing.
    pub router: GlobalRouter,
    /// Multi-hub event stream aggregation (push-based, no background tasks).
    pub aggregator: EventAggregator,
    /// UI client connections and event broadcast.
    pub ui: UiDispatcher,
}

impl MetaHub {
    /// Create a new MetaHub with empty subsystems.
    pub fn new() -> Self {
        Self {
            registry: HubRegistry::new(),
            router: GlobalRouter::new(),
            aggregator: EventAggregator::new(),
            ui: UiDispatcher::new(),
        }
    }

    /// Remove a sub-hub (disconnect cleanup).
    pub fn remove_hub(&mut self, hub_name: &str) {
        self.registry.unregister(hub_name);
        tracing::info!(hub = %hub_name, "sub-hub fully removed");
    }
}

impl Default for MetaHub {
    fn default() -> Self {
        Self::new()
    }
}
