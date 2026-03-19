use futures::stream::Stream;
use loopal_types::error::{LoopalError, ProviderError};
use reqwest::Response;
use std::pin::Pin;
use std::task::{Context, Poll};

use eventsource_stream::Eventsource;
use futures::StreamExt;

/// A stream of SSE event data strings parsed from an HTTP response.
///
/// # Testing note
/// `SseStream` is not unit-testable in isolation because:
/// - `from_response` requires a real `reqwest::Response` (which cannot be constructed without
///   an HTTP server or `mockito`/`wiremock`).
/// - The `Stream` impl simply delegates to the inner stream with no branching logic of its own.
/// - The actual SSE parsing is handled by the `eventsource_stream` crate.
/// - The `[DONE]` sentinel filtering and error mapping are thin wrappers best exercised via
///   integration tests against a mock HTTP server.
///
/// The event-parsing logic that consumes SSE data (e.g., `parse_anthropic_event`,
/// `parse_openai_event`, `parse_google_event`) is tested extensively in their respective modules.
pub struct SseStream {
    inner: Pin<Box<dyn Stream<Item = Result<String, LoopalError>> + Send>>,
}

impl SseStream {
    /// Create an SSE stream from a reqwest response with a streaming body.
    pub fn from_response(response: Response) -> Self {
        let byte_stream = response.bytes_stream();
        let event_stream = byte_stream.eventsource();

        let mapped = event_stream.filter_map(|result| {
            let out = match result {
                Ok(event) => {
                    let data = event.data;
                    if data == "[DONE]" {
                        None
                    } else {
                        Some(Ok(data))
                    }
                }
                Err(e) => Some(Err(LoopalError::Provider(ProviderError::SseParse(
                    e.to_string(),
                )))),
            };
            futures::future::ready(out)
        });

        Self {
            inner: Box::pin(mapped),
        }
    }
}

impl Stream for SseStream {
    type Item = Result<String, LoopalError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.as_mut().poll_next(cx)
    }
}
