//! Inner turn execution loop.

use loopal_error::Result;
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::StopReason;
use tracing::{info, warn};

use super::TurnOutput;
use super::runner::AgentLoopRunner;
use super::streaming_tool_exec::{self, StreamingToolHandle, ToolUseArrived};
use super::turn_context::TurnContext;

impl AgentLoopRunner {
    /// Inner loop: LLM → [tools → LLM]* → done.
    pub(super) async fn execute_turn_inner(
        &mut self,
        turn_ctx: &mut TurnContext,
    ) -> Result<TurnOutput> {
        let mut last_text = String::new();
        let mut continuation_count: u32 = 0;
        let mut stop_feedback_count: u32 = 0;
        let max_stop_feedback = self.params.harness.max_stop_feedback;
        loop {
            if turn_ctx.cancel.is_cancelled() {
                info!("turn cancelled before LLM call");
                return Ok(TurnOutput { output: last_text });
            }

            self.check_and_compact().await?;
            let working = self.params.store.prepare_for_llm();
            turn_ctx.metrics.llm_calls += 1;
            let result = self.stream_llm_with(&working, &turn_ctx.cancel).await?;

            // MaxTokens + tool calls = truncated args, discard tools.
            let truncated =
                result.stop_reason == StopReason::MaxTokens && !result.tool_uses.is_empty();
            if truncated {
                warn!("max_tokens hit with tool calls — discarding");
            }
            let effective_tools = if truncated {
                &[][..]
            } else {
                &result.tool_uses
            };

            // Auto-continue: MaxTokens+tools, PauseTurn, or stream truncation.
            let needs_auto_continue = truncated || result.stop_reason == StopReason::PauseTurn;
            let stream_truncated = result.stream_error
                && !turn_ctx.cancel.is_cancelled()
                && !(result.assistant_text.is_empty() && result.tool_uses.is_empty());

            if needs_auto_continue || stream_truncated {
                if stream_truncated {
                    warn!("stream truncated — discarding incomplete tool calls");
                }
                let tools = if stream_truncated {
                    &[][..]
                } else {
                    effective_tools
                };
                self.record_assistant_message(
                    &result.assistant_text,
                    tools,
                    &result.thinking_text,
                    result.thinking_signature.as_deref(),
                    result.server_blocks,
                );
                if !result.assistant_text.is_empty() {
                    last_text.clone_from(&result.assistant_text);
                }
                if continuation_count < self.params.harness.max_auto_continuations {
                    continuation_count += 1;
                    turn_ctx.metrics.auto_continuations = continuation_count;
                    self.emit(AgentEventPayload::AutoContinuation {
                        continuation: continuation_count,
                        max_continuations: self.params.harness.max_auto_continuations,
                    })
                    .await?;
                    self.push_continuation_if_thinking();
                    continue;
                }
                return Ok(TurnOutput { output: last_text });
            }

            // Stream error (cancel or empty truncation) — record partial text, then exit.
            if result.stream_error {
                if !result.assistant_text.is_empty() {
                    let no_tools: &[(String, String, serde_json::Value)] = &[];
                    self.record_assistant_message(
                        &result.assistant_text,
                        no_tools,
                        &result.thinking_text,
                        result.thinking_signature.as_deref(),
                        result.server_blocks,
                    );
                    last_text.clone_from(&result.assistant_text);
                }
                return Ok(TurnOutput { output: last_text });
            }

            self.record_assistant_message(
                &result.assistant_text,
                &result.tool_uses,
                &result.thinking_text,
                result.thinking_signature.as_deref(),
                result.server_blocks,
            );
            if !result.assistant_text.is_empty() {
                last_text.clone_from(&result.assistant_text);
            }

            // No tool calls — check MaxTokens continuation or stop hooks.
            if result.tool_uses.is_empty() {
                if result.stop_reason == StopReason::MaxTokens
                    && continuation_count < self.params.harness.max_auto_continuations
                {
                    continuation_count += 1;
                    turn_ctx.metrics.auto_continuations = continuation_count;
                    self.emit(AgentEventPayload::AutoContinuation {
                        continuation: continuation_count,
                        max_continuations: self.params.harness.max_auto_continuations,
                    })
                    .await?;
                    self.push_continuation_if_thinking();
                    continue;
                }
                if stop_feedback_count < max_stop_feedback
                    && let Some(feedback) = self.run_stop_hooks().await
                {
                    stop_feedback_count += 1;
                    self.push_stop_feedback(feedback);
                    continue;
                }
                return Ok(TurnOutput { output: last_text });
            }

            // Observer: on_before_tools — may inject warnings or abort.
            if self.run_before_tools(turn_ctx, &result.tool_uses).await? {
                return Ok(TurnOutput { output: last_text });
            }

            // Start ReadOnly tools early (parallel with permission checks).
            let kernel = std::sync::Arc::clone(&self.params.deps.kernel);
            let mut early_handle = StreamingToolHandle::empty();
            for (idx, (id, name, input)) in result.tool_uses.iter().enumerate() {
                streaming_tool_exec::feed_tool(
                    &mut early_handle,
                    &kernel,
                    &self.tool_ctx,
                    self.params.config.mode,
                    &ToolUseArrived {
                        index: idx,
                        id: id.clone(),
                        name: name.clone(),
                        input: input.clone(),
                    },
                    self.params.deps.frontend.event_emitter(),
                );
            }

            let tool_names: Vec<&str> = result
                .tool_uses
                .iter()
                .map(|(_, n, _)| n.as_str())
                .collect();
            info!(
                tool_count = result.tool_uses.len(),
                ?tool_names,
                "tool exec start"
            );
            let cancel = &turn_ctx.cancel;
            turn_ctx.metrics.tool_calls_requested += result.tool_uses.len() as u32;
            let stats = self
                .execute_tools_with_early(result.tool_uses.clone(), cancel, early_handle)
                .await?;
            turn_ctx.metrics.tool_calls_approved += stats.approved;
            turn_ctx.metrics.tool_calls_denied += stats.denied;
            turn_ctx.metrics.tool_errors += stats.errors;
            info!("tool exec complete");

            let warnings = std::mem::take(&mut turn_ctx.pending_warnings);
            self.params.store.append_warnings_to_last_user(warnings);

            self.inject_pending_messages().await;
            let result_blocks = self
                .params
                .store
                .messages()
                .last()
                .map(|m| m.content.as_slice())
                .unwrap_or(&[]);
            for obs in &mut self.observers {
                obs.on_after_tools(turn_ctx, &result.tool_uses, result_blocks);
            }

            continuation_count = 0;
        }
    }
}
