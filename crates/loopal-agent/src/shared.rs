use std::path::PathBuf;
use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use loopal_ipc::connection::Connection;
use loopal_kernel::Kernel;
use loopal_protocol::{AgentEvent, Envelope, MessageSource};
use loopal_scheduler::CronScheduler;

use crate::task_store::TaskStore;

/// Handle to the per-agent cron scheduler.
///
/// Owns the `CronScheduler` (for tool access) and the `CancellationToken`
/// (to stop the tick loop on drop).
pub struct SchedulerHandle {
    pub scheduler: Arc<CronScheduler>,
    cancel: CancellationToken,
}

impl SchedulerHandle {
    /// Create a fully wired scheduler pipeline.
    ///
    /// Starts the tick loop and an adapter task that converts
    /// `ScheduledTrigger` → `Envelope` for the agent loop.
    /// Returns the handle (for tools) and a receiver (for `AgentLoopParams`).
    pub fn create() -> (Self, tokio::sync::mpsc::Receiver<Envelope>) {
        Self::create_with_scheduler(Arc::new(CronScheduler::new()))
    }

    /// Create with a custom `CronScheduler` (e.g., one using `ManualClock`).
    pub fn create_with_scheduler(
        scheduler: Arc<CronScheduler>,
    ) -> (Self, tokio::sync::mpsc::Receiver<Envelope>) {
        let cancel = CancellationToken::new();
        let (trigger_tx, mut trigger_rx) = tokio::sync::mpsc::channel(16);
        scheduler.start(trigger_tx, cancel.clone());

        let (env_tx, env_rx) = tokio::sync::mpsc::channel(16);
        tokio::spawn(async move {
            while let Some(t) = trigger_rx.recv().await {
                let env = Envelope::new(MessageSource::Scheduled, "self", t.prompt);
                if env_tx.send(env).await.is_err() {
                    break;
                }
            }
        });

        (Self { scheduler, cancel }, env_rx)
    }

    /// Create a handle without starting the pipeline (for tests that
    /// manage the scheduler lifecycle manually).
    pub fn new(scheduler: Arc<CronScheduler>, cancel: CancellationToken) -> Self {
        Self { scheduler, cancel }
    }
}

impl Drop for SchedulerHandle {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

/// Shared runtime context accessible by all agent tools via `ToolContext.shared`.
///
/// Main agent and sub-agents are **homogeneous**: both hold an `AgentShared`
/// with the same kernel and task_store references.
/// Sub-agents differ only in `depth`, `agent_name`, and `cancel_token`.
pub struct AgentShared {
    pub kernel: Arc<Kernel>,
    pub task_store: Arc<TaskStore>,
    /// Connection to Hub for `hub/*` IPC requests (route, broadcast, channels).
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
    /// Per-agent cron scheduler (tick loop cancelled on drop).
    pub scheduler_handle: SchedulerHandle,
}
