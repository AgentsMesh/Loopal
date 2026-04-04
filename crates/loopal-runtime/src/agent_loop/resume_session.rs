//! Session resume — hot-swap agent context to a different persisted session.

use loopal_context::ContextStore;
use loopal_error::Result;
use loopal_protocol::AgentEventPayload;
use tracing::info;

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Replace the agent's session context with a different persisted session.
    ///
    /// Loads the target session's metadata and messages from storage,
    /// replaces the in-memory context store, and resets per-session counters.
    pub(super) async fn handle_resume_session(&mut self, session_id: &str) -> Result<()> {
        info!(session_id, "resuming session");
        let (session, messages) = self
            .params
            .deps
            .session_manager
            .resume_session(session_id)?;

        // Replace session identity + conversation context
        self.params.session = session;
        self.params.store =
            ContextStore::from_messages(messages, self.params.store.budget().clone());

        // Reset per-session counters
        self.turn_count = 0;
        self.tokens.reset();

        // Update tool context so subsequent tool calls persist to the new session
        self.tool_ctx.session_id.clone_from(&self.params.session.id);

        // Notify frontend
        let message_count = self.params.store.len();
        self.emit(AgentEventPayload::SessionResumed {
            session_id: self.params.session.id.clone(),
            message_count,
        })
        .await?;

        // Reset token display
        self.emit(AgentEventPayload::TokenUsage {
            input_tokens: 0,
            output_tokens: 0,
            context_window: self.params.store.budget().context_window,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            thinking_tokens: 0,
        })
        .await?;

        Ok(())
    }
}
