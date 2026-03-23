use std::sync::Arc;

use loopal_error::{AgentOutput, Result};
use loopal_protocol::{AgentEventPayload, InterruptSignal};
use loopal_tool_api::ToolContext;
use tokio::sync::Notify;
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
    pub interrupt_notify: Arc<Notify>,
    pub observers: Vec<Box<dyn TurnObserver>>,
}

impl AgentLoopRunner {
    pub fn new(params: AgentLoopParams) -> Self {
        let tool_ctx = ToolContext {
            backend: params
                .kernel
                .create_backend(std::path::Path::new(&params.session.cwd)),
            session_id: params.session.id.clone(),
            shared: params.shared.clone(),
            pending_cwd_switch: Default::default(),
            memory_channel: params.memory_channel.clone(),
        };
        let model_config = ModelConfig::from_model(&params.model, params.thinking_config.clone());
        let interrupt = params.interrupt.clone();
        let interrupt_notify = params.interrupt_notify.clone();
        Self {
            params,
            tool_ctx,
            turn_count: 0,
            tokens: TokenAccumulator::new(),
            model_config,
            interrupt,
            interrupt_notify,
            observers: Vec::new(),
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
        info!(model = %self.params.model, "agent loop started");
        self.emit(AgentEventPayload::Started).await?;

        let result = self.run_loop().await;

        if let Err(ref e) = result {
            let _ = self
                .emit(AgentEventPayload::Error {
                    message: e.to_string(),
                })
                .await;
        }

        let _ = self.emit(AgentEventPayload::Finished).await;
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
        self.params.frontend.emit(payload).await
    }

    /// If a tool (e.g. EnterWorktree) requested a cwd switch, recreate the backend.
    pub(super) fn apply_pending_cwd_switch(&mut self) {
        let new_cwd = self
            .tool_ctx
            .pending_cwd_switch
            .lock()
            .ok()
            .and_then(|mut guard| guard.take());
        if let Some(cwd) = new_cwd {
            info!(new_cwd = %cwd.display(), "applying cwd switch");
            self.tool_ctx.backend = self.params.kernel.create_backend(&cwd);
        }
    }
}
