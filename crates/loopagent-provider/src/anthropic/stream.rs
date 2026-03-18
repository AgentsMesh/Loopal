use futures::stream::Stream;
use loopagent_types::error::{LoopAgentError, ProviderError};
use loopagent_types::provider::{StopReason, StreamChunk};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Context, Poll};

#[derive(Default)]
pub(crate) struct ToolUseAccumulator {
    current_tool_id: Option<String>,
    current_tool_name: Option<String>,
    json_fragments: String,
    stop_reason: Option<StopReason>,
}

pub(crate) struct AnthropicStream {
    pub(crate) inner: Pin<Box<dyn Stream<Item = Result<String, LoopAgentError>> + Send>>,
    pub(crate) state: ToolUseAccumulator,
    pub(crate) buffer: VecDeque<Result<StreamChunk, LoopAgentError>>,
}

impl Stream for AnthropicStream {
    type Item = Result<StreamChunk, LoopAgentError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        // Drain buffer first
        if let Some(item) = this.buffer.pop_front() {
            return Poll::Ready(Some(item));
        }

        // Poll inner SSE stream
        match this.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(data))) => {
                let chunks = parse_anthropic_event(&data, &mut this.state);
                let mut iter = chunks.into_iter();
                if let Some(first) = iter.next() {
                    this.buffer.extend(iter);
                    Poll::Ready(Some(first))
                } else {
                    // No chunks from this event, wake and try again
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

// SAFETY: All fields are Send
unsafe impl Send for AnthropicStream {}
impl Unpin for AnthropicStream {}

pub(crate) fn parse_anthropic_event(
    data: &str,
    state: &mut ToolUseAccumulator,
) -> Vec<Result<StreamChunk, LoopAgentError>> {
    let parsed: Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(e) => {
            return vec![Err(ProviderError::SseParse(format!(
                "invalid JSON: {e}: {data}"
            ))
            .into())]
        }
    };

    let event_type = parsed["type"].as_str().unwrap_or("");
    let mut chunks = Vec::new();

    match event_type {
        "content_block_start" => {
            let block = &parsed["content_block"];
            let block_type = block["type"].as_str().unwrap_or("");
            if block_type == "tool_use" {
                state.current_tool_id = block["id"].as_str().map(String::from);
                state.current_tool_name = block["name"].as_str().map(String::from);
                state.json_fragments.clear();
            }
        }
        "content_block_delta" => {
            let delta = &parsed["delta"];
            let delta_type = delta["type"].as_str().unwrap_or("");
            match delta_type {
                "text_delta" => {
                    if let Some(text) = delta["text"].as_str() {
                        chunks.push(Ok(StreamChunk::Text {
                            text: text.to_string(),
                        }));
                    }
                }
                "input_json_delta" => {
                    if let Some(partial) = delta["partial_json"].as_str() {
                        state.json_fragments.push_str(partial);
                    }
                }
                _ => {}
            }
        }
        "content_block_stop" => {
            // If we were accumulating tool use, emit it now
            if let (Some(id), Some(name)) = (
                state.current_tool_id.take(),
                state.current_tool_name.take(),
            ) {
                let input: Value = if state.json_fragments.is_empty() {
                    json!({})
                } else {
                    serde_json::from_str(&state.json_fragments).unwrap_or(json!({}))
                };
                state.json_fragments.clear();
                chunks.push(Ok(StreamChunk::ToolUse { id, name, input }));
            }
        }
        "message_delta" => {
            if let (Some(input), Some(output)) = (
                parsed["usage"]["input_tokens"].as_u64(),
                parsed["usage"]["output_tokens"].as_u64(),
            ) {
                let cache_creation = parsed["usage"]["cache_creation_input_tokens"]
                    .as_u64().unwrap_or(0) as u32;
                let cache_read = parsed["usage"]["cache_read_input_tokens"]
                    .as_u64().unwrap_or(0) as u32;
                chunks.push(Ok(StreamChunk::Usage {
                    input_tokens: input as u32,
                    output_tokens: output as u32,
                    cache_creation_input_tokens: cache_creation,
                    cache_read_input_tokens: cache_read,
                }));
            }
            if let Some(reason) = parsed["delta"]["stop_reason"].as_str() {
                state.stop_reason = match reason {
                    "max_tokens" => Some(StopReason::MaxTokens),
                    _ => Some(StopReason::EndTurn),
                };
            }
        }
        "message_start" => {
            if let (Some(input), Some(output)) = (
                parsed["message"]["usage"]["input_tokens"].as_u64(),
                parsed["message"]["usage"]["output_tokens"].as_u64(),
            ) {
                let cache_creation = parsed["message"]["usage"]["cache_creation_input_tokens"]
                    .as_u64().unwrap_or(0) as u32;
                let cache_read = parsed["message"]["usage"]["cache_read_input_tokens"]
                    .as_u64().unwrap_or(0) as u32;
                chunks.push(Ok(StreamChunk::Usage {
                    input_tokens: input as u32,
                    output_tokens: output as u32,
                    cache_creation_input_tokens: cache_creation,
                    cache_read_input_tokens: cache_read,
                }));
            }
        }
        "message_stop" => {
            let reason = state.stop_reason.take().unwrap_or(StopReason::EndTurn);
            chunks.push(Ok(StreamChunk::Done { stop_reason: reason }));
        }
        _ => {}
    }

    chunks
}
