mod llm;
mod middleware;
mod permission;
mod runner;
mod tools;

use std::sync::Arc;

use loopagent_context::ContextPipeline;
use loopagent_kernel::Kernel;
use loopagent_storage::Session;
use loopagent_types::command::UserCommand;
use loopagent_types::error::Result;
use loopagent_types::event::AgentEvent;
use loopagent_types::message::Message;
use loopagent_types::permission::PermissionMode;
use tokio::sync::mpsc;

use crate::mode::AgentMode;
use crate::session::SessionManager;

pub(crate) use runner::AgentLoopRunner;

pub struct AgentLoopParams {
    pub kernel: Arc<Kernel>,
    pub session: Session,
    pub messages: Vec<Message>,
    pub model: String,
    pub system_prompt: String,
    pub mode: AgentMode,
    pub permission_mode: PermissionMode,
    pub max_turns: u32,
    pub event_tx: mpsc::Sender<AgentEvent>,
    pub input_rx: mpsc::Receiver<UserCommand>,
    pub permission_rx: mpsc::Receiver<bool>,
    pub session_manager: SessionManager,
    pub context_pipeline: ContextPipeline,
}

/// Public wrapper function that preserves the existing API.
pub async fn agent_loop(params: AgentLoopParams) -> Result<()> {
    let mut runner = AgentLoopRunner::new(params);
    runner.run().await
}

/// Compact messages by keeping only the most recent `keep_last` messages.
pub(crate) fn compact_messages(messages: &mut Vec<Message>, keep_last: usize) {
    if messages.len() > keep_last {
        let drain_end = messages.len() - keep_last;
        messages.drain(..drain_end);
    }
}

/// Result of waiting for user input
pub(crate) enum WaitResult {
    /// A mode switch occurred — caller should `continue` without consuming a turn
    Continue,
    /// A user message was added to the conversation
    MessageAdded,
}
