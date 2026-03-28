//! Connection types for AgentHub.
//!
//! In Hub-only gateway architecture, all agents connect via stdio (managed
//! by Hub) and all clients connect via TCP. No agent-level TCP listeners.

use std::sync::Arc;

use tokio::sync::mpsc;

use loopal_ipc::connection::Connection;
use loopal_protocol::{ControlCommand, Envelope, InterruptSignal, UserQuestionResponse};

use crate::topology::AgentInfo;

/// Connection state for a managed agent or client.
pub(crate) enum AgentConnectionState {
    /// In-process channels (for unit tests — no real Hub).
    Local(LocalChannels),
    /// Hub-mode: uniform IPC connection (agents via stdio, clients via TCP).
    Connected(Arc<Connection>),
}

impl AgentConnectionState {
    /// Extract the IPC Connection if available.
    pub(crate) fn connection(&self) -> Option<Arc<Connection>> {
        match self {
            Self::Connected(conn) => Some(Arc::clone(conn)),
            Self::Local(_) => None,
        }
    }
}

/// In-process channel bundle — used by tests and local-mode SessionController.
pub struct LocalChannels {
    pub control_tx: mpsc::Sender<ControlCommand>,
    pub permission_tx: mpsc::Sender<bool>,
    pub question_tx: mpsc::Sender<UserQuestionResponse>,
    pub mailbox_tx: Option<mpsc::Sender<Envelope>>,
    pub interrupt: InterruptSignal,
    pub interrupt_tx: Arc<tokio::sync::watch::Sender<u64>>,
}

/// Internal wrapper for an agent/client entry in the hub.
pub(crate) struct ManagedAgent {
    pub(crate) state: AgentConnectionState,
    pub(crate) info: AgentInfo,
}
