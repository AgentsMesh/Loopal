use loopal_error::{LoopalError, ProviderError, Result};
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::ContentBlock;
use loopal_provider_api::StreamChunk;
use tracing::error;

use super::llm_result::LlmStreamResult;
use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Process a single stream chunk. Returns false to break the loop.
    pub(super) async fn handle_stream_chunk(
        &mut self,
        chunk: std::result::Result<StreamChunk, loopal_error::LoopalError>,
        result: &mut LlmStreamResult,
        received_done: &mut bool,
    ) -> Result<bool> {
        match chunk {
            Ok(StreamChunk::Text { text }) => {
                result.assistant_text.push_str(&text);
                self.emit_in_turn(AgentEventPayload::Stream { text })
                    .await?;
            }
            Ok(StreamChunk::Thinking { text }) => {
                result.thinking_text.push_str(&text);
                self.emit_in_turn(AgentEventPayload::ThinkingStream { text })
                    .await?;
            }
            Ok(StreamChunk::ThinkingSignature { signature }) => {
                // reason: OpenAI emits multiple reasoning items (one per web_search_call);
                // flush each as a Thinking block in stream order so replay has correct pairing
                result.server_blocks.push(ContentBlock::Thinking {
                    thinking: std::mem::take(&mut result.thinking_text),
                    signature: Some(signature),
                });
            }
            Ok(StreamChunk::ToolUse { id, name, input }) => {
                self.emit_in_turn(AgentEventPayload::ToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                })
                .await?;
                result.tool_uses.push((id, name, input));
            }
            Ok(StreamChunk::ServerToolUse { id, name, input }) => {
                self.emit_in_turn(AgentEventPayload::ServerToolUse {
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
                self.emit_in_turn(AgentEventPayload::ServerToolResult {
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
                let full_input =
                    input_tokens + cache_creation_input_tokens + cache_read_input_tokens;
                if full_input > 0 {
                    self.turns.record_actual_input_tokens(full_input);
                }
                self.emit_in_turn(AgentEventPayload::TokenUsage {
                    input_tokens,
                    output_tokens,
                    context_window: self.turns.view().budget().context_window,
                    cache_creation_input_tokens,
                    cache_read_input_tokens,
                    thinking_tokens,
                })
                .await?;
            }
            Ok(StreamChunk::Done { stop_reason }) => {
                *received_done = true;
                result.stop_reason = stop_reason;
                return Ok(false);
            }
            Err(e) => {
                error!(error = %e, turn = self.turn_count, model = %self.params.config.model(), "stream error");
                self.emit_in_turn(AgentEventPayload::ProviderWarning {
                    message: e.to_string(),
                })
                .await?;
                if is_transport_truncation(&e) {
                    result.stream_error = true;
                } else {
                    result.terminal_error = Some(e);
                }
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn is_transport_truncation(error: &LoopalError) -> bool {
    matches!(
        error,
        LoopalError::Provider(
            ProviderError::Http(_) | ProviderError::SseParse(_) | ProviderError::StreamEnded
        )
    )
}
