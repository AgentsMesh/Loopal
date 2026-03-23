use futures::StreamExt;
use loopal_error::Result;
use loopal_message::Message;
use loopal_protocol::AgentEventPayload;
use loopal_provider::{get_thinking_capability, resolve_thinking_config};
use loopal_provider_api::{ChatParams, StreamChunk};
use std::time::Instant;
use tracing::{error, info, warn};

use super::cancel::TurnCancel;
use super::llm_result::LlmStreamResult;
use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Build chat params from a provided message slice (typically a working copy).
    pub fn prepare_chat_params_with(&self, messages: &[Message]) -> Result<ChatParams> {
        let env_section = super::env_context::build_env_section(
            self.tool_ctx.backend.cwd(),
            self.turn_count,
            self.params.max_turns,
        );
        let full_system_prompt = format!(
            "{}{}{}",
            self.params.system_prompt,
            self.params.mode.system_prompt_suffix(),
            env_section,
        );
        let mut tool_defs = self.params.kernel.tool_definitions();
        if let Some(ref filter) = self.params.tool_filter {
            tool_defs.retain(|t| filter.contains(&t.name));
        }
        let capability = get_thinking_capability(&self.params.model);
        let resolved_thinking = resolve_thinking_config(
            &self.model_config.thinking,
            capability,
            self.model_config.max_output_tokens,
        );
        Ok(ChatParams {
            model: self.params.model.clone(),
            messages: messages.to_vec(),
            system_prompt: full_system_prompt,
            tools: tool_defs,
            max_tokens: self.model_config.max_output_tokens,
            temperature: None,
            thinking: resolved_thinking,
            debug_dump_dir: Some(loopal_config::tmp_dir()),
        })
    }

    /// Stream the LLM response using a provided working copy of messages.
    pub async fn stream_llm_with(
        &mut self,
        messages: &[Message],
        cancel: &TurnCancel,
    ) -> Result<LlmStreamResult> {
        if cancel.is_cancelled() {
            return Ok(LlmStreamResult {
                stream_error: true,
                ..Default::default()
            });
        }

        let chat_params = self.prepare_chat_params_with(messages)?;
        let provider = self.params.kernel.resolve_provider(&self.params.model)?;
        let llm_start = Instant::now();
        info!(
            model = %self.params.model, messages = messages.len(),
            tools = chat_params.tools.len(), max_tokens = chat_params.max_tokens,
            thinking = ?chat_params.thinking, "LLM request"
        );

        let mut stream = self
            .retry_stream_chat(&chat_params, &*provider, cancel)
            .await?;
        let mut result = LlmStreamResult::default();

        loop {
            tokio::select! {
                biased;
                chunk = stream.next() => {
                    let Some(chunk_result) = chunk else { break; };
                    if !self.handle_stream_chunk(chunk_result, &mut result).await? {
                        break;
                    }
                }
                _ = cancel.cancelled() => {
                    info!("cancelled during LLM streaming");
                    result.stream_error = true;
                    break;
                }
            }
        }

        self.emit_thinking_complete(&result).await?;
        let llm_duration = llm_start.elapsed();
        info!(
            duration_ms = llm_duration.as_millis() as u64,
            tool_calls = result.tool_uses.len(),
            has_text = !result.assistant_text.is_empty(),
            thinking_tokens = result.thinking_tokens,
            "LLM complete"
        );
        Ok(result)
    }

    /// Retry loop for the initial stream_chat API call.
    async fn retry_stream_chat(
        &mut self,
        params: &ChatParams,
        provider: &dyn loopal_provider_api::Provider,
        cancel: &TurnCancel,
    ) -> Result<loopal_provider_api::ChatStream> {
        const MAX_RETRIES: u32 = 6;
        const BASE_WAIT_MS: u64 = 2000;
        let mut retry_count = 0;
        loop {
            match provider.stream_chat(params).await {
                Ok(s) => return Ok(s),
                Err(e) if e.is_retryable() && retry_count < MAX_RETRIES => {
                    retry_count += 1;
                    let wait_ms =
                        e.retry_after_ms().unwrap_or(BASE_WAIT_MS) * (1 << (retry_count - 1));
                    warn!(retry = retry_count, max_retries = MAX_RETRIES, wait_ms, error = %e, "retrying");
                    self.emit(AgentEventPayload::Error {
                        message: format!(
                            "{}. Retrying in {:.1}s ({}/{})",
                            e,
                            wait_ms as f64 / 1000.0,
                            retry_count,
                            MAX_RETRIES
                        ),
                    })
                    .await?;
                    // Interruptible sleep via select!
                    tokio::select! {
                        _ = tokio::time::sleep(std::time::Duration::from_millis(wait_ms)) => {}
                        _ = cancel.cancelled() => {
                            info!("cancelled during retry wait");
                            return Ok(Box::pin(futures::stream::empty()));
                        }
                    }
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Process a single stream chunk. Returns false to break the loop.
    async fn handle_stream_chunk(
        &mut self,
        chunk: std::result::Result<StreamChunk, loopal_error::LoopalError>,
        result: &mut LlmStreamResult,
    ) -> Result<bool> {
        match chunk {
            Ok(StreamChunk::Text { text }) => {
                result.assistant_text.push_str(&text);
                self.emit(AgentEventPayload::Stream { text }).await?;
            }
            Ok(StreamChunk::Thinking { text }) => {
                result.thinking_text.push_str(&text);
                self.emit(AgentEventPayload::ThinkingStream { text })
                    .await?;
            }
            Ok(StreamChunk::ThinkingSignature { signature }) => {
                result.thinking_signature = Some(signature);
            }
            Ok(StreamChunk::ToolUse { id, name, input }) => {
                self.emit(AgentEventPayload::ToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                })
                .await?;
                result.tool_uses.push((id, name, input));
            }
            Ok(StreamChunk::Usage {
                input_tokens,
                output_tokens,
                cache_creation_input_tokens,
                cache_read_input_tokens,
                thinking_tokens,
            }) => {
                self.tokens.input += input_tokens;
                self.tokens.output += output_tokens;
                self.tokens.cache_creation += cache_creation_input_tokens;
                self.tokens.cache_read += cache_read_input_tokens;
                self.tokens.thinking += thinking_tokens;
                result.thinking_tokens += thinking_tokens;
                self.emit(AgentEventPayload::TokenUsage {
                    input_tokens,
                    output_tokens,
                    context_window: self.model_config.max_context_tokens,
                    cache_creation_input_tokens,
                    cache_read_input_tokens,
                    thinking_tokens,
                })
                .await?;
            }
            Ok(StreamChunk::Done { stop_reason }) => {
                result.stop_reason = stop_reason;
                return Ok(false);
            }
            Err(e) => {
                error!(error = %e, turn = self.turn_count, model = %self.params.model, "stream error");
                self.emit(AgentEventPayload::Error {
                    message: e.to_string(),
                })
                .await?;
                result.stream_error = true;
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Emit ThinkingComplete if thinking content or tokens were received.
    async fn emit_thinking_complete(&self, result: &LlmStreamResult) -> Result<()> {
        if result.thinking_text.is_empty() && result.thinking_tokens == 0 {
            return Ok(());
        }
        let token_count = if result.thinking_text.is_empty() {
            result.thinking_tokens
        } else {
            result
                .thinking_tokens
                .max(result.thinking_text.len() as u32 / 4)
        };
        self.emit(AgentEventPayload::ThinkingComplete { token_count })
            .await
    }
}
