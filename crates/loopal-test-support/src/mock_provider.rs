//! Mock LLM providers for deterministic integration tests.
//!
//! `MockStreamChunks` supports an optional per-chunk delay for simulating
//! slow streaming. When `delay` is `None`, chunks are returned synchronously
//! (zero overhead, identical to previous behavior).

use std::collections::VecDeque;
use std::time::Duration;

use loopal_error::LoopalError;
use loopal_provider_api::{ChatParams, ChatStream, Provider, StreamChunk};
use tokio::time::Sleep;

/// In-memory `Stream` with optional per-chunk delay.
///
/// - `delay: None` → synchronous drain (default, zero-cost)
/// - `delay: Some(d)` → each chunk preceded by an async sleep
pub struct MockStreamChunks {
    pub chunks: VecDeque<Result<StreamChunk, LoopalError>>,
    delay: Option<Duration>,
    pending_sleep: Option<std::pin::Pin<Box<Sleep>>>,
}

impl MockStreamChunks {
    pub fn new(chunks: VecDeque<Result<StreamChunk, LoopalError>>) -> Self {
        Self {
            chunks,
            delay: None,
            pending_sleep: None,
        }
    }

    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }
}

impl futures::Stream for MockStreamChunks {
    type Item = Result<StreamChunk, LoopalError>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        // Fast path: no delay configured — synchronous drain (unchanged behavior)
        if self.delay.is_none() {
            return std::task::Poll::Ready(self.chunks.pop_front());
        }

        // Slow path: delay between chunks
        if let Some(ref mut sleep) = self.pending_sleep {
            match sleep.as_mut().poll(cx) {
                std::task::Poll::Pending => return std::task::Poll::Pending,
                std::task::Poll::Ready(()) => {
                    self.pending_sleep = None;
                }
            }
        }

        // Return next chunk, then arm delay for the following one
        let item = self.chunks.pop_front();
        if item.is_some() && !self.chunks.is_empty() {
            if let Some(d) = self.delay {
                self.pending_sleep = Some(Box::pin(tokio::time::sleep(d)));
            }
        }
        std::task::Poll::Ready(item)
    }
}

impl Unpin for MockStreamChunks {}

// ── Providers ──────────────────────────────────────────────────────

/// Single-call mock provider. Returns the configured chunks once, then empty.
pub struct MockProvider {
    pub chunks: std::sync::Mutex<Option<Vec<Result<StreamChunk, LoopalError>>>>,
}

impl MockProvider {
    pub fn new(chunks: Vec<Result<StreamChunk, LoopalError>>) -> Self {
        Self {
            chunks: std::sync::Mutex::new(Some(chunks)),
        }
    }
}

#[async_trait::async_trait]
impl Provider for MockProvider {
    fn name(&self) -> &str {
        "anthropic"
    }
    async fn stream_chat(&self, _params: &ChatParams) -> Result<ChatStream, LoopalError> {
        let chunks = self.chunks.lock().unwrap().take().unwrap_or_default();
        Ok(Box::pin(MockStreamChunks::new(VecDeque::from(chunks))))
    }
}

/// Multi-call mock provider. Pops a fresh chunk sequence per `stream_chat` call.
pub struct MultiCallProvider {
    pub calls: std::sync::Mutex<VecDeque<Vec<Result<StreamChunk, LoopalError>>>>,
    /// Optional per-chunk delay applied to all returned streams.
    delay: Option<Duration>,
}

impl MultiCallProvider {
    pub fn new(calls: Vec<Vec<Result<StreamChunk, LoopalError>>>) -> Self {
        Self {
            calls: std::sync::Mutex::new(VecDeque::from(calls)),
            delay: None,
        }
    }

    /// Create a provider that inserts a delay between each streamed chunk.
    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }
}

#[async_trait::async_trait]
impl Provider for MultiCallProvider {
    fn name(&self) -> &str {
        "anthropic"
    }
    async fn stream_chat(&self, _p: &ChatParams) -> Result<ChatStream, LoopalError> {
        let chunks = self.calls.lock().unwrap().pop_front().unwrap_or_default();
        let mut stream = MockStreamChunks::new(VecDeque::from(chunks));
        if let Some(d) = self.delay {
            stream = stream.with_delay(d);
        }
        Ok(Box::pin(stream))
    }
}
