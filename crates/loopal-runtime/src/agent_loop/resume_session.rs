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
    /// After the message-layer swap is committed, fans out to every
    /// registered [`SessionResumeHook`](crate::SessionResumeHook) so per-
    /// session resources (cron scheduler, task store, etc.) follow the
    /// agent across the resume.
    ///
    /// Public so integration tests can drive resume directly without
    /// going through the channel-select `wait_for_input` loop. The
    /// runtime itself calls this from `wait_for_input` on receiving
    /// `ControlCommand::ResumeSession`.
    pub async fn handle_resume_session(&mut self, session_id: &str) -> Result<()> {
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
        // Drop pending InboxConsumed ids belonging to the previous session —
        // emitting them after the swap would surface as ghost events on the
        // resumed conversation.
        self.pending_consumed_ids.clear();

        // Update tool context so subsequent tool calls persist to the new session
        self.tool_ctx.session_id.clone_from(&self.params.session.id);

        // Clear the shared message snapshot — the previous session's
        // entries must not leak into a sub-agent fork that happens
        // before the runner has had a chance to refresh the snapshot.
        // The snapshot is shared via Arc with `AgentShared`, so dropping
        // the inner Vec is sufficient.
        if let Some(snapshot) = self.params.message_snapshot.as_ref()
            && let Ok(mut guard) = snapshot.write()
        {
            guard.clear();
        }

        // Fan out to per-session resources (cron, task list, ...).
        // Hooks must not abort the resume — message history is already
        // committed at this point. Aggregate failures into a single
        // SessionResumeWarnings event so the frontend can surface them.
        let mut warnings: Vec<String> = Vec::new();
        for hook in &self.params.resume_hooks {
            if let Err(e) = hook.on_session_changed(&self.params.session.id).await {
                tracing::warn!(error = %e, "session resume hook failed");
                warnings.push(e.to_string());
            }
        }
        if !warnings.is_empty() {
            self.emit(AgentEventPayload::SessionResumeWarnings {
                session_id: self.params.session.id.clone(),
                warnings,
            })
            .await?;
        }

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
