//! Managed hub — wraps a sub-hub connection with its metadata.

use std::sync::Arc;

use loopal_ipc::connection::Connection;

use crate::hub_info::HubInfo;

/// A connected sub-hub tracked by the MetaHub.
///
/// Owns the IPC connection and associated metadata.
/// Analogous to `ManagedAgent` in `loopal-agent-hub`.
pub struct ManagedHub {
    /// IPC connection to the sub-hub (TCP).
    pub(crate) conn: Arc<Connection>,
    /// Metadata snapshot (name, status, capabilities, etc.).
    pub(crate) info: HubInfo,
}

impl ManagedHub {
    /// Create a new managed hub entry.
    pub fn new(conn: Arc<Connection>, info: HubInfo) -> Self {
        Self { conn, info }
    }

    /// Get the connection to this sub-hub.
    pub fn connection(&self) -> &Arc<Connection> {
        &self.conn
    }

    /// Get metadata (read-only).
    pub fn info(&self) -> &HubInfo {
        &self.info
    }

    /// Get mutable metadata (for heartbeat updates).
    pub fn info_mut(&mut self) -> &mut HubInfo {
        &mut self.info
    }
}
