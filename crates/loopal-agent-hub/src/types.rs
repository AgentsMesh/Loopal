//! Connection types for Hub.
//!
//! In Hub-only gateway architecture, all agents connect via stdio (managed
//! by Hub) and all clients connect via TCP. No agent-level TCP listeners.

use std::sync::Arc;

use tokio::sync::{Mutex, mpsc};

use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::{
    AgentCompletion, ControlCommand, Envelope, InterruptSignal, UserQuestionResponse,
};
use loopal_view_state::ViewStateReducer;

use crate::topology::AgentInfo;

/// Connection state for a managed agent or client.
pub(crate) enum AgentConnectionState {
    /// In-process channels (for unit tests — no real Hub).
    Local(LocalChannels),
    /// Hub-mode: uniform IPC connection (agents via stdio, clients via TCP).
    Connected(Arc<Connection<Listening>>),
    /// Shadow entry for a remote agent spawned on another Hub via MetaHub.
    /// No real connection — only a placeholder so wait_agent and completion work.
    Shadow,
}

impl AgentConnectionState {
    /// Extract the IPC Connection if available.
    pub(crate) fn connection(&self) -> Option<Arc<Connection<Listening>>> {
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
    /// Hub-local generation of a local parent captured when this edge was
    /// registered. Names alone are not ownership: a reconnected same-name
    /// parent must not receive or cascade an older child's completion.
    pub(crate) parent_generation: Option<u64>,
    /// Channel for delivering sub-agent completion notifications to this agent.
    /// When a child of this agent finishes, Hub sends an Envelope here.
    /// None for agents that don't spawn children (or weren't given a channel).
    pub(crate) completion_tx: Option<mpsc::Sender<Envelope>>,
    pub(crate) notify_parent_on_completion: bool,
    /// Per-agent ViewState reducer. The Hub event router applies each
    /// incoming `AgentEvent` here so `view/snapshot` returns the latest
    /// observable state. UI clients subscribe to the existing
    /// `agent/event` broadcast for incremental updates and apply the
    /// same events locally — there is no separate `view/delta` channel.
    pub(crate) view: Arc<Mutex<ViewStateReducer>>,
    /// Authoritative terminal completion captured before the connection is
    /// detached. Kept here only during the short emit → unregister window.
    pub(crate) completion: Option<AgentCompletion>,
    /// Whether the latest admitted lifecycle event is an Error that has not
    /// been followed by Running/Started. This is admission state, independent
    /// of the asynchronously reduced topology projection.
    pub(crate) admitted_error: bool,
    /// Monotonic Hub-local identity for this same-name registration.
    pub(crate) generation: u64,
}

impl ManagedAgent {
    /// Build the ViewState reducer for a freshly registered agent.
    /// Starts empty (rev=0) and is later reseeded by event flow.
    pub(crate) fn new_view_reducer(agent_name: &str) -> Arc<Mutex<ViewStateReducer>> {
        Arc::new(Mutex::new(ViewStateReducer::new(agent_name)))
    }
}

/// Read-only state retained after an agent connection is detached.
/// Deliberately excludes connection and control channels.
pub(crate) struct CompletedAgent {
    pub(crate) info: AgentInfo,
    pub(crate) parent_generation: Option<u64>,
    pub(crate) completion: AgentCompletion,
    pub(crate) view: Arc<Mutex<ViewStateReducer>>,
    pub(crate) shadow: bool,
    pub(crate) generation: u64,
}
