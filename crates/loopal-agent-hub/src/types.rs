//! Connection types for multi-agent hub.
//!
//! Defines the connection states for root agent (Primary) and
//! sub-agents (Attached/Detached) managed by [`AgentHub`](crate::AgentHub).

use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use loopal_ipc::connection::Connection;
use loopal_protocol::{ControlCommand, Envelope, InterruptSignal, UserQuestionResponse};

/// Connection state for a managed agent.
pub(crate) enum AgentConnectionState {
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
pub(crate) struct AttachedConn {
    pub(crate) connection: Arc<Connection>,
    pub(crate) event_task: JoinHandle<()>,
    pub port: u16,
    pub token: String,
}

/// Internal wrapper for an agent entry in the hub.
pub(crate) struct ManagedAgent {
    pub(crate) state: AgentConnectionState,
}
