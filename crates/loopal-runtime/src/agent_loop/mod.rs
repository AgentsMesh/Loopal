mod input;
mod llm;
mod llm_record;
mod middleware;
mod permission;
mod preflight;
pub mod rewind;
mod run;
mod runner;
pub(crate) mod tool_exec;
mod tools;

use std::collections::HashSet;
use std::sync::Arc;

use loopal_context::ContextPipeline;
use loopal_kernel::Kernel;
use loopal_storage::Session;
use loopal_error::{AgentOutput, Result};
use crate::frontend::traits::AgentFrontend;
use loopal_message::Message;
use loopal_tool_api::PermissionMode;

use crate::mode::AgentMode;
use crate::session::SessionManager;

pub use runner::AgentLoopRunner;

/// Maximum number of automatic continuations when LLM hits max_tokens.
pub(crate) const MAX_AUTO_CONTINUATIONS: u32 = 3;

pub struct AgentLoopParams {
    pub kernel: Arc<Kernel>,
    pub session: Session,
    pub messages: Vec<Message>,
    pub model: String,
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
}

/// Public wrapper function that preserves the existing API.
/// Returns structured AgentOutput with result text and termination reason.
pub async fn agent_loop(params: AgentLoopParams) -> Result<AgentOutput> {
    let mut runner = AgentLoopRunner::new(params);
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
