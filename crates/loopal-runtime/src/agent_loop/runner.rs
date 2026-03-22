use std::sync::Arc;
use loopal_provider::get_model_info;
use loopal_error::{AgentOutput, Result};
use loopal_protocol::{AgentEventPayload, InterruptSignal};
use loopal_provider_api::{StopReason, ThinkingConfig};
use loopal_tool_api::ToolContext;
use tokio::sync::Notify;
use tracing::{Instrument, info, info_span, warn};

use super::cancel::TurnCancel;
use super::{AgentLoopParams, MAX_AUTO_CONTINUATIONS, TurnOutput};

/// Encapsulates the agent loop state and behavior.
pub struct AgentLoopRunner {
    pub params: AgentLoopParams,
    pub tool_ctx: ToolContext,
    pub turn_count: u32,
    pub total_input_tokens: u32,
    pub total_output_tokens: u32,
    pub total_cache_creation_tokens: u32,
    pub total_cache_read_tokens: u32,
    pub total_thinking_tokens: u32,
    pub thinking_config: ThinkingConfig,
    pub max_context_tokens: u32,
    pub max_output_tokens: u32,
    pub interrupt: InterruptSignal,
    pub interrupt_notify: Arc<Notify>,
}

impl AgentLoopRunner {
    pub fn new(params: AgentLoopParams) -> Self {
        let tool_ctx = ToolContext {
            backend: params.kernel.create_backend(
                std::path::Path::new(&params.session.cwd),
            ),
            session_id: params.session.id.clone(),
            shared: params.shared.clone(),
            pending_cwd_switch: Default::default(),
            memory_channel: params.memory_channel.clone(),
        };
        let model_info = get_model_info(&params.model);
        let max_context_tokens = model_info.as_ref().map_or(200_000, |m| m.context_window);
        let max_output_tokens = model_info.as_ref().map_or(16_384, |m| m.max_output_tokens);
        let thinking_config = params.thinking_config.clone();
        let interrupt = params.interrupt.clone();
        let interrupt_notify = params.interrupt_notify.clone();
        Self {
            params, tool_ctx, turn_count: 0,
            total_input_tokens: 0, total_output_tokens: 0,
            total_cache_creation_tokens: 0, total_cache_read_tokens: 0,
            total_thinking_tokens: 0,
            thinking_config,
            max_context_tokens, max_output_tokens,
            interrupt, interrupt_notify,
        }
    }

    /// Main loop — orchestrates input, middleware, LLM, and tool execution.
    /// Guarantees `Finished` event is emitted regardless of exit path.
    /// Returns structured AgentOutput with result text and termination reason.
    pub async fn run(&mut self) -> Result<AgentOutput> {
        let span = info_span!("agent", session_id = %self.params.session.id);
        self.run_instrumented().instrument(span).await
    }

    /// Actual run logic, executed inside the `agent` span.
    async fn run_instrumented(&mut self) -> Result<AgentOutput> {
        info!(model = %self.params.model, "agent loop started");
        self.emit(AgentEventPayload::Started).await?;

        let result = self.run_loop().await;

        // Surface the fatal error to TUI before finishing
        if let Err(ref e) = result {
            let _ = self.emit(AgentEventPayload::Error {
                message: e.to_string(),
            }).await;
        }

        // Always emit Finished, even on error — TUI relies on this to exit "Working" state.
        let _ = self.emit(AgentEventPayload::Finished).await;

        result
    }

    /// One complete turn: LLM → [tools → LLM]* → returns when no tool calls.
    ///
    /// Each LLM call uses a **working copy** of `params.messages`:
    /// middleware compaction and preflight truncation only affect the copy,
    /// so the persistent history (`params.messages` + storage) is never polluted.
    pub(super) async fn execute_turn(&mut self, cancel: &TurnCancel) -> Result<TurnOutput> {
        let mut last_text = String::new();
        let mut continuation_count: u32 = 0;
        loop {
            // Early exit if interrupted (e.g. tools returned "Interrupted" results
            // and the loop came back — skip the next LLM call).
            if cancel.is_cancelled() {
                return Ok(TurnOutput { output: last_text });
            }

            // Clone persistent history → middleware → preflight → send to LLM
            let mut working = self.params.messages.clone();
            if !self.execute_middleware_on(&mut working).await? {
                return Ok(TurnOutput { output: last_text });
            }
            self.preflight_check_on(&mut working);
            let result = self.stream_llm_with(&working, cancel).await?;

            // max_tokens + tool calls → tool arguments may be truncated, discard tools
            if result.stop_reason == StopReason::MaxTokens && !result.tool_uses.is_empty() {
                warn!("max_tokens hit with tool calls — discarding truncated tools");
                self.record_assistant_message(
                    &result.assistant_text, &[],
                    &result.thinking_text, result.thinking_signature.as_deref(),
                );
                if !result.assistant_text.is_empty() { last_text.clone_from(&result.assistant_text); }
                if continuation_count < MAX_AUTO_CONTINUATIONS {
                    continuation_count += 1;
                    self.emit(AgentEventPayload::AutoContinuation {
                        continuation: continuation_count,
                        max_continuations: MAX_AUTO_CONTINUATIONS,
                    }).await?;
                    continue;
                }
                return Ok(TurnOutput { output: last_text });
            }

            // Record to persistent history (params.messages + storage)
            self.record_assistant_message(
                &result.assistant_text, &result.tool_uses,
                &result.thinking_text, result.thinking_signature.as_deref(),
            );
            if !result.assistant_text.is_empty() { last_text.clone_from(&result.assistant_text); }

            if result.stream_error && result.tool_uses.is_empty() && result.assistant_text.is_empty() {
                return Ok(TurnOutput { output: last_text });
            }

            // max_tokens + pure text → auto-continue
            if result.tool_uses.is_empty() && result.stop_reason == StopReason::MaxTokens {
                if continuation_count < MAX_AUTO_CONTINUATIONS {
                    continuation_count += 1;
                    self.emit(AgentEventPayload::AutoContinuation {
                        continuation: continuation_count,
                        max_continuations: MAX_AUTO_CONTINUATIONS,
                    }).await?;
                    continue;
                }
                return Ok(TurnOutput { output: last_text });
            }

            if result.tool_uses.is_empty() {
                return Ok(TurnOutput { output: result.assistant_text });
            }

            // Execute tools → results appended to persistent params.messages
            let completion_result = self.execute_tools(result.tool_uses, cancel).await?;
            self.apply_pending_cwd_switch();
            self.inject_pending_messages().await;
            continuation_count = 0;

            if let Some(result) = completion_result {
                return Ok(TurnOutput { output: result });
            }
        }
    }

    /// Send an event payload via the frontend.
    pub async fn emit(&self, payload: AgentEventPayload) -> Result<()> {
        self.params.frontend.emit(payload).await
    }

    /// If a tool (e.g. EnterWorktree) requested a cwd switch, recreate the backend.
    fn apply_pending_cwd_switch(&mut self) {
        let new_cwd = self.tool_ctx.pending_cwd_switch.lock().ok()
            .and_then(|mut guard| guard.take());
        if let Some(cwd) = new_cwd {
            info!(new_cwd = %cwd.display(), "applying cwd switch");
            self.tool_ctx.backend = self.params.kernel.create_backend(&cwd);
        }
    }
}
