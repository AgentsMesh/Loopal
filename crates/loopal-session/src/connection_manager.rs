//! Unified agent connection manager for multi-agent TUI.
//!
//! Manages connections to all agent servers (root + sub-agents).
//! Root agent connects via stdio Bridge (PrimaryConn), sub-agents
//! connect via TCP (AttachedConn). Supports attach/detach lifecycle.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use loopal_ipc::connection::Connection;
use loopal_protocol::{
    AgentEvent, ControlCommand, Envelope, InterruptSignal, UserQuestionResponse,
};

/// Manages all agent connections: root (Primary) and sub-agents (Attached/Detached).
pub struct AgentConnectionManager {
    pub(crate) agents: HashMap<String, ManagedAgent>,
    pub(crate) event_tx: mpsc::Sender<AgentEvent>,
}

pub(crate) struct ManagedAgent {
    pub(crate) state: AgentConnectionState,
}

/// Connection state for a managed agent.
pub enum AgentConnectionState {
    /// Root agent — full bidirectional control via IPC Bridge.
    Primary(PrimaryConn),
    /// Sub-agent — observing events via TCP.
    Attached(AttachedConn),
    /// Disconnected but agent still alive — can re-attach.
    Detached { port: u16, token: String },
}

/// Root agent connection — full bidirectional control.
pub struct PrimaryConn {
    pub control_tx: mpsc::Sender<ControlCommand>,
    pub permission_tx: mpsc::Sender<bool>,
    pub question_tx: mpsc::Sender<UserQuestionResponse>,
    pub mailbox_tx: Option<mpsc::Sender<Envelope>>,
    pub interrupt: InterruptSignal,
    pub interrupt_tx: Arc<tokio::sync::watch::Sender<u64>>,
}

/// Sub-agent TCP observer connection.
pub struct AttachedConn {
    pub(crate) connection: Arc<Connection>,
    pub(crate) event_task: JoinHandle<()>,
    pub port: u16,
    pub token: String,
}

impl AgentConnectionManager {
    pub fn new(event_tx: mpsc::Sender<AgentEvent>) -> Self {
        Self {
            agents: HashMap::new(),
            event_tx,
        }
    }

    /// Create a no-op manager (for tests).
    pub fn noop() -> Self {
        let (tx, _rx) = mpsc::channel(1);
        Self {
            agents: HashMap::new(),
            event_tx: tx,
        }
    }

    /// Register the root agent (called once at bootstrap).
    pub fn set_primary(&mut self, name: &str, conn: PrimaryConn) {
        self.agents.insert(
            name.to_string(),
            ManagedAgent {
                state: AgentConnectionState::Primary(conn),
            },
        );
    }

    /// Get the primary (root) agent connection for sending user input.
    pub fn primary(&self) -> Option<(&str, &PrimaryConn)> {
        self.agents.iter().find_map(|(name, agent)| {
            if let AgentConnectionState::Primary(conn) = &agent.state {
                Some((name.as_str(), conn))
            } else {
                None
            }
        })
    }
}
