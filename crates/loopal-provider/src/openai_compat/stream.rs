use futures::stream::Stream;
use loopal_error::{LoopalError, ProviderError};
use loopal_provider_api::{StopReason, StreamChunk};
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Context, Poll};

#[derive(Default)]
pub(crate) struct ToolCallAccumulator {
    /// Maps tool call index -> (id, name, arguments_json)
    pub(super) calls: Vec<(String, String, String)>,
}

pub(crate) struct CompatStream {
    pub(crate) inner: Pin<Box<dyn Stream<Item = Result<String, LoopalError>> + Send>>,
    pub(crate) state: ToolCallAccumulator,
    pub(crate) buffer: VecDeque<Result<StreamChunk, LoopalError>>,
}

impl Stream for CompatStream {
    type Item = Result<StreamChunk, LoopalError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if let Some(item) = this.buffer.pop_front() {
            return Poll::Ready(Some(item));
        }
        match this.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(data))) => {
                let chunks = parse_chat_completions_event(&data, &mut this.state);
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

unsafe impl Send for CompatStream {}
impl Unpin for CompatStream {}

pub(crate) fn parse_chat_completions_event(
    data: &str,
    state: &mut ToolCallAccumulator,
) -> Vec<Result<StreamChunk, LoopalError>> {
    let parsed: Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(e) => {
            return vec![Err(ProviderError::SseParse(format!(
                "invalid JSON: {e}: {data}"
            ))
            .into())];
        }
    };

    let mut chunks = Vec::new();

    if let Some(usage) = parsed.get("usage").filter(|u| !u.is_null())
        && let (Some(input), Some(output)) = (
            usage["prompt_tokens"].as_u64(),
            usage["completion_tokens"].as_u64(),
        )
    {
        let thinking_tokens = usage["completion_tokens_details"]["reasoning_tokens"]
            .as_u64()
            .unwrap_or(0) as u32;
        chunks.push(Ok(StreamChunk::Usage {
            input_tokens: input as u32,
            output_tokens: output as u32,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            thinking_tokens,
        }));
    }

    let Some(choices) = parsed["choices"].as_array() else {
        return chunks;
    };

    for choice in choices {
        let delta = &choice["delta"];
        let finish_reason = choice["finish_reason"].as_str();

        if let Some(content) = delta["content"].as_str()
            && !content.is_empty()
        {
            chunks.push(Ok(StreamChunk::Text {
                text: content.to_string(),
            }));
        }

        if let Some(tool_calls) = delta["tool_calls"].as_array() {
            for tc in tool_calls {
                let index = tc["index"].as_u64().unwrap_or(0) as usize;
                if index > 128 {
                    continue;
                }
                while state.calls.len() <= index {
                    state
                        .calls
                        .push((String::new(), String::new(), String::new()));
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

        if matches!(finish_reason, Some("tool_calls" | "stop" | "length")) {
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
