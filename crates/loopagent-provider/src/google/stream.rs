use futures::stream::Stream;
use loopagent_types::error::{LoopAgentError, ProviderError};
use loopagent_types::provider::StreamChunk;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Context, Poll};

pub(crate) struct GoogleStream {
    pub(crate) inner: Pin<Box<dyn Stream<Item = Result<String, LoopAgentError>> + Send>>,
    pub(crate) buffer: VecDeque<Result<StreamChunk, LoopAgentError>>,
}

impl Stream for GoogleStream {
    type Item = Result<StreamChunk, LoopAgentError>;

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

pub(crate) fn parse_google_event(data: &str) -> Vec<Result<StreamChunk, LoopAgentError>> {
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

    // Usage metadata
    if let Some(usage) = parsed.get("usageMetadata") {
        let input = usage["promptTokenCount"].as_u64().unwrap_or(0) as u32;
        let output = usage["candidatesTokenCount"].as_u64().unwrap_or(0) as u32;
        if input > 0 || output > 0 {
            chunks.push(Ok(StreamChunk::Usage {
                input_tokens: input,
                output_tokens: output,
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
                        && !text.is_empty() {
                            chunks.push(Ok(StreamChunk::Text {
                                text: text.to_string(),
                            }));
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

            if finish_reason == Some("STOP") || finish_reason == Some("MAX_TOKENS") {
                chunks.push(Ok(StreamChunk::Done));
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
    format!("{:x}", nanos)
}
