use std::time::Instant;

use futures::StreamExt;
use loopagent_types::error::Result;
use loopagent_types::event::AgentEvent;
use loopagent_types::message::{ContentBlock, Message, MessageRole};
use loopagent_types::provider::{ChatParams, StreamChunk};
use tracing::{error, info, warn};

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Build chat params and prepare for LLM call.
    pub(crate) fn prepare_chat_params(&self) -> Result<ChatParams> {
        let full_system_prompt = format!(
            "{}{}",
            self.params.system_prompt,
            self.params.mode.system_prompt_suffix()
        );
        let tool_defs = self.params.kernel.tool_definitions();

        Ok(ChatParams {
            model: self.params.model.clone(),
            messages: self.params.messages.clone(),
            system_prompt: full_system_prompt,
            tools: tool_defs,
            max_tokens: self.max_output_tokens,
            temperature: None,
        })
    }

    /// Stream the LLM response, collecting text, tool uses, and usage stats.
    /// Returns (assistant_text, tool_uses, stream_error).
    /// Includes automatic retry with exponential backoff for rate limiting (429).
    pub(crate) async fn stream_llm(&mut self) -> Result<(String, Vec<(String, String, serde_json::Value)>, bool)> {
        let chat_params = self.prepare_chat_params()?;
        let provider = self.params.kernel.resolve_provider(&self.params.model)?;

        let llm_start = Instant::now();
        info!(
            model = %self.params.model,
            messages = self.params.messages.len(),
            tools = chat_params.tools.len(),
            max_tokens = chat_params.max_tokens,
            "LLM request"
        );

        // Retry loop for rate limiting
        const MAX_RETRIES: u32 = 3;
        let mut retry_count = 0;
        let mut stream = loop {
            match provider.stream_chat(&chat_params).await {
                Ok(stream) => break stream,
                Err(e) if e.is_rate_limited() && retry_count < MAX_RETRIES => {
                    retry_count += 1;
                    let base_wait = e.retry_after_ms().unwrap_or(1000);
                    // Exponential backoff: base_wait * 2^(retry-1)
                    let wait_ms = base_wait * (1 << (retry_count - 1));
                    warn!(
                        retry = retry_count,
                        max_retries = MAX_RETRIES,
                        wait_ms,
                        "rate limited, retrying after backoff"
                    );
                    self.emit(AgentEvent::Error {
                        message: format!(
                            "Rate limited. Retrying in {:.1}s ({}/{})",
                            wait_ms as f64 / 1000.0,
                            retry_count,
                            MAX_RETRIES
                        ),
                    })
                    .await?;
                    tokio::time::sleep(std::time::Duration::from_millis(wait_ms)).await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        };

        let mut assistant_text = String::new();
        let mut tool_uses: Vec<(String, String, serde_json::Value)> = Vec::new();
        let mut stream_error = false;

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(StreamChunk::Text { text }) => {
                    assistant_text.push_str(&text);
                    self.emit(AgentEvent::Stream { text }).await?;
                }
                Ok(StreamChunk::ToolUse { id, name, input }) => {
                    self.emit(AgentEvent::ToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        input: input.clone(),
                    })
                    .await?;
                    tool_uses.push((id, name, input));
                }
                Ok(StreamChunk::Usage { input_tokens, output_tokens }) => {
                    self.total_input_tokens += input_tokens;
                    self.total_output_tokens += output_tokens;
                    self.emit(AgentEvent::TokenUsage {
                        input_tokens,
                        output_tokens,
                        context_window: self.max_context_tokens,
                    })
                    .await?;
                    info!(
                        total_input = self.total_input_tokens,
                        total_output = self.total_output_tokens,
                        context_window = self.max_context_tokens,
                        input_tokens,
                        output_tokens,
                        "token usage"
                    );
                }
                Ok(StreamChunk::Done) => break,
                Err(e) => {
                    error!(error = %e, turn = self.turn_count, model = %self.params.model, "stream error");
                    self.emit(AgentEvent::Error {
                        message: e.to_string(),
                    })
                    .await?;
                    stream_error = true;
                    break;
                }
            }
        }

        let llm_duration = llm_start.elapsed();
        info!(
            duration_ms = llm_duration.as_millis() as u64,
            tool_calls = tool_uses.len(),
            has_text = !assistant_text.is_empty(),
            "LLM complete"
        );

        Ok((assistant_text, tool_uses, stream_error))
    }

    /// Record the assistant response as a message in the conversation history.
    pub(crate) fn record_assistant_message(
        &mut self,
        assistant_text: &str,
        tool_uses: &[(String, String, serde_json::Value)],
    ) {
        let mut assistant_content: Vec<ContentBlock> = Vec::new();
        if !assistant_text.is_empty() {
            assistant_content.push(ContentBlock::Text {
                text: assistant_text.to_string(),
            });
        }
        for (id, name, input) in tool_uses {
            assistant_content.push(ContentBlock::ToolUse {
                id: id.clone(),
                name: name.clone(),
                input: input.clone(),
            });
        }

        if !assistant_content.is_empty() {
            let assistant_msg = Message {
                role: MessageRole::Assistant,
                content: assistant_content,
            };
            if let Err(e) = self.params.session_manager.save_message(&self.params.session.id, &assistant_msg) {
                error!(error = %e, "failed to persist message");
            }
            self.params.messages.push(assistant_msg);
        }
    }
}
