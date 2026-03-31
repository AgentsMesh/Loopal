pub mod cancel;
mod compaction;
mod context_prep;
pub mod diff_tracker;
pub mod env_context;
mod finished_guard;
mod input;
mod llm;
mod llm_params;
mod llm_record;
pub(crate) mod llm_result;
mod llm_retry;
pub mod loop_detector;
pub(crate) mod message_build;
pub(crate) mod model_config;
mod permission;
mod question_parse;
pub mod rewind;
mod run;
mod runner;
pub(crate) mod token_accumulator;
mod tool_collect;
pub(crate) mod tool_exec;
mod tool_progress;
mod tools;
mod tools_check;
mod tools_inject;
mod tools_resolve;
pub mod turn_context;
mod turn_exec;
pub mod turn_observer;

use std::collections::HashSet;
use std::sync::Arc;

use crate::frontend::traits::AgentFrontend;
use loopal_context::ContextStore;
use loopal_error::{AgentOutput, Result};
use loopal_kernel::Kernel;
use loopal_protocol::InterruptSignal;
use loopal_provider_api::{ModelRouter, ThinkingConfig};
use loopal_storage::Session;
use loopal_tool_api::{MemoryChannel, PermissionMode};
use tokio::sync::watch;

use crate::mode::AgentMode;
use crate::session::SessionManager;

use finished_guard::FinishedGuard;

pub use runner::AgentLoopRunner;

/// Maximum number of automatic continuations when LLM hits max_tokens.
pub(crate) const MAX_AUTO_CONTINUATIONS: u32 = 3;

// ── Sub-structs ────────────────────────────────────────────────────

/// Agent configuration — mostly immutable, some fields switchable at runtime.
pub struct AgentConfig {
    pub router: ModelRouter,
    pub system_prompt: String,
    pub mode: AgentMode,
    pub permission_mode: PermissionMode,
    pub max_turns: u32,
    /// Tool whitelist filter — if `Some`, only tools in this set are exposed.
    pub tool_filter: Option<HashSet<String>>,
    /// Thinking/reasoning configuration (default: Auto).
    pub thinking_config: ThinkingConfig,
    /// Context tokens cap from settings (0 = auto, use model's context_window).
    pub context_tokens_cap: u32,
}

impl AgentConfig {
    /// The effective main conversation model (respects model_routing.default override).
    pub fn model(&self) -> &str {
        self.router.resolve(loopal_provider_api::TaskType::Default)
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            router: ModelRouter::new("claude-sonnet-4-20250514".into()),
            system_prompt: String::new(),
            mode: AgentMode::Act,
            permission_mode: PermissionMode::Bypass,
            max_turns: 50,
            tool_filter: None,
            thinking_config: ThinkingConfig::Auto,
            context_tokens_cap: 0,
        }
    }
}

/// Injected dependencies — set once at construction, never modified.
pub struct AgentDeps {
    pub kernel: Arc<Kernel>,
    pub frontend: Arc<dyn AgentFrontend>,
    pub session_manager: SessionManager,
}

/// Interrupt/cancellation signals shared with the consumer.
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

// ── AgentLoopParams ────────────────────────────────────────────────

pub struct AgentLoopParams {
    pub config: AgentConfig,
    pub deps: AgentDeps,
    pub session: Session,
    pub store: ContextStore,
    pub interrupt: InterruptHandle,
    /// Opaque shared state forwarded to ToolContext for agent tool access.
    pub shared: Option<Arc<dyn std::any::Any + Send + Sync>>,
    /// Memory channel for the Memory tool → Observer sidebar.
    pub memory_channel: Option<Arc<dyn MemoryChannel>>,
    /// Receive end for scheduler-injected messages.
    /// When a cron job fires, an `Envelope` with `MessageSource::Scheduled`
    /// arrives here and is consumed by `wait_for_input()` alongside normal
    /// user input.
    pub scheduled_rx: Option<tokio::sync::mpsc::Receiver<loopal_protocol::Envelope>>,
    /// Auto-mode LLM classifier (active when permission_mode == Auto).
    pub auto_classifier: Option<Arc<loopal_auto_mode::AutoClassifier>>,
}

/// Public wrapper — constructs default observers and runs the loop.
///
/// A `FinishedGuard` ensures `Finished` is always emitted — even on panic.
pub async fn agent_loop(params: AgentLoopParams) -> Result<AgentOutput> {
    let mut guard = FinishedGuard::new(params.deps.frontend.clone());
    let observers: Vec<Box<dyn turn_observer::TurnObserver>> = vec![
        Box::new(loop_detector::LoopDetector::new()),
        Box::new(diff_tracker::DiffTracker::new(params.deps.frontend.clone())),
    ];
    let mut runner = AgentLoopRunner::new(params);
    runner.observers = observers;
    let result = runner.run().await;
    guard.disarm();
    result
}

/// Output from a single turn (LLM → [tools → LLM]* → done).
pub(crate) struct TurnOutput {
    /// The final assistant text of this turn.
    pub output: String,
}

/// Result of waiting for user input.
pub enum WaitResult {
    /// A user message was added to the conversation
    MessageAdded,
}
