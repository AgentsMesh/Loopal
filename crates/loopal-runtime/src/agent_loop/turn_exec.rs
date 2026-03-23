//! Inner turn execution loop and observer dispatch.
//!
//! Split from runner.rs to keep files under 200 lines.

use loopal_error::Result;
use loopal_message::{ContentBlock, Message, MessageRole};
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::StopReason;
use tracing::warn;

use super::runner::AgentLoopRunner;
use super::turn_context::TurnContext;
use super::turn_observer::ObserverAction;
use super::{MAX_AUTO_CONTINUATIONS, TurnOutput};

impl AgentLoopRunner {
    /// Inner loop: LLM → [tools → LLM]* → done.
    pub(super) async fn execute_turn_inner(
        &mut self,
        turn_ctx: &mut TurnContext,
    ) -> Result<TurnOutput> {
        let mut last_text = String::new();
        let mut continuation_count: u32 = 0;
        loop {
            if turn_ctx.cancel.is_cancelled() {
                return Ok(TurnOutput { output: last_text });
            }

            let mut working = self.params.messages.clone();
            if !self.execute_middleware_on(&mut working).await? {
                return Ok(TurnOutput { output: last_text });
            }
            self.preflight_check_on(&mut working);
            let result = self.stream_llm_with(&working, &turn_ctx.cancel).await?;

            // max_tokens + tool calls → discard truncated tools
            if result.stop_reason == StopReason::MaxTokens && !result.tool_uses.is_empty() {
                warn!("max_tokens hit with tool calls — discarding truncated tools");
                self.record_assistant_message(
                    &result.assistant_text,
                    &[],
                    &result.thinking_text,
                    result.thinking_signature.as_deref(),
                );
                if !result.assistant_text.is_empty() {
                    last_text.clone_from(&result.assistant_text);
                }
                if continuation_count < MAX_AUTO_CONTINUATIONS {
                    continuation_count += 1;
                    self.emit(AgentEventPayload::AutoContinuation {
                        continuation: continuation_count,
                        max_continuations: MAX_AUTO_CONTINUATIONS,
                    })
                    .await?;
                    continue;
                }
                return Ok(TurnOutput { output: last_text });
            }

            self.record_assistant_message(
                &result.assistant_text,
                &result.tool_uses,
                &result.thinking_text,
                result.thinking_signature.as_deref(),
            );
            if !result.assistant_text.is_empty() {
                last_text.clone_from(&result.assistant_text);
            }

            if result.stream_error
                && result.tool_uses.is_empty()
                && result.assistant_text.is_empty()
            {
                return Ok(TurnOutput { output: last_text });
            }

            if result.tool_uses.is_empty() && result.stop_reason == StopReason::MaxTokens {
                if continuation_count < MAX_AUTO_CONTINUATIONS {
                    continuation_count += 1;
                    self.emit(AgentEventPayload::AutoContinuation {
                        continuation: continuation_count,
                        max_continuations: MAX_AUTO_CONTINUATIONS,
                    })
                    .await?;
                    continue;
                }
                return Ok(TurnOutput { output: last_text });
            }

            if result.tool_uses.is_empty() {
                return Ok(TurnOutput {
                    output: result.assistant_text,
                });
            }

            // Observer: on_before_tools
            if self.run_before_tools(turn_ctx, &result.tool_uses).await? {
                return Ok(TurnOutput { output: last_text });
            }

            let cancel = &turn_ctx.cancel;
            let completion = self.execute_tools(result.tool_uses.clone(), cancel).await?;
            self.apply_pending_cwd_switch();
            self.inject_pending_messages().await;

            // Observer: on_after_tools with results from the last message
            let result_blocks = self
                .params
                .messages
                .last()
                .map(|m| m.content.as_slice())
                .unwrap_or(&[]);
            for obs in &mut self.observers {
                obs.on_after_tools(turn_ctx, &result.tool_uses, result_blocks);
            }

            continuation_count = 0;
            if let Some(r) = completion {
                return Ok(TurnOutput { output: r });
            }
        }
    }

    /// Run before-tools observers. Returns true if the turn should abort.
    pub(super) async fn run_before_tools(
        &mut self,
        turn_ctx: &mut TurnContext,
        tool_uses: &[(String, String, serde_json::Value)],
    ) -> Result<bool> {
        for obs in &mut self.observers {
            match obs.on_before_tools(turn_ctx, tool_uses) {
                ObserverAction::Continue => {}
                ObserverAction::InjectWarning(msg) => {
                    self.params.messages.push(Message {
                        id: None,
                        role: MessageRole::User,
                        content: vec![ContentBlock::Text { text: msg }],
                    });
                }
                ObserverAction::AbortTurn(reason) => {
                    warn!(%reason, "observer aborted turn");
                    self.emit(AgentEventPayload::Error { message: reason })
                        .await?;
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}
