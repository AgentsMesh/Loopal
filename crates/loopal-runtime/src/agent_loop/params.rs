use std::collections::HashSet;
use std::sync::Arc;

use loopal_config::HarnessConfig;
use loopal_context::ContextBudget;
use loopal_kernel::Kernel;
use loopal_protocol::InterruptSignal;
use loopal_provider_api::{SharedModelRouter, ThinkingConfig};
use loopal_storage::Session;
use loopal_tool_api::{FetchRefinerPolicy, MemoryChannel, OneShotChatService, PermissionMode};
use tokio::sync::watch;

use crate::frontend::DecisionCell;
use crate::frontend::DecisionContext;
use crate::frontend::traits::AgentFrontend;
use crate::mode::AgentMode;
use crate::session::SessionManager;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LifecycleMode {
    #[default]
    Persistent,
    Ephemeral,
}

pub struct AgentConfig {
    pub lifecycle: LifecycleMode,
    pub router: SharedModelRouter,
    pub system_prompt: String,
    pub mode: AgentMode,
    pub permission_mode: PermissionMode,
    pub tool_filter: Option<HashSet<String>>,
    pub thinking_config: ThinkingConfig,
    pub context_tokens_cap: u32,
    pub microcompact_idle: std::time::Duration,
    pub plan_state: Option<PlanModeState>,
}

pub struct PlanModeState {
    pub previous_mode: AgentMode,
    // reason: snapshot of permission_mode taken on EnterPlanMode so we can
    // restore it on exit. DecisionMode is not tracked by the runtime — it
    // lives only in the frontend handler chain (Manual vs Auto wraps).
    pub previous_permission_mode: PermissionMode,
    pub tool_filter: HashSet<String>,
}

impl AgentConfig {
    pub fn model(&self) -> String {
        self.router.model()
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            lifecycle: LifecycleMode::default(),
            // reason: test-only fixture (production sets the real router in
            // agent_setup). Pinned to a specific model so token-math tests stay
            // calibrated — NOT coupled to the production default model.
            router: SharedModelRouter::with_default("claude-sonnet-4-20250514".into()),
            system_prompt: String::new(),
            mode: AgentMode::Act,
            permission_mode: PermissionMode::Bypass,
            tool_filter: None,
            thinking_config: ThinkingConfig::Auto,
            context_tokens_cap: 0,
            microcompact_idle: std::time::Duration::from_secs(60 * 60),
            plan_state: None,
        }
    }
}

pub struct AgentDeps {
    pub kernel: Arc<Kernel>,
    pub frontend: Arc<dyn AgentFrontend>,
    pub session_manager: SessionManager,
    pub decision_context: DecisionContext,
}

pub struct InterruptHandle {
    pub signal: InterruptSignal,
    pub tx: Arc<watch::Sender<u64>>,
}

impl InterruptHandle {
    pub fn new() -> Self {
        Self {
            signal: InterruptSignal::new(),
            tx: Arc::new(watch::channel(0u64).0),
        }
    }
}

impl Default for InterruptHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// Parameters for the agent loop.
///
/// Use [`AgentLoopParamsBuilder`](crate::agent_loop::AgentLoopParamsBuilder)
/// for construction — the struct is `#[non_exhaustive]` so external
/// callers cannot use struct-literal init. New optional fields are
/// added without breaking existing call sites because all defaults
/// live in the builder.
///
/// `pub` fields stay readable for ergonomic field access on already-
/// built instances; only construction is gated.
#[non_exhaustive]
pub struct AgentLoopParams {
    pub config: AgentConfig,
    pub deps: AgentDeps,
    pub session: Session,
    pub budget: ContextBudget,
    pub initial_turns: Vec<loopal_turn::Turn>,
    pub interrupt: InterruptHandle,
    pub shared: Option<Arc<dyn std::any::Any + Send + Sync>>,
    pub memory_channel: Option<Arc<dyn MemoryChannel>>,
    pub one_shot_chat: Option<Arc<dyn OneShotChatService>>,
    pub fetch_refiner_policy: Option<Arc<dyn FetchRefinerPolicy>>,
    pub goal_session: Option<Arc<crate::goal::GoalRuntimeSession>>,
    pub scheduled_rx: Option<tokio::sync::mpsc::Receiver<loopal_protocol::Envelope>>,
    pub harness: HarnessConfig,
    pub rewake_rx: Option<tokio::sync::mpsc::Receiver<loopal_protocol::Envelope>>,
    pub message_snapshot: Option<Arc<std::sync::RwLock<Vec<loopal_provider_api::Message>>>>,
    pub scheduler: Option<Arc<loopal_scheduler::CronScheduler>>,
    /// Hooks invoked after `handle_resume_session` swaps the active
    /// session, so per-session state (cron, task list, etc.) can follow.
    /// Default is empty — runtime callers that don't supply hooks see no
    /// behavioral change.
    pub resume_hooks: Vec<Arc<dyn crate::session_resume_hook::SessionResumeHook>>,
    /// Runtime-mutable decision mode shared with the classifier handlers.
    /// `DecisionModeSwitch` writes it; defaults to `DecisionMode::default()`
    /// (Manual) for callers that don't wire it to their handler chain.
    pub decision_cell: DecisionCell,
}

impl AgentLoopParams {
    pub fn session(&self) -> &Session {
        &self.session
    }
    pub fn config(&self) -> &AgentConfig {
        &self.config
    }
}
