//! Hub registry — manages connected sub-hub lifecycle.
//!
//! Analogous to `AgentRegistry` in `loopal-agent-hub`, but manages
//! Hub-level connections instead of individual agents.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::connection::Connection;

use crate::hub_info::{HubInfo, HubStatus};
use crate::managed_hub::ManagedHub;

/// Heartbeat timeout — mark as degraded after this duration.
const HEARTBEAT_DEGRADED_TIMEOUT: Duration = Duration::from_secs(30);
/// Heartbeat timeout — mark as disconnected after this duration.
const HEARTBEAT_DISCONNECT_TIMEOUT: Duration = Duration::from_secs(90);

/// Manages the set of connected sub-hubs.
pub struct HubRegistry {
    hubs: HashMap<String, ManagedHub>,
}

impl Default for HubRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl HubRegistry {
    pub fn new() -> Self {
        Self {
            hubs: HashMap::new(),
        }
    }

    /// Register a new sub-hub. Returns error if name already taken.
    pub fn register(
        &mut self,
        name: &str,
        conn: Arc<Connection>,
        capabilities: Vec<String>,
    ) -> Result<(), String> {
        if self.hubs.contains_key(name) {
            return Err(format!("hub '{name}' already registered"));
        }
        let info = HubInfo::new(name, capabilities);
        self.hubs
            .insert(name.to_string(), ManagedHub::new(conn, info));
        tracing::info!(hub = %name, "sub-hub registered");
        Ok(())
    }

    /// Unregister a sub-hub (on disconnect or shutdown).
    pub fn unregister(&mut self, name: &str) -> Option<ManagedHub> {
        let removed = self.hubs.remove(name);
        if removed.is_some() {
            tracing::info!(hub = %name, "sub-hub unregistered");
        }
        removed
    }

    /// Get a sub-hub by name.
    pub fn get(&self, name: &str) -> Option<&ManagedHub> {
        self.hubs.get(name)
    }

    /// Get the connection to a sub-hub by name.
    pub fn connection(&self, name: &str) -> Option<Arc<Connection>> {
        self.hubs.get(name).map(|h| Arc::clone(&h.conn))
    }

    /// Update heartbeat for a sub-hub.
    pub fn heartbeat(&mut self, name: &str, agent_count: usize) -> Result<(), String> {
        let hub = self
            .hubs
            .get_mut(name)
            .ok_or_else(|| format!("unknown hub '{name}'"))?;
        hub.info_mut().heartbeat(agent_count);
        Ok(())
    }

    /// List all registered hub names.
    pub fn hub_names(&self) -> Vec<String> {
        self.hubs.keys().cloned().collect()
    }

    /// List all alive (Connected or Degraded) hubs with their connections.
    pub fn alive_hubs(&self) -> Vec<(&str, &Arc<Connection>)> {
        self.hubs
            .iter()
            .filter(|(_, h)| h.info().is_alive())
            .map(|(name, h)| (name.as_str(), h.connection()))
            .collect()
    }

    /// Number of registered sub-hubs.
    pub fn len(&self) -> usize {
        self.hubs.len()
    }

    /// Whether no sub-hubs are registered.
    pub fn is_empty(&self) -> bool {
        self.hubs.is_empty()
    }

    /// Check all hubs for heartbeat timeout, update status accordingly.
    /// Returns names of hubs that transitioned to Disconnected.
    pub fn check_health(&mut self) -> Vec<String> {
        let mut disconnected = Vec::new();
        for (name, hub) in &mut self.hubs {
            let elapsed = hub.info().last_heartbeat.elapsed();
            match hub.info().status {
                HubStatus::Connected if elapsed > HEARTBEAT_DEGRADED_TIMEOUT => {
                    hub.info_mut().mark_degraded();
                    tracing::warn!(hub = %name, "sub-hub heartbeat degraded");
                }
                HubStatus::Degraded if elapsed > HEARTBEAT_DISCONNECT_TIMEOUT => {
                    hub.info_mut().mark_disconnected();
                    tracing::error!(hub = %name, "sub-hub heartbeat timed out");
                    disconnected.push(name.clone());
                }
                _ => {}
            }
        }
        disconnected
    }

    /// Get a snapshot of all hub info (for topology queries).
    pub fn snapshot(&self) -> Vec<HubInfo> {
        self.hubs.values().map(|h| h.info().clone()).collect()
    }
}
