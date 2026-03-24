use futures::stream::Stream;
use loopal_error::{LoopalError, ProviderError};
use loopal_provider_api::{StopReason, StreamChunk};
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Context, Poll};

pub(crate) struct OpenAiStream {
    pub(crate) inner: Pin<Box<dyn Stream<Item = Result<String, LoopalError>> + Send>>,
    pub(crate) buffer: VecDeque<Result<StreamChunk, LoopalError>>,
}

impl Stream for OpenAiStream {
    type Item = Result<StreamChunk, LoopalError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if let Some(item) = this.buffer.pop_front() {
            return Poll::Ready(Some(item));
        }

        match this.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(data))) => {
                let chunks = parse_responses_event(&data);
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

/// Parse a single SSE event from the OpenAI Responses API.
pub(crate) fn parse_responses_event(data: &str) -> Vec<Result<StreamChunk, LoopalError>> {
    let parsed: Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(e) => {
            return vec![Err(ProviderError::SseParse(format!(
                "invalid JSON: {e}: {data}"
            ))
            .into())];
        }
    };

    let event_type = parsed["type"].as_str().unwrap_or("");
    let mut chunks = Vec::new();

    match event_type {
        "response.output_text.delta" => {
            if let Some(delta) = parsed["delta"].as_str()
                && !delta.is_empty()
            {
                chunks.push(Ok(StreamChunk::Text {
                    text: delta.to_string(),
                }));
            }
        }
        "response.reasoning_summary_text.delta" => {
            if let Some(delta) = parsed["delta"].as_str() {
                chunks.push(Ok(StreamChunk::Thinking {
                    text: delta.to_string(),
                }));
            }
        }
        "response.output_item.done" => {
            parse_output_item_done(&parsed["item"], &mut chunks);
        }
        "response.completed" => {
            parse_completed(&parsed["response"], &mut chunks);
        }
        "response.failed" => {
            if let Some(err) = parsed["response"]["error"].as_object() {
                let code = err
                    .get("code")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let message = err
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error");

                if code == "rate_limit_exceeded" {
                    chunks.push(Err(ProviderError::RateLimited {
                        retry_after_ms: 30_000,
                    }
                    .into()));
                } else if code == "context_length_exceeded" {
                    chunks.push(Err(ProviderError::ContextOverflow {
                        message: format!("{code}: {message}"),
                    }
                    .into()));
                } else {
                    chunks.push(Err(ProviderError::Api {
                        status: 400,
                        message: format!("{code}: {message}"),
                    }
                    .into()));
                }
            } else {
                // Unrecognized error structure -- emit generic error
                chunks.push(Err(ProviderError::Api {
                    status: 400,
                    message: format!("response.failed: {}", parsed["response"]),
                }
                .into()));
            }
        }
        "response.incomplete" => {
            chunks.push(Ok(StreamChunk::Done {
                stop_reason: StopReason::MaxTokens,
            }));
        }
        _ => {} // Ignore other events (response.created, response.output_item.added, etc.)
    }

    chunks
}

/// Parse a completed output item (function_call, web_search_call, message, etc.).
fn parse_output_item_done(item: &Value, chunks: &mut Vec<Result<StreamChunk, LoopalError>>) {
    let item_type = item["type"].as_str().unwrap_or("");
    match item_type {
        "function_call" => {
            let call_id = item["call_id"].as_str().unwrap_or("").to_string();
            let name = item["name"].as_str().unwrap_or("").to_string();
            let args_str = item["arguments"].as_str().unwrap_or("{}");
            let input = serde_json::from_str(args_str).unwrap_or(json!({}));
            if !call_id.is_empty() && !name.is_empty() {
                chunks.push(Ok(StreamChunk::ToolUse {
                    id: call_id,
                    name,
                    input,
                }));
            }
        }
        "web_search_call" => {
            let id = item["id"].as_str().unwrap_or("").to_string();
            let query = item["action"]["query"].as_str().unwrap_or("").to_string();
            chunks.push(Ok(StreamChunk::ServerToolUse {
                id: id.clone(),
                name: "web_search".to_string(),
                input: json!({"query": query}),
            }));
            // OpenAI server-side search results are implicit (not streamed separately).
            // Emit a synthetic result to close the TUI pending state.
            chunks.push(Ok(StreamChunk::ServerToolResult {
                block_type: "web_search_tool_result".to_string(),
                tool_use_id: id,
                content: json!({"status": "completed"}),
            }));
        }
        _ => {}
    }
}

/// Parse the response.completed event for usage stats.
fn parse_completed(response: &Value, chunks: &mut Vec<Result<StreamChunk, LoopalError>>) {
    if let Some(usage) = response.get("usage") {
        let input_tokens = usage["input_tokens"].as_u64().unwrap_or(0) as u32;
        let output_tokens = usage["output_tokens"].as_u64().unwrap_or(0) as u32;
        let cache_read = usage["input_tokens_details"]["cached_tokens"]
            .as_u64()
            .unwrap_or(0) as u32;
        let thinking_tokens = usage["output_tokens_details"]["reasoning_tokens"]
            .as_u64()
            .unwrap_or(0) as u32;
        chunks.push(Ok(StreamChunk::Usage {
            input_tokens,
            output_tokens,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: cache_read,
            thinking_tokens,
        }));
    }
    chunks.push(Ok(StreamChunk::Done {
        stop_reason: StopReason::EndTurn,
    }));
}
