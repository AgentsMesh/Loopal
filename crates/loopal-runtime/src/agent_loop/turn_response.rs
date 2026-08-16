use loopal_error::Result;
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::{ContinuationReason, StopReason};
use loopal_tool_invocation::{StaleReason, ToolResultMetadata};
use std::hash::{Hash, Hasher};
use tracing::warn;

use super::llm_result::LlmStreamResult;
use super::runner::AgentLoopRunner;
use super::tool_result_sink::PendingToolResult;
use super::turn_context::TurnContext;
use super::turn_state::TurnState;

pub(super) struct TurnLoopCounters {
    pub continuation_count: u32,
    pub stop_feedback_count: u32,
    pub max_continuations: u32,
    pub max_stop_feedback: u32,
}

fn hash_text(text: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

fn record_text_metrics(turn_ctx: &mut TurnContext, text: &str) {
    if text.is_empty() {
        return;
    }
    turn_ctx.metrics.text_output_len = text.len() as u32;
    turn_ctx.metrics.text_hash = Some(hash_text(text));
}

fn record_thinking_metrics(turn_ctx: &mut TurnContext, thinking_blocks: u32) {
    if thinking_blocks > 0 {
        turn_ctx.metrics.thinking_block_count = turn_ctx
            .metrics
            .thinking_block_count
            .saturating_add(thinking_blocks);
    }
}

impl AgentLoopRunner {
    pub(super) async fn handle_response_recorded(
        &mut self,
        turn_ctx: &mut TurnContext,
        mut result: LlmStreamResult,
        c: &mut TurnLoopCounters,
    ) -> Result<TurnState> {
        if let Some(error) = result.terminal_error.take() {
            self.terminalize_discarded_tool_calls(&result.tool_uses)
                .await?;
            self.terminalize_discarded_server_tool_calls(&result.incomplete_server_tool_uses())
                .await?;
            if !result.assistant_text.is_empty() {
                turn_ctx.record_output(&result.assistant_text);
                record_text_metrics(turn_ctx, &result.assistant_text);
            }
            return Err(error);
        }
        let truncated = result.stop_reason == StopReason::MaxTokens && !result.tool_uses.is_empty();
        if truncated {
            warn!("max_tokens hit with tool calls — discarding");
        }
        let incomplete_server_tools = result.incomplete_server_tool_uses();
        let cancelled = turn_ctx.cancel.is_cancelled();
        let stream_truncated = !cancelled
            && (!incomplete_server_tools.is_empty()
                || (result.stream_error
                    && (!result.assistant_text.is_empty()
                        || !result.tool_uses.is_empty()
                        || !result.thinking_text.is_empty()
                        || !result.server_blocks.is_empty())));
        let needs_auto_continue = truncated || result.stop_reason == StopReason::PauseTurn;

        if needs_auto_continue || stream_truncated {
            return self
                .record_for_continuation(turn_ctx, result, c, truncated, stream_truncated)
                .await;
        }

        if result.stream_error {
            self.terminalize_discarded_tool_calls(&result.tool_uses)
                .await?;
            self.terminalize_discarded_server_tool_calls(&incomplete_server_tools)
                .await?;
            if !result.assistant_text.is_empty() {
                let thinking_blocks = result.thinking_block_count();
                self.record_assistant_message(&result.assistant_text, &[], result.server_blocks);
                turn_ctx.record_output(&result.assistant_text);
                record_text_metrics(turn_ctx, &result.assistant_text);
                record_thinking_metrics(turn_ctx, thinking_blocks);
            }
            return Ok(TurnState::Complete);
        }

        if cancelled {
            self.terminalize_discarded_server_tool_calls(&incomplete_server_tools)
                .await?;
        }

        let thinking_blocks = result.thinking_block_count();
        self.record_assistant_message(
            &result.assistant_text,
            &result.tool_uses,
            result.server_blocks,
        );
        if !result.assistant_text.is_empty() {
            turn_ctx.record_output(&result.assistant_text);
            record_text_metrics(turn_ctx, &result.assistant_text);
        }
        record_thinking_metrics(turn_ctx, thinking_blocks);

        if result.tool_uses.is_empty() {
            return self
                .classify_post_tool_empty(turn_ctx, result.stop_reason, c)
                .await;
        }

        Ok(TurnState::NeedsToolExecution {
            tool_uses: result.tool_uses,
        })
    }

    async fn record_for_continuation(
        &mut self,
        turn_ctx: &mut TurnContext,
        result: LlmStreamResult,
        c: &mut TurnLoopCounters,
        truncated: bool,
        stream_truncated: bool,
    ) -> Result<TurnState> {
        if stream_truncated {
            warn!("stream truncated — discarding incomplete tool calls");
        }
        if truncated || stream_truncated {
            self.terminalize_discarded_tool_calls(&result.tool_uses)
                .await?;
            self.terminalize_discarded_server_tool_calls(&result.incomplete_server_tool_uses())
                .await?;
        }
        let tools = if truncated || stream_truncated {
            &[][..]
        } else {
            &result.tool_uses
        };
        let thinking_blocks = result.thinking_block_count();
        self.record_assistant_message(&result.assistant_text, tools, result.server_blocks);
        if !result.assistant_text.is_empty() {
            turn_ctx.record_output(&result.assistant_text);
            record_text_metrics(turn_ctx, &result.assistant_text);
        }
        record_thinking_metrics(turn_ctx, thinking_blocks);
        if c.continuation_count >= c.max_continuations {
            if stream_truncated {
                return Err(result
                    .stream_failure
                    .unwrap_or_else(|| loopal_error::ProviderError::StreamEnded.into()));
            }
            return Ok(TurnState::Complete);
        }
        let reason = if stream_truncated {
            ContinuationReason::StreamTruncated
        } else if truncated {
            ContinuationReason::MaxTokensWithTools
        } else {
            ContinuationReason::PauseTurn
        };
        c.continuation_count += 1;
        turn_ctx.metrics.auto_continuations = c.continuation_count;
        self.emit_in_turn(AgentEventPayload::AutoContinuation {
            continuation: c.continuation_count,
            max_continuations: c.max_continuations,
            reason: continuation_reason_wire(reason).into(),
        })
        .await?;
        Ok(TurnState::NeedsContinuation { reason })
    }

    async fn terminalize_discarded_tool_calls(
        &self,
        tool_uses: &[(String, String, serde_json::Value)],
    ) -> Result<()> {
        for (id, name, _) in tool_uses {
            let finalized = PendingToolResult::new(
                id,
                name,
                "Tool call discarded because the model response was incomplete",
                true,
                Some(ToolResultMetadata::stale(
                    StaleReason::IncompleteModelResponse,
                )),
            )
            .finalize(self)
            .await?;
            self.emit_in_turn(finalized.event).await?;
        }
        Ok(())
    }

    async fn terminalize_discarded_server_tool_calls(
        &self,
        tool_uses: &[(String, String)],
    ) -> Result<()> {
        for (tool_use_id, _) in tool_uses {
            self.emit_in_turn(AgentEventPayload::ServerToolDiscarded {
                tool_use_id: tool_use_id.clone(),
                reason: StaleReason::IncompleteModelResponse,
            })
            .await?;
        }
        Ok(())
    }

    async fn classify_post_tool_empty(
        &mut self,
        turn_ctx: &mut TurnContext,
        stop_reason: StopReason,
        c: &mut TurnLoopCounters,
    ) -> Result<TurnState> {
        if stop_reason == StopReason::MaxTokens && c.continuation_count < c.max_continuations {
            c.continuation_count += 1;
            turn_ctx.metrics.auto_continuations = c.continuation_count;
            self.emit_in_turn(AgentEventPayload::AutoContinuation {
                continuation: c.continuation_count,
                max_continuations: c.max_continuations,
                reason: continuation_reason_wire(ContinuationReason::MaxTokensWithoutTools).into(),
            })
            .await?;
            return Ok(TurnState::NeedsContinuation {
                reason: ContinuationReason::MaxTokensWithoutTools,
            });
        }
        if c.stop_feedback_count < c.max_stop_feedback
            && let Some(feedback) = self.run_stop_hooks().await
        {
            c.stop_feedback_count += 1;
            return Ok(TurnState::NeedsStopFeedback { feedback });
        }
        Ok(TurnState::Complete)
    }
}

fn continuation_reason_wire(reason: ContinuationReason) -> &'static str {
    match reason {
        ContinuationReason::MaxTokensWithoutTools => "max_tokens_without_tools",
        ContinuationReason::MaxTokensWithTools => "max_tokens_with_tools",
        ContinuationReason::PauseTurn => "pause_turn",
        ContinuationReason::StreamTruncated => "stream_truncated",
        ContinuationReason::RecoveryRetry => "recovery_retry",
    }
}

#[cfg(test)]
#[path = "turn_response_tests.rs"]
mod tests;
