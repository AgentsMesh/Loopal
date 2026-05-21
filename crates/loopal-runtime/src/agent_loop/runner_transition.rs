use loopal_error::Result;
use loopal_protocol::{AgentEventPayload, AgentStatus};

use super::runner::AgentLoopRunner;
use crate::fire_hooks::fire_hooks;

impl AgentLoopRunner {
    /// Fire a session-level hook (SessionStart, SessionEnd).
    pub(super) async fn fire_session_hook(&self, event: loopal_config::HookEvent) {
        fire_hooks(
            &self.params.deps.kernel,
            event,
            &loopal_hooks::HookContext {
                session_id: Some(&self.params.session.id),
                cwd: Some(&self.params.session.cwd),
                ..Default::default()
            },
        )
        .await;
    }

    /// Transition to a new agent status. Skips if already in target (idempotent).
    ///
    /// If the event emission fails, the local status is rolled back so the
    /// transition can be retried. This keeps `self.status` consistent with
    /// what observers have actually seen.
    pub(super) async fn transition(&mut self, new_status: AgentStatus) -> Result<()> {
        if self.status == new_status {
            return Ok(());
        }
        let old = self.status;
        self.status = new_status;
        let result = match new_status {
            AgentStatus::Starting => Ok(()),
            AgentStatus::Running => self.emit(AgentEventPayload::Running).await,
            AgentStatus::WaitingForInput => self.emit(AgentEventPayload::AwaitingInput).await,
            AgentStatus::Suspended => self.emit(AgentEventPayload::AwaitingInput).await,
            AgentStatus::Finished => self.emit(AgentEventPayload::Finished).await,
            AgentStatus::Error => Ok(()), // Error event carries a message; use transition_error().
        };
        if result.is_err() {
            self.status = old;
        }
        result
    }

    /// Transition to Error status with a message.
    pub(super) async fn transition_error(&mut self, message: String) -> Result<()> {
        self.status = AgentStatus::Error;
        self.emit(AgentEventPayload::Error { message }).await
    }

    /// Recalculate context budget from current model config.
    /// Called after model switch so the compaction thresholds match the new model.
    pub(super) fn recalculate_budget(&mut self) {
        let tool_defs = self.params.deps.kernel.tool_definitions();
        let tool_tokens = loopal_context::ContextBudget::estimate_tool_tokens(&tool_defs);
        let budget = self
            .model_config
            .build_budget(&self.params.config.system_prompt, tool_tokens);
        self.params.store.update_budget(budget);
    }
}
