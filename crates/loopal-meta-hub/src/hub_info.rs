//! Hub info — metadata snapshot for a connected sub-hub.

use std::time::Instant;

/// Health status of a connected sub-hub.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HubStatus {
    /// Healthy — heartbeat received within expected interval.
    Connected,
    /// Heartbeat delayed beyond threshold but not yet timed out.
    Degraded,
    /// Connection lost or heartbeat timed out.
    Disconnected,
}

/// Metadata for a single sub-hub known to the MetaHub.
///
/// This is a snapshot — does not hold the connection itself.
/// Connection is owned by `ManagedHub`.
#[derive(Debug, Clone)]
pub struct HubInfo {
    /// Unique hub name (assigned by the sub-hub on registration).
    pub name: String,
    /// Current health status.
    pub status: HubStatus,
    /// Number of agents currently managed by this sub-hub.
    pub agent_count: usize,
    /// Optional capability tags declared by the sub-hub.
    pub capabilities: Vec<String>,
    /// When this sub-hub first connected.
    pub connected_at: Instant,
    /// When the last heartbeat was received.
    pub last_heartbeat: Instant,
}

impl HubInfo {
    /// Create info for a newly connected sub-hub.
    pub fn new(name: &str, capabilities: Vec<String>) -> Self {
        let now = Instant::now();
        Self {
            name: name.to_string(),
            status: HubStatus::Connected,
            agent_count: 0,
            capabilities,
            connected_at: now,
            last_heartbeat: now,
        }
    }

    /// Update heartbeat timestamp and agent count.
    pub fn heartbeat(&mut self, agent_count: usize) {
        self.last_heartbeat = Instant::now();
        self.agent_count = agent_count;
        self.status = HubStatus::Connected;
    }

    /// Mark as degraded (heartbeat delayed).
    pub fn mark_degraded(&mut self) {
        self.status = HubStatus::Degraded;
    }

    /// Mark as disconnected.
    pub fn mark_disconnected(&mut self) {
        self.status = HubStatus::Disconnected;
    }

    /// Whether this hub is alive (Connected or Degraded).
    pub fn is_alive(&self) -> bool {
        matches!(self.status, HubStatus::Connected | HubStatus::Degraded)
    }
}
