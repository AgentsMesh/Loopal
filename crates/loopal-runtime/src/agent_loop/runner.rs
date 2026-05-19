use std::sync::Arc;

use loopal_context::ContextPipeline;
use loopal_error::{AgentOutput, Result};
use loopal_protocol::{AgentEventPayload, AgentStatus, InterruptSignal};
use loopal_tool_api::{GoalSession, ToolContext};
use tokio::sync::watch;
use tracing::{Instrument, info, info_span};

use super::AgentLoopParams;
use super::governance::aggregator::{FirstDenyWins, VerdictAggregator};
use super::governance::traits::{Governance, TurnHook};
use super::model_config::ModelConfig;
use super::token_accumulator::TokenAccumulator;
use crate::goal::GoalSessionToolAdapter;
use crate::goal::prompts::DEFAULT_MAX_BARREN_CONTINUATIONS;
use crate::plan_file::PlanFile;

/// Encapsulates the agent loop state and behavior.
pub struct AgentLoopRunner {
    pub params: AgentLoopParams,
    pub tool_ctx: ToolContext,
    pub turn_count: u32,
    pub tokens: TokenAccumulator,
    pub model_config: ModelConfig,
    pub interrupt: InterruptSignal,
    pub interrupt_tx: Arc<watch::Sender<u64>>,
    /// Decision-making governance chain (LoopDetector, future Watchdog).
    pub governance: Vec<Box<dyn Governance>>,
    /// Policy for combining per-governance verdicts into a final outcome.
    /// Default is `FirstDenyWins`; tests or future configs can replace it.
    pub aggregator: Box<dyn VerdictAggregator>,
    /// Observation-only turn hooks (DiffTracker, future telemetry hooks).
    pub hooks: Vec<Box<dyn TurnHook>>,
    pub pipeline: ContextPipeline,
    /// Scheduler message receiver — consumed in `wait_for_input()`.
    pub trigger_rx: Option<tokio::sync::mpsc::Receiver<loopal_protocol::Envelope>>,
    /// Async hook rewake channel — background hooks send Envelopes here.
    pub rewake_rx: Option<tokio::sync::mpsc::Receiver<loopal_protocol::Envelope>>,
    /// Local status for idempotent `transition()` checks.
    ///
    /// This is NOT the authoritative status for external observers. The session
    /// layer derives its `observable.status` solely from agent events
    /// (`AwaitingInput`, `Finished`, `Error`, etc.). If `emit()` fails during
    /// `transition()`, this field is rolled back so the event can be retried.
    pub status: AgentStatus,
    pub plan_file: PlanFile,
    pub pending_consumed_ids: Vec<String>,
    pub barren_continuation_count: u32,
    pub max_barren_continuations: u32,
    pub last_continuation_goal_id: Option<String>,
}

impl AgentLoopRunner {
    pub fn new(mut params: AgentLoopParams) -> Self {
        let goal_adapter: Option<Arc<dyn GoalSession>> = params
            .goal_session
            .as_ref()
            .map(|s| Arc::new(GoalSessionToolAdapter::new(Arc::clone(s))) as Arc<dyn GoalSession>);
        let tool_ctx = ToolContext::new(
            params.deps.kernel.create_backend(
                std::path::Path::new(&params.session.cwd),
                &params.session.id,
            ),
            params.session.id.clone(),
        )
        .with_shared_opt(params.shared.clone())
        .with_memory_channel_opt(params.memory_channel.clone())
        .with_one_shot_chat_opt(params.one_shot_chat.clone())
        .with_fetch_refiner_policy_opt(params.fetch_refiner_policy.clone())
        .with_goal_session_opt(goal_adapter)
        .with_secrets_opt(params.deps.kernel.secrets().cloned());
        let model_config = ModelConfig::from_model(
            params.config.model(),
            params.config.thinking_config.clone(),
            params.config.context_tokens_cap,
        );
        let interrupt = params.interrupt.signal.clone();
        let interrupt_tx = params.interrupt.tx.clone();
        let trigger_rx = params.scheduled_rx.take();
        let rewake_rx = params.rewake_rx.take();
        let plan_file = PlanFile::new(std::path::Path::new(&params.session.cwd));
        Self {
            params,
            tool_ctx,
            turn_count: 0,
            tokens: TokenAccumulator::new(),
            model_config,
            interrupt,
            interrupt_tx,
            governance: Vec::new(),
            aggregator: Box::new(FirstDenyWins),
            hooks: Vec::new(),
            pipeline: ContextPipeline::new(),
            trigger_rx,
            rewake_rx,
            status: AgentStatus::Starting,
            plan_file,
            pending_consumed_ids: Vec::new(),
            barren_continuation_count: 0,
            max_barren_continuations: DEFAULT_MAX_BARREN_CONTINUATIONS,
            last_continuation_goal_id: None,
        }
    }

    /// Main loop — orchestrates input, middleware, LLM, and tool execution.
    /// Guarantees `Finished` event is emitted regardless of exit path.
    pub async fn run(&mut self) -> Result<AgentOutput> {
        let span = info_span!("agent", session.id = %self.params.session.id);
        self.run_instrumented().instrument(span).await
    }

    /// Actual run logic, executed inside the `agent` span.
    async fn run_instrumented(&mut self) -> Result<AgentOutput> {
        info!(model = %self.params.config.model(), "agent loop started");
        self.transition(AgentStatus::Running).await?;
        self.emit(AgentEventPayload::Started).await?;
        self.emit_cold_start_observables().await?;
        self.emit_initial_mcp_status().await;
        self.fire_session_hook(loopal_config::HookEvent::SessionStart)
            .await;

        let result = self.run_loop().await;

        self.fire_session_hook(loopal_config::HookEvent::SessionEnd)
            .await;
        self.cleanup_session_tmp().await;
        self.emit_inbox_consumed().await;

        if let Err(ref e) = result
            && let Err(te) = self.transition_error(e.to_string()).await
        {
            tracing::error!(error = %te, original = %e, "transition_error during shutdown");
        }
        if let Err(e) = self.transition(AgentStatus::Finished).await {
            tracing::error!(error = %e, "transition to Finished during shutdown");
        }
        result
    }

    /// Send an event payload via the frontend.
    pub async fn emit(&self, payload: AgentEventPayload) -> Result<()> {
        self.params.deps.frontend.emit(payload).await
    }

    /// Capability-checked emit — panics if called outside `scope_turn`.
    /// Use from in-turn emit sites that MUST be scoped so missing scope
    /// is a loud failure instead of a silent `turn_id=0`.
    pub async fn emit_in_turn(&self, payload: AgentEventPayload) -> Result<()> {
        self.params.deps.frontend.emit_in_turn(payload).await
    }

    /// Remove this session's tmp dir (`$TMPDIR/loopal/{id}/`).
    /// Excludes log files of still-running background tasks.
    pub(crate) async fn cleanup_session_tmp(&self) {
        let exclude: Vec<std::path::PathBuf> = self
            .params
            .deps
            .kernel
            .bg_store()
            .snapshot(loopal_tool_background::StatusFilter::Running)
            .iter()
            .filter_map(|s| {
                self.params
                    .deps
                    .kernel
                    .bg_store()
                    .read_task(&s.id, |t| t.log_path().to_path_buf())
            })
            .collect();
        loopal_backend::cleanup_session_tmp(&self.params.session.id, &exclude).await;
    }
}
