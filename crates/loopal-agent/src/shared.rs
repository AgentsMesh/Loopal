use std::path::PathBuf;
use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use loopal_ipc::connection::Connection;
use loopal_kernel::Kernel;
use loopal_protocol::AgentEvent;

use crate::task_store::TaskStore;

/// Shared runtime context accessible by all agent tools via `ToolContext.shared`.
///
/// Main agent and sub-agents are **homogeneous**: both hold an `AgentShared`
/// with the same kernel and task_store references.
/// Sub-agents differ only in `depth`, `agent_name`, and `cancel_token`.
pub struct AgentShared {
    pub kernel: Arc<Kernel>,
    pub task_store: Arc<TaskStore>,
    /// Connection to AgentHub for `hub/*` IPC requests (route, broadcast, channels).
    pub hub_connection: Arc<Connection>,
    /// Initial working directory. Immutable after construction.
    pub cwd: PathBuf,
    /// Current nesting depth (0 = root agent).
    pub depth: u32,
    /// Maximum allowed nesting depth.
    pub max_depth: u32,
    /// Name of the current agent.
    pub agent_name: String,
    /// Event sender for forwarding sub-agent events up the chain.
    pub parent_event_tx: Option<tokio::sync::mpsc::Sender<AgentEvent>>,
    /// This agent's own cancellation token.
    pub cancel_token: Option<CancellationToken>,
}
