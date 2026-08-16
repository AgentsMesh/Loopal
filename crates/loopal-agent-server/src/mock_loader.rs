//! Load scripted LLM streams for system integration tests.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_provider_api::{ChatParams, ChatStream, Provider, StopReason, StreamChunk};

/// Load a mock provider from a JSON fixture file.
pub fn load_mock_provider(path: &str) -> anyhow::Result<Arc<dyn Provider>> {
    let content = std::fs::read_to_string(path)?;
    let raw: Vec<Vec<serde_json::Value>> = serde_json::from_str(&content)?;
    let calls = raw
        .into_iter()
        .map(|call| call.into_iter().map(parse_item).collect())
        .collect::<Vec<_>>();
    Ok(Arc::new(JsonMockProvider {
        calls: std::sync::Mutex::new(VecDeque::from(calls)),
    }))
}

enum MockItem {
    Chunk(Result<StreamChunk, LoopalError>),
    Delay(Duration),
}

fn parse_item(v: serde_json::Value) -> MockItem {
    let chunk = match v["type"].as_str().unwrap_or("") {
        "delay" => return MockItem::Delay(Duration::from_millis(v["ms"].as_u64().unwrap_or(0))),
        "text" => StreamChunk::Text {
            text: v["text"].as_str().unwrap_or("").into(),
        },
        "thinking" => StreamChunk::Thinking {
            text: v["text"].as_str().unwrap_or("").into(),
        },
        "thinking_signature" => StreamChunk::ThinkingSignature {
            signature: v["signature"].as_str().unwrap_or("test-signature").into(),
        },
        "tool_use" => StreamChunk::ToolUse {
            id: v["id"].as_str().unwrap_or("tc-1").into(),
            name: v["name"].as_str().unwrap_or("").into(),
            input: v.get("input").cloned().unwrap_or_default(),
        },
        "usage" => StreamChunk::Usage {
            input_tokens: v["input"].as_u64().unwrap_or(10) as u32,
            output_tokens: v["output"].as_u64().unwrap_or(5) as u32,
            cache_creation_input_tokens: v["cache_creation"].as_u64().unwrap_or(0) as u32,
            cache_read_input_tokens: v["cache_read"].as_u64().unwrap_or(0) as u32,
            thinking_tokens: v["thinking"].as_u64().unwrap_or(0) as u32,
        },
        "done" => StreamChunk::Done {
            stop_reason: StopReason::EndTurn,
        },
        _ => StreamChunk::Text {
            text: String::new(),
        },
    };
    MockItem::Chunk(Ok(chunk))
}

struct JsonMockProvider {
    calls: std::sync::Mutex<VecDeque<Vec<MockItem>>>,
}

struct JsonMockStream {
    items: VecDeque<MockItem>,
    sleep: Option<Pin<Box<tokio::time::Sleep>>>,
}

impl futures::Stream for JsonMockStream {
    type Item = Result<StreamChunk, LoopalError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            if let Some(sleep) = self.sleep.as_mut() {
                if sleep.as_mut().poll(cx).is_pending() {
                    return Poll::Pending;
                }
                self.sleep = None;
            }
            match self.items.pop_front() {
                Some(MockItem::Chunk(chunk)) => return Poll::Ready(Some(chunk)),
                Some(MockItem::Delay(duration)) => {
                    self.sleep = Some(Box::pin(tokio::time::sleep(duration)));
                }
                None => return Poll::Ready(None),
            }
        }
    }
}

impl Unpin for JsonMockStream {}

#[async_trait]
impl Provider for JsonMockProvider {
    fn name(&self) -> &str {
        "anthropic" // Match default model prefix for resolve_provider
    }

    async fn stream_chat(&self, _p: &ChatParams) -> Result<ChatStream, LoopalError> {
        let chunks = self
            .calls
            .lock()
            .expect("mock provider mutex poisoned")
            .pop_front()
            .ok_or_else(|| {
                LoopalError::Other(
                    "JsonMockProvider script exhausted: add an explicit response for every expected LLM call"
                        .into(),
                )
            })?;
        let stream = JsonMockStream {
            items: VecDeque::from(chunks),
            sleep: None,
        };
        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    fn params() -> ChatParams {
        ChatParams::new("mock".into(), Vec::new(), String::new())
    }

    #[test]
    fn parses_rich_script_items() {
        assert!(matches!(
            parse_item(serde_json::json!({"type": "thinking", "text": "plan"})),
            MockItem::Chunk(Ok(StreamChunk::Thinking { text })) if text == "plan"
        ));
        assert!(matches!(
            parse_item(serde_json::json!({"type": "thinking_signature", "signature": "sig"})),
            MockItem::Chunk(Ok(StreamChunk::ThinkingSignature { signature })) if signature == "sig"
        ));
        assert!(matches!(
            parse_item(serde_json::json!({"type": "delay", "ms": 25})),
            MockItem::Delay(duration) if duration == Duration::from_millis(25)
        ));
    }

    #[tokio::test]
    async fn explicit_empty_call_is_eof_but_queue_underflow_is_a_fixture_error() {
        let provider = JsonMockProvider {
            calls: std::sync::Mutex::new(VecDeque::from([Vec::new()])),
        };

        let mut stream = provider
            .stream_chat(&params())
            .await
            .expect("an explicit empty call is a valid EOF fixture");
        assert!(stream.next().await.is_none());

        let error = provider
            .stream_chat(&params())
            .await
            .err()
            .expect("an unscripted call must fail fast");
        assert!(!error.is_retryable());
        assert!(
            error
                .to_string()
                .contains("JsonMockProvider script exhausted")
        );
    }

    #[tokio::test(start_paused = true)]
    async fn delayed_script_yields_pending_before_the_next_chunk() {
        let mut stream = JsonMockStream {
            items: VecDeque::from([
                MockItem::Delay(Duration::from_secs(1)),
                MockItem::Chunk(Ok(StreamChunk::Text {
                    text: "after delay".into(),
                })),
            ]),
            sleep: None,
        };
        let mut next = std::pin::pin!(stream.next());
        assert!(matches!(futures::poll!(&mut next), Poll::Pending));

        tokio::time::advance(Duration::from_secs(1)).await;
        assert!(
            matches!(next.await, Some(Ok(StreamChunk::Text { text })) if text == "after delay")
        );
    }
}
