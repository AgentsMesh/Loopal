pub mod cancel;
pub mod diff_tracker;
pub mod env_context;
mod input;
mod llm;
mod llm_record;
pub(crate) mod llm_result;
pub mod loop_detector;
mod middleware;
pub(crate) mod model_config;
mod permission;
mod preflight;
pub mod rewind;
mod run;
mod runner;
pub(crate) mod token_accumulator;
pub(crate) mod tool_exec;
mod tools;
mod tools_util;
pub mod turn_context;
mod turn_exec;
pub mod turn_observer;

use std::collections::HashSet;
use std::sync::Arc;

use crate::frontend::traits::AgentFrontend;
use loopal_context::ContextPipeline;
use loopal_error::{AgentOutput, Result};
use loopal_kernel::Kernel;
use loopal_message::Message;
use loopal_protocol::InterruptSignal;
use loopal_provider_api::ThinkingConfig;
use loopal_storage::Session;
use loopal_tool_api::{MemoryChannel, PermissionMode};
use tokio::sync::Notify;

use crate::mode::AgentMode;
use crate::session::SessionManager;

pub use runner::AgentLoopRunner;

/// Maximum number of automatic continuations when LLM hits max_tokens.
pub(crate) const MAX_AUTO_CONTINUATIONS: u32 = 3;

/// Number of recent messages to keep when user triggers `/compact`.
pub(crate) const COMPACT_KEEP_LAST: usize = 10;

pub struct AgentLoopParams {
    pub kernel: Arc<Kernel>,
    pub session: Session,
    pub messages: Vec<Message>,
    pub model: String,
    /// Model for compaction/summarization. None = use main model.
    pub compact_model: Option<String>,
    pub system_prompt: String,
    pub mode: AgentMode,
    pub permission_mode: PermissionMode,
    pub max_turns: u32,
    pub frontend: Arc<dyn AgentFrontend>,
    pub session_manager: SessionManager,
    pub context_pipeline: ContextPipeline,
    /// Tool whitelist filter — if `Some`, only tools in this set are exposed to LLM.
    pub tool_filter: Option<HashSet<String>>,
    /// Opaque shared state forwarded to ToolContext for agent tool access.
    pub shared: Option<Arc<dyn std::any::Any + Send + Sync>>,
    /// Whether this agent waits for user input between turns.
    /// `true` for root agent (TUI interaction), `false` for sub-agents (exit on no tool calls).
    pub interactive: bool,
    /// Thinking/reasoning configuration (default: Auto).
    pub thinking_config: ThinkingConfig,
    /// Shared interrupt signal — TUI sets it on ESC or message-while-busy.
    pub interrupt: InterruptSignal,
    /// Async wakeup companion for `interrupt` — allows `tokio::select!` responsiveness.
    pub interrupt_notify: Arc<Notify>,
    /// Memory channel for the Memory tool → Observer sidebar.
    pub memory_channel: Option<Arc<dyn MemoryChannel>>,
}

/// Public wrapper function that preserves the existing API.
/// Constructs default observers (loop detection, diff tracking) and runs the loop.
pub async fn agent_loop(params: AgentLoopParams) -> Result<AgentOutput> {
    let observers: Vec<Box<dyn turn_observer::TurnObserver>> = vec![
        Box::new(loop_detector::LoopDetector::new()),
        Box::new(diff_tracker::DiffTracker::new(params.frontend.clone())),
    ];
    let mut runner = AgentLoopRunner::new(params);
    runner.observers = observers;
    runner.run().await
}

/// Output from a single turn (LLM → [tools → LLM]* → done).
pub(crate) struct TurnOutput {
    /// The final assistant text of this turn.
    pub output: String,
}

/// Compact messages by keeping only the most recent `keep_last` messages.
pub(crate) fn compact_messages(messages: &mut Vec<Message>, keep_last: usize) {
    if messages.len() > keep_last {
        let drain_end = messages.len() - keep_last;
        messages.drain(..drain_end);
    }
}

/// Result of waiting for user input.
///
/// `wait_for_input` handles control commands (clear, compact, mode switch,
/// rewind, etc.) internally — only a real user message exits the wait.
pub enum WaitResult {
    /// A user message was added to the conversation
    MessageAdded,
}
