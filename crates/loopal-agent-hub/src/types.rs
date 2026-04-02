//! Connection types for Hub.
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
    /// Shadow entry for a remote agent spawned on another Hub via MetaHub.
    /// No real connection — only a placeholder so wait_agent and completion work.
    Shadow,
}

impl AgentConnectionState {
    /// Extract the IPC Connection if available.
    pub(crate) fn connection(&self) -> Option<Arc<Connection>> {
        match self {
            Self::Connected(conn) => Some(Arc::clone(conn)),
            Self::Local(_) | Self::Shadow => None,
        }
    }

    /// Whether this is a shadow entry (remote agent placeholder).
    pub(crate) fn is_shadow(&self) -> bool {
        matches!(self, Self::Shadow)
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
    /// Channel for delivering sub-agent completion notifications to this agent.
    /// When a child of this agent finishes, Hub sends an Envelope here.
    /// None for agents that don't spawn children (or weren't given a channel).
    pub(crate) completion_tx: Option<mpsc::Sender<Envelope>>,
}
