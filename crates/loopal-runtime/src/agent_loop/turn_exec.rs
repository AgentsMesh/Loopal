//! Inner turn execution loop and observer dispatch.
//!
//! Split from runner.rs to keep files under 200 lines.

use loopal_error::Result;
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::StopReason;
use tracing::{debug, info, warn};

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
                info!("turn cancelled before LLM call");
                return Ok(TurnOutput { output: last_text });
            }

            // Persistent compaction (LLM summarization if over budget)
            self.check_and_compact().await?;
            // Prepare context for LLM (clone + strip old thinking)
            let working = self.params.store.prepare_for_llm();
            let result = self.stream_llm_with(&working, &turn_ctx.cancel).await?;

            // Determine tool list for recording. MaxTokens+tools = truncated args.
            let truncated =
                result.stop_reason == StopReason::MaxTokens && !result.tool_uses.is_empty();
            if truncated {
                warn!("max_tokens hit with tool calls — discarding truncated tools");
            }
            let effective_tools = if truncated {
                &[][..]
            } else {
                &result.tool_uses
            };

            // Auto-continue triggers: MaxTokens+tools, PauseTurn (server-side limit)
            let needs_auto_continue = truncated || result.stop_reason == StopReason::PauseTurn;
            if needs_auto_continue {
                self.record_assistant_message(
                    &result.assistant_text,
                    effective_tools,
                    &result.thinking_text,
                    result.thinking_signature.as_deref(),
                    result.server_blocks,
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
                result.server_blocks,
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
            debug!(tool_count = result.tool_uses.len(), "pre-tool phase");
            if self.run_before_tools(turn_ctx, &result.tool_uses).await? {
                return Ok(TurnOutput { output: last_text });
            }

            let tool_names: Vec<&str> = result.tool_uses.iter().map(|(_, n, _)| n.as_str()).collect();
            info!(tool_count = result.tool_uses.len(), ?tool_names, "tool exec start");
            let cancel = &turn_ctx.cancel;
            let completion = self.execute_tools(result.tool_uses.clone(), cancel).await?;
            info!("tool exec complete");

            // Append observer warnings (e.g. loop detector) AFTER tool results.
            // They must come after ToolResult blocks — inserting them before
            // breaks tool_use/tool_result pairing when normalize_messages merges
            // consecutive same-role User messages.
            let warnings = std::mem::take(&mut turn_ctx.pending_warnings);
            self.params.store.append_warnings_to_last_user(warnings);

            self.inject_pending_messages().await;

            // Observer: on_after_tools with results from the last message
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
                    // Store in context — appended to tool results message later.
                    // Pushing a separate User(Text) message here would break
                    // tool_use/tool_result pairing after normalize_messages merges
                    // consecutive same-role messages.
                    turn_ctx.pending_warnings.push(msg);
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
