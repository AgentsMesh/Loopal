use super::cancel::TurnCancel;
use super::llm_result::LlmStreamResult;
use super::runner::AgentLoopRunner;
use futures::StreamExt;
use loopal_error::Result;
use loopal_message::{ContentBlock, Message};
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::StreamChunk;
use std::time::Instant;
use tracing::{error, info};

impl AgentLoopRunner {
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
        let provider = self
            .params
            .deps
            .kernel
            .resolve_provider(&self.params.config.model)?;
        let llm_start = Instant::now();
        info!(
            model = %self.params.config.model, messages = messages.len(),
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
            server_blocks = result.server_blocks.len(),
            has_text = !result.assistant_text.is_empty(),
            thinking_tokens = result.thinking_tokens,
            "LLM complete"
        );
        Ok(result)
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
            Ok(StreamChunk::ServerToolUse { id, name, input }) => {
                self.emit(AgentEventPayload::ServerToolUse {
                    id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                })
                .await?;
                result
                    .server_blocks
                    .push(ContentBlock::ServerToolUse { id, name, input });
            }
            Ok(StreamChunk::ServerToolResult {
                block_type,
                tool_use_id,
                content,
            }) => {
                self.emit(AgentEventPayload::ServerToolResult {
                    tool_use_id: tool_use_id.clone(),
                    content: content.clone(),
                })
                .await?;
                result.server_blocks.push(ContentBlock::ServerToolResult {
                    block_type,
                    tool_use_id,
                    content,
                });
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
                error!(error = %e, turn = self.turn_count, model = %self.params.config.model, "stream error");
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
}
