use std::collections::HashSet;
use std::sync::Arc;

use loopal_config::HarnessConfig;
use loopal_context::ContextStore;
use loopal_kernel::Kernel;
use loopal_protocol::InterruptSignal;
use loopal_provider_api::{ModelRouter, ThinkingConfig};
use loopal_storage::Session;
use loopal_tool_api::{FetchRefinerPolicy, MemoryChannel, OneShotChatService, PermissionMode};
use tokio::sync::watch;

use crate::frontend::traits::AgentFrontend;
use crate::mode::AgentMode;
use crate::session::SessionManager;

/// Agent lifecycle mode — determines idle behavior after turn completion.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LifecycleMode {
    #[default]
    Persistent,
    Ephemeral,
}

/// Agent configuration — mostly immutable, some fields switchable at runtime.
pub struct AgentConfig {
    pub lifecycle: LifecycleMode,
    pub router: ModelRouter,
    pub system_prompt: String,
    pub mode: AgentMode,
    pub permission_mode: PermissionMode,
    pub tool_filter: Option<HashSet<String>>,
    pub thinking_config: ThinkingConfig,
    pub context_tokens_cap: u32,
    pub plan_state: Option<PlanModeState>,
}

pub struct PlanModeState {
    pub previous_mode: AgentMode,
    pub previous_permission_mode: PermissionMode,
    pub tool_filter: HashSet<String>,
}

impl AgentConfig {
    pub fn model(&self) -> &str {
        self.router.resolve(loopal_provider_api::TaskType::Default)
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            lifecycle: LifecycleMode::default(),
            router: ModelRouter::new("claude-sonnet-4-20250514".into()),
            system_prompt: String::new(),
            mode: AgentMode::Act,
            permission_mode: PermissionMode::Bypass,
            tool_filter: None,
            thinking_config: ThinkingConfig::Auto,
            context_tokens_cap: 0,
            plan_state: None,
        }
    }
}

pub struct AgentDeps {
    pub kernel: Arc<Kernel>,
    pub frontend: Arc<dyn AgentFrontend>,
    pub session_manager: SessionManager,
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
    pub store: ContextStore,
    pub interrupt: InterruptHandle,
    pub shared: Option<Arc<dyn std::any::Any + Send + Sync>>,
    pub memory_channel: Option<Arc<dyn MemoryChannel>>,
    pub one_shot_chat: Option<Arc<dyn OneShotChatService>>,
    pub fetch_refiner_policy: Option<Arc<dyn FetchRefinerPolicy>>,
    pub goal_session: Option<Arc<crate::goal::GoalRuntimeSession>>,
    pub scheduled_rx: Option<tokio::sync::mpsc::Receiver<loopal_protocol::Envelope>>,
    pub auto_classifier: Option<Arc<loopal_auto_mode::AutoClassifier>>,
    pub harness: HarnessConfig,
    pub rewake_rx: Option<tokio::sync::mpsc::Receiver<loopal_protocol::Envelope>>,
    pub message_snapshot: Option<Arc<std::sync::RwLock<Vec<loopal_message::Message>>>>,
    /// Hooks invoked after `handle_resume_session` swaps the active
    /// session, so per-session state (cron, task list, etc.) can follow.
    /// Default is empty — runtime callers that don't supply hooks see no
    /// behavioral change.
    pub resume_hooks: Vec<Arc<dyn crate::session_resume_hook::SessionResumeHook>>,
}

impl AgentLoopParams {
    pub fn session(&self) -> &Session {
        &self.session
    }
    pub fn config(&self) -> &AgentConfig {
        &self.config
    }
}
