//! MetaHub — top-level composition of all subsystems.
//!
//! Cluster coordinator. Owns HubRegistry + GlobalRouter only.
//! No UI clients connect to MetaHub; no event aggregation across hubs
//! is wired up. Cross-hub permission/question is not supported (see
//! `pending_relay::handle_agent_permission`'s "no UI fast deny" path).

use crate::hub_registry::HubRegistry;
use crate::router::GlobalRouter;

pub struct MetaHub {
    pub registry: HubRegistry,
    pub router: GlobalRouter,
}

impl MetaHub {
    pub fn new() -> Self {
        Self {
            registry: HubRegistry::new(),
            router: GlobalRouter::new(),
        }
    }

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
