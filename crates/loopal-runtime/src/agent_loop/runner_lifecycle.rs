use loopal_error::{AgentOutput, Result};
use loopal_protocol::{AgentEventPayload, AgentStatus};
use tracing::{Instrument, info, info_span};

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Main loop — orchestrates input, middleware, LLM, and tool execution.
    /// Guarantees `Finished` event is emitted regardless of exit path.
    pub async fn run(&mut self) -> Result<AgentOutput> {
        let span = info_span!("agent", session.id = %self.params.session.id);
        self.run_instrumented().instrument(span).await
    }

    async fn run_instrumented(&mut self) -> Result<AgentOutput> {
        info!(model = %self.params.config.model(), "agent loop started");
        self.transition(AgentStatus::Running).await?;
        self.emit(AgentEventPayload::Started).await?;
        self.emit_cold_start_observables().await?;
        self.emit_initial_mcp_status().await;
        self.spawn_mcp_settle_emitter();
        self.spawn_hub_health_poller();
        self.fire_session_hook(loopal_config::HookEvent::SessionStart)
            .await;

        let result = self.run_loop().await;

        self.fire_session_hook(loopal_config::HookEvent::SessionEnd)
            .await;
        self.cleanup_session_tmp().await;
        self.emit_inbox_consumed().await;

        if let Err(ref error) = result
            && let Err(transition_error) = self.transition_error(error.to_string()).await
        {
            tracing::error!(
                error = %transition_error,
                original = %error,
                "transition_error during shutdown"
            );
        }
        if let Err(error) = self.transition(AgentStatus::Finished).await {
            tracing::error!(error = %error, "transition to Finished during shutdown");
        }
        result
    }

    /// Send an event payload via the frontend.
    pub async fn emit(&self, payload: AgentEventPayload) -> Result<()> {
        self.params.deps.frontend.emit(payload).await
    }

    /// Best-effort emit for cosmetic or progress events.
    pub async fn emit_cosmetic(&self, payload: AgentEventPayload) {
        if let Err(error) = self.params.deps.frontend.emit(payload).await {
            tracing::warn!(error = %error, "cosmetic emit dropped; continuing");
        }
    }

    /// Capability-checked emit for events produced inside a turn scope.
    pub async fn emit_in_turn(&self, payload: AgentEventPayload) -> Result<()> {
        self.params.deps.frontend.emit_in_turn(payload).await
    }

    /// Remove this session's tmp dir while preserving live background logs.
    pub(crate) async fn cleanup_session_tmp(&self) {
        let exclude: Vec<std::path::PathBuf> = self
            .params
            .deps
            .kernel
            .bg_store()
            .snapshot(loopal_tool_background::StatusFilter::Running)
            .iter()
            .filter_map(|summary| {
                self.params
                    .deps
                    .kernel
                    .bg_store()
                    .read_task(&summary.id, |task| task.log_path().to_path_buf())
            })
            .collect();
        loopal_backend::cleanup_session_tmp(&self.params.session.id, &exclude).await;
    }
}
