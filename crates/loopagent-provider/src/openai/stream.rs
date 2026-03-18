use futures::stream::Stream;
use loopagent_types::error::{LoopAgentError, ProviderError};
use loopagent_types::provider::{StopReason, StreamChunk};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Context, Poll};

#[derive(Default)]
pub(crate) struct ToolCallAccumulator {
    /// Maps tool call index -> (id, name, arguments_json)
    pub(super) calls: Vec<(String, String, String)>,
}

pub(crate) struct OpenAiStream {
    pub(crate) inner: Pin<Box<dyn Stream<Item = Result<String, LoopAgentError>> + Send>>,
    pub(crate) state: ToolCallAccumulator,
    pub(crate) buffer: VecDeque<Result<StreamChunk, LoopAgentError>>,
}

impl Stream for OpenAiStream {
    type Item = Result<StreamChunk, LoopAgentError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if let Some(item) = this.buffer.pop_front() {
            return Poll::Ready(Some(item));
        }

        match this.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(data))) => {
                let chunks = parse_openai_event(&data, &mut this.state);
                let mut iter = chunks.into_iter();
                if let Some(first) = iter.next() {
                    this.buffer.extend(iter);
                    Poll::Ready(Some(first))
                } else {
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

unsafe impl Send for OpenAiStream {}
impl Unpin for OpenAiStream {}

pub(crate) fn parse_openai_event(
    data: &str,
    state: &mut ToolCallAccumulator,
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

    let mut chunks = Vec::new();

    // Handle usage (final chunk with usage stats)
    if let Some(usage) = parsed.get("usage").filter(|u| !u.is_null())
        && let (Some(input), Some(output)) = (
            usage["prompt_tokens"].as_u64(),
            usage["completion_tokens"].as_u64(),
        ) {
            chunks.push(Ok(StreamChunk::Usage {
                input_tokens: input as u32,
                output_tokens: output as u32,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            }));
        }

    let choices = match parsed["choices"].as_array() {
        Some(c) => c,
        None => return chunks,
    };

    for choice in choices {
        let delta = &choice["delta"];
        let finish_reason = choice["finish_reason"].as_str();

        // Text content
        if let Some(content) = delta["content"].as_str()
            && !content.is_empty() {
                chunks.push(Ok(StreamChunk::Text {
                    text: content.to_string(),
                }));
            }

        // Tool calls
        if let Some(tool_calls) = delta["tool_calls"].as_array() {
            for tc in tool_calls {
                let index = tc["index"].as_u64().unwrap_or(0) as usize;

                // Reject unreasonably large indices to prevent unbounded allocation
                if index > 128 {
                    continue;
                }

                // Grow the accumulator
                while state.calls.len() <= index {
                    state.calls.push((String::new(), String::new(), String::new()));
                }

                if let Some(id) = tc["id"].as_str() {
                    state.calls[index].0 = id.to_string();
                }
                if let Some(name) = tc["function"]["name"].as_str() {
                    state.calls[index].1 = name.to_string();
                }
                if let Some(args) = tc["function"]["arguments"].as_str() {
                    state.calls[index].2.push_str(args);
                }
            }
        }

        // On finish, emit accumulated tool calls
        if finish_reason == Some("tool_calls") || finish_reason == Some("stop")
            || finish_reason == Some("length")
        {
            for (id, name, args) in state.calls.drain(..) {
                if !id.is_empty() && !name.is_empty() {
                    let input: Value = serde_json::from_str(&args).unwrap_or(json!({}));
                    chunks.push(Ok(StreamChunk::ToolUse { id, name, input }));
                }
            }
            if finish_reason == Some("stop") || finish_reason == Some("length") {
                let stop_reason = if finish_reason == Some("length") {
                    StopReason::MaxTokens
                } else {
                    StopReason::EndTurn
                };
                chunks.push(Ok(StreamChunk::Done { stop_reason }));
            }
        }
    }

    chunks
}
