use futures::stream::Stream;
use loopal_error::{LoopalError, ProviderError};
use loopal_provider_api::{StopReason, StreamChunk};
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Context, Poll};

pub(crate) struct GoogleStream {
    pub(crate) inner: Pin<Box<dyn Stream<Item = Result<String, LoopalError>> + Send>>,
    pub(crate) buffer: VecDeque<Result<StreamChunk, LoopalError>>,
}

impl Stream for GoogleStream {
    type Item = Result<StreamChunk, LoopalError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if let Some(item) = this.buffer.pop_front() {
            return Poll::Ready(Some(item));
        }

        match this.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(data))) => {
                let chunks = parse_google_event(&data);
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

unsafe impl Send for GoogleStream {}
impl Unpin for GoogleStream {}

pub(crate) fn parse_google_event(data: &str) -> Vec<Result<StreamChunk, LoopalError>> {
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

    // Usage metadata
    if let Some(usage) = parsed.get("usageMetadata") {
        let input = usage["promptTokenCount"].as_u64().unwrap_or(0) as u32;
        let output = usage["candidatesTokenCount"].as_u64().unwrap_or(0) as u32;
        let thinking = usage["thoughtsTokenCount"].as_u64().unwrap_or(0) as u32;
        if input > 0 || output > 0 {
            chunks.push(Ok(StreamChunk::Usage {
                input_tokens: input,
                output_tokens: output,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                thinking_tokens: thinking,
            }));
        }
    }

    // Candidates
    if let Some(candidates) = parsed["candidates"].as_array() {
        for candidate in candidates {
            let finish_reason = candidate["finishReason"].as_str();

            if let Some(parts) = candidate["content"]["parts"].as_array() {
                for part in parts {
                    if let Some(text) = part["text"].as_str()
                        && !text.is_empty()
                    {
                        if part
                            .get("thought")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                        {
                            chunks.push(Ok(StreamChunk::Thinking {
                                text: text.to_string(),
                            }));
                        } else {
                            chunks.push(Ok(StreamChunk::Text {
                                text: text.to_string(),
                            }));
                        }
                    }

                    if let Some(fc) = part.get("functionCall") {
                        let name = fc["name"].as_str().unwrap_or("").to_string();
                        let args = fc.get("args").cloned().unwrap_or(json!({}));
                        chunks.push(Ok(StreamChunk::ToolUse {
                            id: format!("call_{}", uuid_v4_simple()),
                            name,
                            input: args,
                        }));
                    }
                }
            }

            match finish_reason {
                Some("MAX_TOKENS") => {
                    chunks.push(Ok(StreamChunk::Done {
                        stop_reason: StopReason::MaxTokens,
                    }));
                }
                Some("STOP") => {
                    chunks.push(Ok(StreamChunk::Done {
                        stop_reason: StopReason::EndTurn,
                    }));
                }
                _ => {}
            }
        }
    }

    chunks
}

/// Simple pseudo-random ID generator (no uuid dep needed).
pub(crate) fn uuid_v4_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{nanos:x}")
}
