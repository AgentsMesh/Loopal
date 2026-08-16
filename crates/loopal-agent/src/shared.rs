use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use loopal_ipc::connection::{Connection, Listening};
use loopal_kernel::Kernel;
use loopal_protocol::{AgentEvent, AgentStateSnapshot, Envelope, MessageSource};
use loopal_provider_api::Message;
use loopal_runtime::GoalRuntimeSession;
use loopal_scheduler::{CronScheduler, ScheduledTrigger};

use crate::state_snapshot::{cron_info_to_snapshot, task_to_snapshot};
use crate::task_store::TaskStore;
use crate::types::TaskStatus;

/// Handle to the per-agent cron scheduler.
///
/// The tick loop does NOT start at construction. Callers must invoke
/// [`start`](SchedulerHandle::start) after binding the scheduler to a
/// session via `switch_session` so the first survey sees the loaded
/// task set rather than running empty.
pub struct SchedulerHandle {
    pub scheduler: Arc<CronScheduler>,
    cancel: CancellationToken,
    /// Trigger sender held until `start()` activates the tick loop.
    /// `None` after `start()` returns, or when constructed via `new`
    /// (which expects the caller to drive lifecycle manually).
    pending: std::sync::Mutex<Option<mpsc::Sender<ScheduledTrigger>>>,
}

impl SchedulerHandle {
    pub fn create() -> (Self, mpsc::Receiver<Envelope>) {
        Self::create_with_scheduler(Arc::new(CronScheduler::new()))
    }

    /// Build a handle wrapping `scheduler`. Spawns the trigger→Envelope
    /// adapter so the returned receiver is wired immediately, but the
    /// cron tick loop stays idle until `start()` is called.
    pub fn create_with_scheduler(
        scheduler: Arc<CronScheduler>,
    ) -> (Self, mpsc::Receiver<Envelope>) {
        let cancel = CancellationToken::new();
        let (trigger_tx, mut trigger_rx) = mpsc::channel::<ScheduledTrigger>(16);
        let (env_tx, env_rx) = mpsc::channel(16);
        tokio::spawn(async move {
            while let Some(t) = trigger_rx.recv().await {
                let env = Envelope::new(MessageSource::Scheduled, "self", t.prompt);
                if env_tx.send(env).await.is_err() {
                    break;
                }
            }
        });

        (
            Self {
                scheduler,
                cancel,
                pending: std::sync::Mutex::new(Some(trigger_tx)),
            },
            env_rx,
        )
    }

    /// Start the cron tick loop. Idempotent at this layer: the second
    /// call observes `pending == None` and returns silently. The
    /// underlying `CronScheduler::start` still asserts single-start —
    /// this wrapper's `pending.take()` ensures that contract is only
    /// reached once even when `start()` is invoked repeatedly.
    pub fn start(&self) {
        let tx = match self.pending.lock().unwrap().take() {
            Some(tx) => tx,
            None => return,
        };
        self.scheduler.start(tx, self.cancel.clone());
    }

    /// Construct without the adapter pipeline — for tests that drive
    /// the scheduler lifecycle directly.
    pub fn new(scheduler: Arc<CronScheduler>, cancel: CancellationToken) -> Self {
        Self {
            scheduler,
            cancel,
            pending: std::sync::Mutex::new(None),
        }
    }
}

impl Drop for SchedulerHandle {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

/// Shared runtime context accessible by all agent tools via `ToolContext.shared`.
///
/// Each agent runs in its own OS process, so every `AgentShared` owns its
/// own `Kernel` (and therefore its own `BackgroundTaskStore`), `TaskStore`,
/// and `SchedulerHandle`. There is no cross-agent sharing of these stores —
/// sub-agent state is fully isolated from the parent.
///
/// Sub-agents only differ from the root agent in `depth`, `agent_name`,
/// `cancel_token`, and the file-vs-memory backing of their session
/// storage (sub-agents use in-memory; root uses file-backed).
pub struct AgentShared {
    pub kernel: Arc<Kernel>,
    pub task_store: Arc<TaskStore>,
    /// Connection to Hub for `hub/*` IPC requests (route, broadcast, channels).
    pub hub_connection: Arc<Connection<Listening>>,
    /// Initial working directory. Immutable after construction.
    pub cwd: PathBuf,
    /// Current nesting depth (0 = root agent). Propagated via IPC so agents
    /// can sense how deep they are in the delegation chain.
    pub depth: u32,
    /// Name of the current agent.
    pub agent_name: String,
    /// Event sender for forwarding sub-agent events up the chain.
    pub parent_event_tx: Option<tokio::sync::mpsc::Sender<AgentEvent>>,
    /// This agent's own cancellation token.
    pub cancel_token: Option<CancellationToken>,
    /// Per-agent cron scheduler (tick loop cancelled on drop).
    pub scheduler_handle: SchedulerHandle,
    /// Conversation snapshot updated by the runtime before each tool batch.
    /// The Agent tool reads this to build fork context for child agents.
    pub message_snapshot: Arc<RwLock<Vec<Message>>>,
    /// Persistent goal session for this agent. None when the goal feature
    /// is disabled in settings. Shared with the runtime so multi-UI clients
    /// see the same single source of truth.
    pub goal_session: Option<Arc<GoalRuntimeSession>>,
    /// Narrow root-only capability for Hub-owned workflow control.
    pub workflow_control: Option<Arc<dyn crate::workflow_control::WorkflowControlClient>>,
}

impl AgentShared {
    /// Aggregate the agent's observable state for `agent/state_snapshot` IPC.
    ///
    /// Returns task list (active only — same filter as `task_bridge`),
    /// scheduled cron jobs, and running background processes.
    /// Token usage / mode / status are intentionally absent: the Hub-side
    /// reducer accumulates those from the live event stream, so including
    /// them here would only duplicate fact and risk drift.
    pub async fn snapshot_state(&self) -> AgentStateSnapshot {
        let tasks = self
            .task_store
            .list()
            .await
            .into_iter()
            .filter(|t| !matches!(t.status, TaskStatus::Completed))
            .map(|t| task_to_snapshot(&t))
            .collect();
        let crons = self
            .scheduler_handle
            .scheduler
            .list()
            .await
            .into_iter()
            .map(cron_info_to_snapshot)
            .collect();
        let bg_tasks = self
            .kernel
            .bg_store()
            .snapshot(loopal_tool_background::StatusFilter::All);
        let thread_goal = match self.goal_session.as_ref() {
            Some(s) => s.snapshot().await.ok().flatten(),
            None => None,
        };
        AgentStateSnapshot {
            tasks,
            crons,
            bg_tasks,
            thread_goal,
            workflows: Default::default(),
        }
    }

    /// Whether child agents should inherit a disabled sandbox. Read from
    /// startup `settings()` so spawn inheritance stays uniform with
    /// `permission_mode` / `decision_mode` / `model` (all startup-sourced).
    /// The parent's OWN tool enforcement tracks the live policy separately
    /// via `kernel.create_backend` — runtime `/sandbox` switches are
    /// parent-local and intentionally do not propagate to new children.
    pub fn no_sandbox(&self) -> bool {
        matches!(
            self.kernel.settings().sandbox.policy,
            loopal_config::SandboxPolicy::Disabled
        )
    }
}
