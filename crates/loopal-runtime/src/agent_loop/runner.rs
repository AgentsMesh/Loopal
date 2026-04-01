use std::sync::Arc;

use loopal_error::{AgentOutput, Result};
use loopal_protocol::{AgentEventPayload, AgentStatus, InterruptSignal};
use loopal_tool_api::ToolContext;
use tokio::sync::watch;
use tracing::{Instrument, info, info_span};

use super::model_config::ModelConfig;
use super::token_accumulator::TokenAccumulator;
use super::turn_context::TurnContext;
use super::turn_observer::TurnObserver;
use super::{AgentLoopParams, TurnOutput};

/// Encapsulates the agent loop state and behavior.
pub struct AgentLoopRunner {
    pub params: AgentLoopParams,
    pub tool_ctx: ToolContext,
    pub turn_count: u32,
    pub tokens: TokenAccumulator,
    pub model_config: ModelConfig,
    pub interrupt: InterruptSignal,
    pub interrupt_tx: Arc<watch::Sender<u64>>,
    pub observers: Vec<Box<dyn TurnObserver>>,
    /// Scheduler message receiver — consumed in `wait_for_input()`.
    pub trigger_rx: Option<tokio::sync::mpsc::Receiver<loopal_protocol::Envelope>>,
    /// Explicit agent state — source of truth, propagated via events to Session layer.
    pub status: AgentStatus,
}

impl AgentLoopRunner {
    pub fn new(mut params: AgentLoopParams) -> Self {
        let tool_ctx = ToolContext {
            backend: params
                .deps
                .kernel
                .create_backend(std::path::Path::new(&params.session.cwd)),
            session_id: params.session.id.clone(),
            shared: params.shared.clone(),
            memory_channel: params.memory_channel.clone(),
            output_tail: None,
        };
        let model_config = ModelConfig::from_model(
            params.config.model(),
            params.config.thinking_config.clone(),
            params.config.context_tokens_cap,
        );
        let interrupt = params.interrupt.signal.clone();
        let interrupt_tx = params.interrupt.tx.clone();
        let trigger_rx = params.scheduled_rx.take();
        Self {
            params,
            tool_ctx,
            turn_count: 0,
            tokens: TokenAccumulator::new(),
            model_config,
            interrupt,
            interrupt_tx,
            observers: Vec::new(),
            trigger_rx,
            status: AgentStatus::Starting,
        }
    }

    /// Main loop — orchestrates input, middleware, LLM, and tool execution.
    /// Guarantees `Finished` event is emitted regardless of exit path.
    pub async fn run(&mut self) -> Result<AgentOutput> {
        let span = info_span!("agent", session_id = %self.params.session.id);
        self.run_instrumented().instrument(span).await
    }

    /// Actual run logic, executed inside the `agent` span.
    async fn run_instrumented(&mut self) -> Result<AgentOutput> {
        info!(model = %self.params.config.model(), "agent loop started");
        // Started is a one-time lifecycle event (not a status transition).
        // Status moves to Running via transition() before each turn.
        self.transition(AgentStatus::Running).await?;
        self.emit(AgentEventPayload::Started).await?;

        let result = self.run_loop().await;

        if let Err(ref e) = result {
            let _ = self.transition_error(e.to_string()).await;
        }

        let _ = self.transition(AgentStatus::Finished).await;
        result
    }

    /// One complete turn: LLM → [tools → LLM]* → returns when no tool calls.
    ///
    /// Wraps `execute_turn_inner` with observer on_turn_start/on_turn_end.
    pub(super) async fn execute_turn(&mut self, turn_ctx: &mut TurnContext) -> Result<TurnOutput> {
        for obs in &mut self.observers {
            obs.on_turn_start(turn_ctx);
        }
        let result = self.execute_turn_inner(turn_ctx).await;
        for obs in &mut self.observers {
            obs.on_turn_end(turn_ctx);
        }
        result
    }

    /// Send an event payload via the frontend.
    pub async fn emit(&self, payload: AgentEventPayload) -> Result<()> {
        self.params.deps.frontend.emit(payload).await
    }

    /// Transition to a new agent status and emit the corresponding event.
    ///
    /// **This is the ONLY way to change agent status.** Every status change
    /// goes through this method, ensuring SSOT and deterministic event emission.
    pub(super) async fn transition(&mut self, new_status: AgentStatus) -> Result<()> {
        self.status = new_status;
        match new_status {
            AgentStatus::Starting => Ok(()),
            AgentStatus::Running => Ok(()), // Running is signaled implicitly by Stream/ToolCall events.
            AgentStatus::WaitingForInput => self.emit(AgentEventPayload::AwaitingInput).await,
            AgentStatus::Finished => self.emit(AgentEventPayload::Finished).await,
            AgentStatus::Error => Ok(()), // Error event carries a message; use transition_error().
        }
    }

    /// Transition to Error status with a message.
    pub(super) async fn transition_error(&mut self, message: String) -> Result<()> {
        self.status = AgentStatus::Error;
        self.emit(AgentEventPayload::Error { message }).await
    }

    /// Recalculate context budget from current model config.
    ///
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
